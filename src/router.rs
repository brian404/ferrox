use anyhow::Result;
use hyper::body::Incoming;
use hyper::header;
use hyper::{Method, Request, Response, StatusCode};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::body::{full_body, ResponseBody};
use crate::config::Config;
use crate::handlers::{handle_health, handle_hello, proxy_request, serve_static, ProxyClient};

pub async fn route(
    req: Request<Incoming>,
    config: Arc<Config>,
    peer: SocketAddr,
    client: ProxyClient,
) -> Result<Response<ResponseBody>, hyper::Error> {
    let path = req.uri().path();
    let method = req.method().clone();

    tracing::info!("Routing request: {} {}", method, path);

    if method == Method::TRACE {
        return Ok(method_not_allowed("GET, HEAD, POST, PUT, DELETE, OPTIONS"));
    }

    if path == "/api" || path.starts_with("/api/") {
        proxy_request(req, config, peer, client).await
    } else if path == "/hello" {
        if method == Method::GET || method == Method::HEAD {
            handle_hello(req).await
        } else {
            Ok(method_not_allowed("GET, HEAD"))
        }
    } else if path == "/health" {
        if method == Method::GET || method == Method::HEAD {
            handle_health(req).await
        } else {
            Ok(method_not_allowed("GET, HEAD"))
        }
    } else if method == Method::GET || method == Method::HEAD {
        serve_static(req).await
    } else {
        Ok(method_not_allowed("GET, HEAD"))
    }
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
