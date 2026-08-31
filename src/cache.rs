use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

// Files larger than this are never cached - this is meant for a
// normal small public/ directory (html/css/js/small images), not a
// general-purpose file server. Without a cap, one big file would sit
// in memory forever with no eviction.
pub const MAX_CACHEABLE_SIZE: u64 = 5 * 1024 * 1024;

pub struct CachedFile {
    pub contents: Bytes,
    pub mtime: SystemTime,
    pub content_type: String,
    pub cache_control: &'static str,
}

static CACHE: OnceLock<RwLock<HashMap<PathBuf, CachedFile>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<PathBuf, CachedFile>> {
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Returns owned copies of the cached fields if present and still
/// fresh (mtime matches what the caller has on disk right now).
/// Deliberately synchronous and returns owned data, not a guard - so
/// callers never risk holding a lock across an .await point.
pub fn get(path: &PathBuf, mtime: SystemTime) -> Option<(Bytes, String, &'static str)> {
    let guard = cache().read().unwrap();
    let entry = guard.get(path)?;
    if entry.mtime == mtime {
        Some((
            entry.contents.clone(),
            entry.content_type.clone(),
            entry.cache_control,
        ))
    } else {
        None
    }
}

pub fn insert(path: PathBuf, entry: CachedFile) {
    cache().write().unwrap().insert(path, entry);
}
