use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

pub mod body;
pub mod body_limit;
pub mod cache;
pub mod config;
pub mod handlers;
pub mod headers;
pub mod range;
pub mod router;
pub mod security;
pub mod timeout_io;

pub use config::Config;
pub use handlers::{build_client, ProxyClient};
pub use timeout_io::ReadTimeoutStream;

use crate::router::route;

pub const SLOWLORIS_TIMEOUT: Duration = Duration::from_secs(30);

/// Handles a single accepted TCP connection end to end: wraps it with
/// the Slowloris read-timeout, drives the HTTP/1.1 connection (with
/// upgrade support for WebSockets), and dispatches requests through
/// the router. Shared by the real server's accept loop (main.rs) and
/// by tests that want to exercise the exact same connection path.
pub async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    config: Arc<Config>,
    client: ProxyClient,
) {
    let stream = ReadTimeoutStream::new(stream, SLOWLORIS_TIMEOUT);
    let io = TokioIo::new(stream);

    let service = service_fn(move |req| route(req, Arc::clone(&config), peer, client.clone()));

    if let Err(err) = http1::Builder::new()
        .serve_connection(io, service)
        .with_upgrades()
        .await
    {
        error!("Connection error: {}", err);
    }
}

/// A minimal accept loop with no signal handling or graceful drain -
/// intended for tests, which spawn this once and let it end naturally
/// when the test's runtime tears down. The real binary implements its
/// own loop instead, since process lifecycle (SIGTERM, graceful
/// shutdown) is a binary concern, not a library one.
pub async fn serve_forever(listener: TcpListener, config: Arc<Config>, client: ProxyClient) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                info!("New connection from {}", peer);
                let config = Arc::clone(&config);
                let client = client.clone();
                tokio::spawn(handle_connection(stream, peer, config, client));
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}
