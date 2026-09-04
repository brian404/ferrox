 use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use ferrox::{build_client, serve_forever, Config};

/// Starts a real ferrox server on an OS-assigned free port and
/// returns that port. Runs for the lifetime of the test's tokio
/// runtime - no explicit shutdown needed, since #[tokio::test] tears
/// its runtime (and every task on it) down when the test returns.
pub async fn start_test_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test listener");
    let port = listener.local_addr().unwrap().port();

    let config = Arc::new(Config {
        listen: format!("127.0.0.1:{}", port),
        // Deliberately unroutable - nothing is meant to be listening
        // here. Tests that need a real upstream response should not
        // rely on this default.
        proxy_upstream: "127.0.0.1:1".to_string(),
    });

    let client = build_client();

    tokio::spawn(serve_forever(listener, config, client));

    port
}

pub struct RawResponse {
    pub status: u16,
    pub headers: String,
    pub body: Vec<u8>,
}

/// Sends a raw HTTP/1.1 request over a fresh TCP connection and
/// parses just enough of the response to be useful in tests - status
/// code, header block, body bytes. Deliberately not a full HTTP
/// client crate: ferrox stays dependency-light, and testing it
/// shouldn't require pulling one in just to talk to it.
pub async fn raw_request(
    port: u16,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
) -> RawResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("failed to connect to test server");

    let mut request = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        method, path, port
    );
    for (k, v) in extra_headers {
        request.push_str(&format!("{}: {}\r\n", k, v));
    }
    request.push_str("\r\n");

    stream.write_all(request.as_bytes()).await.unwrap();

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();

    let split_at = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);

    let (header_bytes, body) = match split_at {
        Some(i) => (&raw[..i - 4], raw[i..].to_vec()),
        None => (&raw[..], Vec::new()),
    };

    let headers = String::from_utf8_lossy(header_bytes).to_string();
    let status = headers
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    RawResponse {
        status,
        headers,
        body,
    }
}
