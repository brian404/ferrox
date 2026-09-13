use anyhow::{Context, Result};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

/// Builds a TLS acceptor from a PEM certificate chain and private key
/// on disk. Called once at startup, not per-connection - the
/// resulting acceptor is cheap to clone (internally Arc'd) and shared
/// across every accepted connection.
pub fn build_acceptor(cert_path: &str, key_path: &str) -> Result<TlsAcceptor> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("invalid TLS certificate/key pair")?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path).with_context(|| format!("failed to open cert file '{}'", path))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse certificates in '{}'", path))
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path).with_context(|| format!("failed to open key file '{}'", path))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .with_context(|| format!("failed to parse private key in '{}'", path))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in '{}'", path))
}
