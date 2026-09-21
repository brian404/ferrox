use anyhow::Result;
use hyper::body::Incoming;
use hyper::header;
use hyper::{Method, Request, Response, StatusCode};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::body::{full_body, ResponseBody};
use crate::config::Config;
use crate::handlers::{handle_health, handle_hello, proxy_request, serve_static, ProxyClient};
use crate::metrics;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_request_id() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[tracing::instrument(
    skip(req, config, client),
    fields(request_id = next_request_id(), method = %req.method(), path = %req.uri().path())
)]
pub async fn route(
    req: Request<Incoming>,
    config: Arc<Config>,
    peer: SocketAddr,
    client: ProxyClient,
) -> Result<Response<ResponseBody>, hyper::Error> {
    let path = req.uri().path();
    let method = req.method().clone();

    tracing::info!("request received");

    if method == Method::TRACE {
        return Ok(record(
            method_not_allowed("GET, HEAD, POST, PUT, DELETE, OPTIONS"),
            "trace_rejected",
        ));
    }

    let (response, route_label) = if path == "/metrics" {
        if method == Method::GET || method == Method::HEAD {
            (metrics_response(), "/metrics")
        } else {
            (method_not_allowed("GET, HEAD"), "/metrics")
        }
    } else if path == "/api" || path.starts_with("/api/") {
        (proxy_request(req, config, peer, client).await?, "/api")
    } else if path == "/hello" {
        if method == Method::GET || method == Method::HEAD {
            (handle_hello(req).await?, "/hello")
        } else {
            (method_not_allowed("GET, HEAD"), "/hello")
        }
    } else if path == "/health" {
        if method == Method::GET || method == Method::HEAD {
            (handle_health(req).await?, "/health")
        } else {
            (method_not_allowed("GET, HEAD"), "/health")
        }
    } else if method == Method::GET || method == Method::HEAD {
        (serve_static(req).await?, "/static")
    } else {
        (method_not_allowed("GET, HEAD"), "/static")
    };

    Ok(record(response, route_label))
}

fn record(response: Response<ResponseBody>, route: &'static str) -> Response<ResponseBody> {
    let status = response.status().as_u16();
    let bytes = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    metrics::record_request(route, status, bytes);
    response
}

fn metrics_response() -> Response<ResponseBody> {
    let body = metrics::render_prometheus();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")
        .body(full_body(body))
        .unwrap()
}

fn method_not_allowed(allowed: &str) -> Response<ResponseBody> {
    let response = Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, allowed)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(full_body("405 Method Not Allowed"))
        .unwrap();
    crate::headers::add_common_headers(response)
}
