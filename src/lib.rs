use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

pub mod etag;
pub mod metrics;
pub mod compression;
pub mod body;
pub mod body_limit;
pub mod cache;
pub mod config;
pub mod handlers;
pub mod headers;
pub mod range;
pub mod router;
pub mod security;
#[cfg(feature = "tls")]
pub mod tls;
pub mod timeout_io;

pub use config::Config;
pub use handlers::{build_client, ProxyClient};
pub use timeout_io::ReadTimeoutStream;

use crate::router::route;

pub const SLOWLORIS_TIMEOUT: Duration = Duration::from_secs(30);

/// How an accepted connection should be finished: plain HTTP, or a
/// TLS handshake first. The `Tls` variant only exists when compiled
/// with the `tls` feature - without it, every call site just uses
/// `TlsMode::Plain`, which always exists regardless of feature state.
#[derive(Clone)]
pub enum TlsMode {
    Plain,
    #[cfg(feature = "tls")]
    Tls(tokio_rustls::TlsAcceptor),
}

/// Handles a single accepted TCP connection end to end: wraps it with
/// a read-stall timeout, optionally performs a TLS handshake (inside
/// that same timeout, so a client that opens a connection and never
/// completes the handshake gets cut the same as any other stalled
/// connection), then drives the HTTP/1.1 connection and dispatches
/// requests through the router.
pub async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    config: Arc<Config>,
    client: ProxyClient,
    read_timeout: Duration,
    tls_mode: TlsMode,
) {
    let stream = ReadTimeoutStream::new(stream, read_timeout);

    match tls_mode {
        TlsMode::Plain => {
            serve(stream, peer, config, client).await;
        }
        #[cfg(feature = "tls")]
        TlsMode::Tls(acceptor) => match acceptor.accept(stream).await {
            Ok(tls_stream) => {
                serve(tls_stream, peer, config, client).await;
            }
            Err(e) => {
                tracing::warn!("TLS handshake failed from {}: {}", peer, e);
            }
        },
    }
}

async fn serve<T>(io: T, peer: SocketAddr, config: Arc<Config>, client: ProxyClient)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(io);
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
pub async fn serve_forever(
    listener: TcpListener,
    config: Arc<Config>,
    client: ProxyClient,
    read_timeout: Duration,
    tls_mode: TlsMode,
) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                info!("New connection from {}", peer);
                let config = Arc::clone(&config);
                let client = client.clone();
                let tls_mode = tls_mode.clone();
                tokio::spawn(handle_connection(
                    stream,
                    peer,
                    config,
                    client,
                    read_timeout,
                    tls_mode,
                ));
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}
