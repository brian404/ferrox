use anyhow::Result;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

use ferrox::{build_client, config, handle_connection, TlsMode, SLOWLORIS_TIMEOUT};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Ferrox starting up...");
ferrox::metrics::init();

    let public_dir = std::env::var("FERROX_PUBLIC_DIR").unwrap_or_else(|_| "public".to_string());
    if !std::path::Path::new(&public_dir).is_dir() {
        anyhow::bail!(
            "public dir '{}' not found in current working directory ({}). \
             Run ferrox from the project root, or set FERROX_PUBLIC_DIR.",
            public_dir,
            std::env::current_dir()?.display()
        );
    }

    let config = Arc::new(config::load()?);
    let client = build_client();

    #[cfg(feature = "tls")]
    let tls_mode = match (&config.tls_cert, &config.tls_key) {
        (Some(cert), Some(key)) => {
            info!("TLS enabled - cert: {}, key: {}", cert, key);
            TlsMode::Tls(ferrox::tls::build_acceptor(cert, key)?)
        }
        _ => TlsMode::Plain,
    };
    #[cfg(not(feature = "tls"))]
    let tls_mode = TlsMode::Plain;

    let addr: SocketAddr = config.listen.parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on http://{}", addr);
    info!("Proxying /api to {}", config.proxy_upstream);

    let active_connections = Arc::new(AtomicUsize::new(0));

    let mut sigterm = signal(SignalKind::terminate())?;

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received SIGINT - shutting down gracefully");
                break;
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM - shutting down gracefully");
                break;
            }

            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer)) => {
                        info!("New connection from {}", peer);
                        let config = Arc::clone(&config);
                        let client = client.clone();
                        let tls_mode = tls_mode.clone();
                        let active_connections = Arc::clone(&active_connections);
                        active_connections.fetch_add(1, Ordering::SeqCst);

                        tokio::spawn(async move {
                            handle_connection(stream, peer, config, client, SLOWLORIS_TIMEOUT, tls_mode).await;
                            active_connections.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
        }
    }

    let drain_timeout = Duration::from_secs(10);
    let start = Instant::now();
    while active_connections.load(Ordering::SeqCst) > 0 {
        if start.elapsed() > drain_timeout {
            warn!(
                "Timed out after {:?} waiting for {} in-flight connection(s); shutting down anyway",
                drain_timeout,
                active_connections.load(Ordering::SeqCst)
            );
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    info!("Ferrox shutdown complete");
    Ok(())
}
