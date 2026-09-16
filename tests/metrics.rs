mod common;

use common::{raw_request, start_test_server};

#[tokio::test]
async fn metrics_endpoint_reports_prometheus_format() {
    let port = start_test_server().await;

    let _ = raw_request(port, "GET", "/hello", &[]).await;

    let res = raw_request(port, "GET", "/metrics", &[]).await;
    assert_eq!(res.status, 200);
    assert!(res.headers.to_lowercase().contains("content-type: text/plain"));

    let body = String::from_utf8_lossy(&res.body);
    assert!(body.contains("ferrox_requests_total"));
    assert!(body.contains("ferrox_uptime_seconds"));
}
