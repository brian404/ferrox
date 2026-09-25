mod common;

use common::{raw_request, start_test_server};

#[tokio::test]
async fn etag_present_and_if_none_match_returns_304() {
    let port = start_test_server().await;

    let first = raw_request(port, "GET", "/", &[]).await;
    assert_eq!(first.status, 200);
    let etag = first
        .headers
        .lines()
        .find(|l| l.to_lowercase().starts_with("etag:"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .expect("expected an ETag header");

    let second = raw_request(port, "GET", "/", &[("If-None-Match", &etag)]).await;
    assert_eq!(second.status, 304);
    assert!(second.body.is_empty());
}
