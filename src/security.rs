use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

static PUBLIC_DIR_PATH: OnceLock<String> = OnceLock::new();

pub fn public_dir() -> &'static str {
    PUBLIC_DIR_PATH.get_or_init(|| {
        std::env::var("FERROX_PUBLIC_DIR").unwrap_or_else(|_| "public".to_string())
    })
}

static PUBLIC_ROOT: OnceLock<PathBuf> = OnceLock::new();

fn public_root() -> &'static PathBuf {
    PUBLIC_ROOT.get_or_init(|| {
        std::fs::canonicalize(public_dir())
            .unwrap_or_else(|e| panic!("cannot resolve public dir '{}': {}", public_dir(), e))
    })
}

pub fn contains_dotdot(path: &str) -> bool {
    path.split('/').any(|segment| segment == "..")
}

// Resolves `candidate` and confirms it lives inside the public root
// after following symlinks, without reading its contents. This is the
// actual traversal guard, and it runs on every request regardless of
// cache state - the cache only skips the byte-for-byte disk read.
pub async fn resolve_within_public(candidate: &Path) -> Option<(PathBuf, SystemTime, u64)> {
    let root = public_root();

    let resolved = tokio::fs::canonicalize(candidate).await.ok()?;
    if !resolved.starts_with(root) {
        tracing::warn!(
            "Blocked request resolving outside public dir: {} -> {}",
            candidate.display(),
            resolved.display()
        );
        return None;
    }

    let metadata = tokio::fs::metadata(&resolved).await.ok()?;
    let mtime = metadata.modified().ok()?;
    Some((resolved, mtime, metadata.len()))
}
