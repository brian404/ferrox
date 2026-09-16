use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

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

pub fn get(path: &PathBuf, mtime: SystemTime) -> Option<(Bytes, String, &'static str)> {
    let guard = cache().read().unwrap();
    match guard.get(path) {
        Some(entry) if entry.mtime == mtime => {
            crate::metrics::record_cache_hit();
            Some((
                entry.contents.clone(),
                entry.content_type.clone(),
                entry.cache_control,
            ))
        }
        _ => {
            crate::metrics::record_cache_miss();
            None
        }
    }
}

pub fn insert(path: PathBuf, entry: CachedFile) {
    cache().write().unwrap().insert(path, entry);
}
