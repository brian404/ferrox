use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use ferrox::{build_client, serve_forever, Config, TlsMode, SLOWLORIS_TIMEOUT};

/// Starts a real ferrox server on an OS-assigned free port with full
/// control over its proxy upstream and read-stall timeout. Always
/// plain HTTP (TlsMode::Plain) - no test exercises TLS itself, since
/// that's the `tls` feature's job and doesn't change routing logic.
pub async fn start_test_server_custom(proxy_upstream: &str, read_timeout: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind test listener");
    let port = listener.local_addr().unwrap().port();

    let config = Arc::new(Config {
        listen: format!("127.0.0.1:{}", port),
        proxy_upstream: proxy_upstream.to_string(),
        tls_cert: None,
        tls_key: None,
    });

    let client = build_client();
    tokio::spawn(serve_forever(
        listener,
        config,
        client,
        read_timeout,
        TlsMode::Plain,
    ));

    port
}

/// The common case for tests that don't care about proxying or the
/// read timeout - deliberately unroutable upstream, real 30s timeout.
pub async fn start_test_server() -> u16 {
    start_test_server_custom("127.0.0.1:1", SLOWLORIS_TIMEOUT).await
}

pub struct RawResponse {
    pub status: u16,
    pub headers: String,
    pub body: Vec<u8>,
}

/// Sends a raw HTTP/1.1 request over a fresh TCP connection and
/// parses just enough of the response to be useful in tests.
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

/// A minimal fake WebSocket-upgrade backend for tests: accepts one
/// connection, reads until the end of the request headers, responds
/// with 101 Switching Protocols (without computing a real, spec-valid
/// Sec-WebSocket-Accept - ferrox's proxy layer never inspects that
/// value either, only the status code, so a real one isn't needed to
/// test the relay), then echoes back whatever raw bytes arrive after
/// the upgrade.
pub async fn start_fake_ws_backend() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let mut total = 0;
            loop {
                let n = stream.read(&mut buf[total..]).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }

            let response = "HTTP/1.1 101 Switching Protocols\r\n\
                             Upgrade: websocket\r\n\
                             Connection: Upgrade\r\n\
                             Sec-WebSocket-Accept: dGVzdA==\r\n\r\n";
            if stream.write_all(response.as_bytes()).await.is_err() {
                return;
            }

            let mut echo_buf = [0u8; 4096];
            loop {
                match stream.read(&mut echo_buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if stream.write_all(&echo_buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    port
}
