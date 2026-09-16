use anyhow::Result;
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{header, Request, Response, StatusCode};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

use bytes::Bytes;
use http_body_util::BodyExt;

use crate::body::{full_body, ResponseBody};
use crate::body_limit::{content_length_exceeds_limit, LimitedBody};
use crate::config::Config;

pub type ProxyClient = Client<HttpConnector, LimitedBody>;

pub fn build_client() -> ProxyClient {
    Client::builder(TokioExecutor::new()).build_http()
}

pub async fn proxy_request(
    mut req: Request<Incoming>,
    config: Arc<Config>,
    peer: SocketAddr,
    client: ProxyClient,
) -> Result<Response<ResponseBody>, hyper::Error> {
    let upstream = config.proxy_upstream.clone();

    let is_websocket_upgrade = req
        .headers()
        .get(header::UPGRADE)
        .map_or(false, |v| v.as_bytes().eq_ignore_ascii_case(b"websocket"))
        && req
            .headers()
            .get(header::CONNECTION)
            .and_then(|v| v.to_str().ok())
            .map_or(false, |v| v.to_lowercase().contains("upgrade"));

    if is_websocket_upgrade {
        return proxy_websocket(req, upstream, peer).await;
    }

    // Reject an honestly-declared oversized body before reading any
    // of it at all - the common case, and the cleanest to respond to.
    if content_length_exceeds_limit(req.headers()) {
        tracing::warn!(
            "Rejected oversized request body (Content-Length) from {}",
            peer
        );
        return Ok(payload_too_large());
    }

    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
    let stripped_path = path.strip_prefix("/api").unwrap_or(&path);
    let stripped_path = if stripped_path.is_empty() { "/" } else { stripped_path };

    let full_uri = format!("http://{}{}{}", upstream, stripped_path, query);
    *req.uri_mut() = match full_uri.parse() {
        Ok(uri) => uri,
        Err(e) => {
            tracing::error!("Failed to build upstream URI: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(full_body("Bad Gateway - Invalid upstream URI"))
                .unwrap());
        }
    };

    let headers = req.headers_mut();
    headers.remove(header::HOST);
    if let Ok(host_value) = upstream.parse() {
        headers.insert(header::HOST, host_value);
    }

    let forwarded_for = match headers.get("X-Forwarded-For") {
        Some(existing) => format!("{}, {}", existing.to_str().unwrap_or(""), peer.ip()),
        None => peer.ip().to_string(),
    };
    if let Ok(value) = forwarded_for.parse() {
        headers.insert("X-Forwarded-For", value);
    }

    // Wrapped here, right before forwarding - the defense-in-depth
    // layer for chunked/no-Content-Length bodies. If a client sneaks
    // more than the limit past the check above, this aborts the
    // stream mid-request rather than forwarding it unbounded.
    let (parts, body) = req.into_parts();
    let req = Request::from_parts(parts, LimitedBody::new(body));

    let upstream_res = match timeout(Duration::from_secs(60), client.request(req)).await {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            tracing::error!("Proxy request to {} failed: {}", upstream, e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(full_body("Bad Gateway - Upstream unreachable"))
                .unwrap());
        }
        Err(_) => {
            tracing::error!("Timeout waiting for upstream response from {}", upstream);
            return Ok(Response::builder()
                .status(StatusCode::GATEWAY_TIMEOUT)
                .body(full_body("Gateway Timeout - Upstream response"))
                .unwrap());
        }
    };

    let mut response = Response::builder()
        .status(upstream_res.status())
        .version(upstream_res.version());

    for (key, value) in upstream_res.headers() {
        if key != header::CONNECTION && key != header::TRANSFER_ENCODING {
            response = response.header(key, value);
        }
    }

    let body_bytes = match upstream_res.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::error!("Failed reading upstream body from {}: {}", upstream, e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(full_body("Bad Gateway - Upstream body error"))
                .unwrap());
        }
    };

    let response = response.body(full_body(body_bytes)).unwrap();
    Ok(crate::headers::add_common_headers(response))
}

fn payload_too_large() -> Response<ResponseBody> {
    let response = Response::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(full_body("Payload Too Large"))
        .unwrap();
    crate::headers::add_common_headers(response)
}

async fn proxy_websocket(
    mut req: Request<Incoming>,
    upstream: String,
    peer: SocketAddr,
) -> Result<Response<ResponseBody>, hyper::Error> {
    let client_upgrade = hyper::upgrade::on(&mut req);

    let upstream_stream = match TcpStream::connect(&upstream).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("WS connect failed: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(full_body("Bad Gateway - WS upstream"))
                .unwrap());
        }
    };

    let io = TokioIo::new(upstream_stream);
    let (mut sender, conn) = ClientBuilder::new().handshake(io).await?;

    tokio::spawn(async move {
        if let Err(e) = conn.with_upgrades().await {
            tracing::error!("Upstream connection error: {}", e);
        }
    });

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.to_string())
        .unwrap_or_else(|| "/".to_string());
    *req.uri_mut() = path_and_query.parse().unwrap();

    let headers = req.headers_mut();
    let forwarded_for = match headers.get("X-Forwarded-For") {
        Some(existing) => format!("{}, {}", existing.to_str().unwrap_or(""), peer.ip()),
        None => peer.ip().to_string(),
    };
    if let Ok(value) = forwarded_for.parse() {
        headers.insert("X-Forwarded-For", value);
    }

    let mut upstream_res = sender.send_request(req).await?;

    if upstream_res.status() != StatusCode::SWITCHING_PROTOCOLS {
        return Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(full_body("Bad Gateway - WS upgrade failed"))
            .unwrap());
    }

    let upstream_upgrade = hyper::upgrade::on(&mut upstream_res);

    let mut response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .version(upstream_res.version());

    for (key, value) in upstream_res.headers() {
        if key != header::CONNECTION && key != header::UPGRADE {
            response = response.header(key, value);
        }
    }

    response = response.header(header::CONNECTION, "Upgrade");
    response = response.header(header::UPGRADE, "websocket");

    let response = response.body(full_body(Bytes::new())).unwrap();

    tokio::spawn(async move {
        let client = match client_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("Client WS upgrade failed: {}", e);
                return;
            }
        };
        let upstream = match upstream_upgrade.await {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("Upstream WS upgrade failed: {}", e);
                return;
            }
        };

        let mut client_io = TokioIo::new(client);
        let mut upstream_io = TokioIo::new(upstream);

        if let Err(e) = copy_bidirectional(&mut client_io, &mut upstream_io).await {
            tracing::debug!("WS proxy stream closed: {}", e);
        }
    });

    Ok(crate::headers::add_common_headers(response))
}
