mod common;

use common::{start_fake_ws_backend, start_test_server_custom};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn websocket_upgrade_and_echo_relay_through_proxy() {
    let backend_port = start_fake_ws_backend().await;
    let proxy_port = start_test_server_custom(
        &format!("127.0.0.1:{}", backend_port),
        Duration::from_secs(30),
    )
    .await;

    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).await.unwrap();

    let request = "GET /api HTTP/1.1\r\n\
                   Host: 127.0.0.1\r\n\
                   Connection: Upgrade\r\n\
                   Upgrade: websocket\r\n\
                   Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                   Sec-WebSocket-Version: 13\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    loop {
        let n = stream.read(&mut buf[total..]).await.unwrap();
        assert!(n > 0, "connection closed before upgrade response completed");
        total += n;
        if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let response_text = String::from_utf8_lossy(&buf[..total]);
    assert!(
        response_text.starts_with("HTTP/1.1 101"),
        "expected 101 Switching Protocols, got: {}",
        response_text
    );

    // Prove the relay actually pipes bytes both ways after the
    // upgrade, not just that the handshake itself succeeded.
    stream
        .write_all(b"hello through the tunnel")
        .await
        .unwrap();

    let mut echo_buf = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut echo_buf))
        .await
        .expect("timed out waiting for echoed bytes")
        .unwrap();

    assert_eq!(&echo_buf[..n], b"hello through the tunnel");
}
