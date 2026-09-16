use anyhow::Result;
use bytes::Bytes;
use hyper::body::Incoming;
use hyper::header;
use hyper::{Request, Response, StatusCode};
use mime_guess::from_path;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::body::{file_body, file_body_range, full_body, ResponseBody};
use crate::cache::{self, CachedFile, MAX_CACHEABLE_SIZE};
use crate::range::{parse_range, RangeOutcome};
use crate::security;

pub async fn serve_static(req: Request<Incoming>) -> Result<Response<ResponseBody>, hyper::Error> {
    let path = req.uri().path();

    if security::contains_dotdot(path) {
        tracing::warn!("Rejected path traversal attempt: {}", path);
        return Ok(not_found_page().await);
    }

    let range_header = req
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let if_modified_since = req
        .headers()
        .get(header::IF_MODIFIED_SINCE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| httpdate::parse_http_date(s).ok());

    let subpath = path.trim_start_matches('/');

    let file_path = if path == "/" || path.is_empty() {
        PathBuf::from(security::public_dir()).join("index.html")
    } else {
        PathBuf::from(security::public_dir()).join(subpath)
    };

    let (resolved_path, mtime, size) = match security::resolve_within_public(&file_path).await {
        Some(r) => r,
        None => return Ok(not_found_page().await),
    };

    // A 304 short-circuits everything below it - no point deciding
    // between cache/stream/range for a body we're not going to send.
    if let Some(since) = if_modified_since {
        if truncate_to_secs(mtime) <= since {
            return Ok(not_modified(mtime));
        }
    }

    let range = parse_range(range_header.as_deref(), size);

    if let RangeOutcome::Unsatisfiable = range {
        return Ok(range_not_satisfiable(size));
    }

    if let Some((contents, content_type, cache_control)) = cache::get(&resolved_path, mtime) {
        tracing::debug!("cache hit: {}", resolved_path.display());
        return Ok(build_cached_response(contents, content_type, cache_control, mtime, range));
    }

    let content_type = from_path(&resolved_path).first_or_octet_stream().to_string();
    let cache_control = cache_control_for(&resolved_path);

    if size > MAX_CACHEABLE_SIZE {
        tracing::debug!("streaming (uncached, {} bytes): {}", size, resolved_path.display());
        let mut file = match File::open(&resolved_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to open {}: {}", resolved_path.display(), e);
                return Ok(not_found_page().await);
            }
        };

        if let RangeOutcome::Satisfiable(start, end) = range {
            if let Err(e) = file.seek(SeekFrom::Start(start)).await {
                tracing::error!("Failed to seek {}: {}", resolved_path.display(), e);
                return Ok(internal_error());
            }
            let len = end - start + 1;
            let response = Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_LENGTH, len)
                .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, size))
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::LAST_MODIFIED, httpdate::fmt_http_date(mtime))
                .body(file_body_range(file, len))
                .unwrap();
            return Ok(crate::headers::add_common_headers(response));
        }

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, size)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::LAST_MODIFIED, httpdate::fmt_http_date(mtime))
            .body(file_body(file))
            .unwrap();
        return Ok(crate::headers::add_common_headers(response));
    }

    tracing::debug!("cache miss: {}", resolved_path.display());
    let mut file = match File::open(&resolved_path).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open {}: {}", resolved_path.display(), e);
            return Ok(not_found_page().await);
        }
    };

    let mut contents = Vec::new();
    if let Err(e) = file.read_to_end(&mut contents).await {
        tracing::error!("Failed to read file {}: {}", resolved_path.display(), e);
        return Ok(internal_error());
    }
    let contents = Bytes::from(contents);

    cache::insert(
        resolved_path.clone(),
        CachedFile {
            contents: contents.clone(),
            mtime,
            content_type: content_type.clone(),
            cache_control,
        },
    );

    Ok(build_cached_response(contents, content_type, cache_control, mtime, range))
}

/// HTTP-date has whole-second resolution, but a filesystem mtime can
/// carry sub-second precision - without truncating, a file modified
/// at (for example) 12:00:00.500 would compare as "newer" than a
/// client's cached 12:00:00, even though they're effectively the same
/// second as far as HTTP caching semantics go.
fn truncate_to_secs(t: SystemTime) -> SystemTime {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => UNIX_EPOCH + Duration::from_secs(d.as_secs()),
        Err(_) => t,
    }
}

fn not_modified(mtime: SystemTime) -> Response<ResponseBody> {
    let response = Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::LAST_MODIFIED, httpdate::fmt_http_date(mtime))
        .body(full_body(Bytes::new()))
        .unwrap();
    crate::headers::add_common_headers(response)
}

fn build_cached_response(
    contents: Bytes,
    content_type: String,
    cache_control: &'static str,
    mtime: SystemTime,
    range: RangeOutcome,
) -> Response<ResponseBody> {
    let size = contents.len() as u64;
    let last_modified = httpdate::fmt_http_date(mtime);

    let response = if let RangeOutcome::Satisfiable(start, end) = range {
        Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, end - start + 1)
            .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, size))
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::LAST_MODIFIED, last_modified)
            .body(full_body(contents.slice(start as usize..=end as usize)))
            .unwrap()
    } else {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, size)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::LAST_MODIFIED, last_modified)
            .body(full_body(contents))
            .unwrap()
    };

    crate::headers::add_common_headers(response)
}

fn range_not_satisfiable(size: u64) -> Response<ResponseBody> {
    let response = Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(header::CONTENT_RANGE, format!("bytes */{}", size))
        .header(header::CONTENT_TYPE, "text/plain")
        .body(full_body("Range Not Satisfiable"))
        .unwrap();
    crate::headers::add_common_headers(response)
}

fn cache_control_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("css") | Some("js") | Some("png") | Some("jpg") | Some("jpeg") | Some("gif")
        | Some("svg") | Some("woff") | Some("woff2") | Some("ico") => "public, max-age=3600",
        _ => "no-cache",
    }
}

async fn not_found_page() -> Response<ResponseBody> {
    match tokio::fs::read(PathBuf::from(security::public_dir()).join("404.html")).await {
        Ok(contents) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(full_body(contents))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(full_body("Not Found"))
            .unwrap(),
    }
}

fn internal_error() -> Response<ResponseBody> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(full_body("Internal Server Error"))
        .unwrap()
}
