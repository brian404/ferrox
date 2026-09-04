mod common;

use common::{raw_request, start_test_server};

#[tokio::test]
async fn hello_returns_200() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/hello", &[]).await;
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"Hello from Ferrox!");
}

#[tokio::test]
async fn health_returns_200() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/health", &[]).await;
    assert_eq!(res.status, 200);
}

#[tokio::test]
async fn unknown_path_returns_404() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/this-does-not-exist", &[]).await;
    assert_eq!(res.status, 404);
}

#[tokio::test]
async fn trace_method_is_rejected() {
    let port = start_test_server().await;
    let res = raw_request(port, "TRACE", "/hello", &[]).await;
    assert_eq!(res.status, 405);
}

#[tokio::test]
async fn patch_on_hello_is_rejected() {
    let port = start_test_server().await;
    let res = raw_request(port, "PATCH", "/hello", &[]).await;
    assert_eq!(res.status, 405);
}

#[tokio::test]
async fn path_traversal_is_blocked() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/../Cargo.toml", &[]).await;
    assert_eq!(res.status, 404);
    assert!(!String::from_utf8_lossy(&res.body).contains("[package]"));
}

#[tokio::test]
async fn deep_traversal_is_blocked() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/../../../../etc/passwd", &[]).await;
    assert_eq!(res.status, 404);
    assert!(!String::from_utf8_lossy(&res.body).contains("root:"));
}

#[tokio::test]
async fn api_prefix_confusion_is_not_proxied() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/apiFoo", &[]).await;
    // Should hit the static file handler (404), not the proxy - a
    // 502/504 here would mean the /api prefix match got too loose.
    assert_eq!(res.status, 404);
}

#[tokio::test]
async fn api_with_no_upstream_returns_bad_gateway() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/api/test", &[]).await;
    assert_eq!(res.status, 502);
}

#[tokio::test]
async fn root_serves_index_html() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/", &[]).await;
    assert_eq!(res.status, 200);
    assert!(res.headers.to_lowercase().contains("content-type: text/html"));
}

#[tokio::test]
async fn security_headers_present() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/hello", &[]).await;
    let h = res.headers.to_lowercase();
    assert!(h.contains("x-content-type-options: nosniff"));
    assert!(h.contains("x-frame-options: deny"));
    assert!(h.contains("server: ferrox"));
}

#[tokio::test]
async fn range_request_returns_partial_content() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/index.html", &[("Range", "bytes=0-9")]).await;
    assert_eq!(res.status, 206);
    assert_eq!(res.body.len(), 10);
    assert!(res.headers.to_lowercase().contains("content-range: bytes 0-9/"));
}
