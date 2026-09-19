mod common;

use common::{raw_request, start_test_server};

#[tokio::test]
async fn compressible_response_is_gzipped_when_accepted() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/", &[("Accept-Encoding", "gzip")]).await;
    assert_eq!(res.status, 200);
    assert!(res.headers.to_lowercase().contains("content-encoding: gzip"));
    assert_eq!(&res.body[..2], &[0x1f, 0x8b]);
}

#[tokio::test]
async fn response_is_not_compressed_without_accept_encoding() {
    let port = start_test_server().await;
    let res = raw_request(port, "GET", "/", &[]).await;
    assert_eq!(res.status, 200);
    assert!(!res.headers.to_lowercase().contains("content-encoding: gzip"));
}
