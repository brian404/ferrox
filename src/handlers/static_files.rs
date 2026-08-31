use anyhow::Result;
use bytes::Bytes;
use hyper::body::Incoming;
use hyper::header;
use hyper::{Request, Response, StatusCode};
use mime_guess::from_path;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::body::{file_body, full_body, ResponseBody};
use crate::cache::{self, CachedFile, MAX_CACHEABLE_SIZE};
use crate::security;

pub async fn serve_static(req: Request<Incoming>) -> Result<Response<ResponseBody>, hyper::Error> {
    let path = req.uri().path();

    if security::contains_dotdot(path) {
        tracing::warn!("Rejected path traversal attempt: {}", path);
        return Ok(not_found_page().await);
    }

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

    if let Some((contents, content_type, cache_control)) = cache::get(&resolved_path, mtime) {
        tracing::debug!("cache hit: {}", resolved_path.display());
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, contents.len() as u64)
            .header(header::CACHE_CONTROL, cache_control)
            .body(full_body(contents))
            .unwrap();
        return Ok(crate::headers::add_common_headers(response));
    }

    let content_type = from_path(&resolved_path).first_or_octet_stream().to_string();
    let cache_control = cache_control_for(&resolved_path);

    if size > MAX_CACHEABLE_SIZE {
        tracing::debug!("streaming (uncached, {} bytes): {}", size, resolved_path.display());
        let file = match File::open(&resolved_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to open {}: {}", resolved_path.display(), e);
                return Ok(not_found_page().await);
            }
        };

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, size)
            .header(header::CACHE_CONTROL, cache_control)
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

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, contents.len() as u64)
        .header(header::CACHE_CONTROL, cache_control)
        .body(full_body(contents))
        .unwrap();

    Ok(crate::headers::add_common_headers(response))
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
