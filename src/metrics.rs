use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();

static TOTAL_REQUESTS: AtomicU64 = AtomicU64::new(0);
static TOTAL_BYTES_OUT: AtomicU64 = AtomicU64::new(0);
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

static STATUS_COUNTS: OnceLock<RwLock<HashMap<u16, u64>>> = OnceLock::new();
static ROUTE_COUNTS: OnceLock<RwLock<HashMap<&'static str, u64>>> = OnceLock::new();

fn status_counts() -> &'static RwLock<HashMap<u16, u64>> {
    STATUS_COUNTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn route_counts() -> &'static RwLock<HashMap<&'static str, u64>> {
    ROUTE_COUNTS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Marks the true process start time for uptime reporting. Called
/// once at startup in main.rs - without this, uptime would instead
/// start counting from whenever the first request happened to arrive.
pub fn init() {
    START_TIME.get_or_init(Instant::now);
}

pub fn record_request(route: &'static str, status: u16, response_bytes: u64) {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
    TOTAL_BYTES_OUT.fetch_add(response_bytes, Ordering::Relaxed);

    *status_counts().write().unwrap().entry(status).or_insert(0) += 1;
    *route_counts().write().unwrap().entry(route).or_insert(0) += 1;
}

pub fn record_cache_hit() {
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

pub fn record_cache_miss() {
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
}

/// Renders all current metrics in standard Prometheus text exposition
/// format - plain text, no external formatting crate needed for
/// something this simple.
pub fn render_prometheus() -> String {
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let total = TOTAL_REQUESTS.load(Ordering::Relaxed);
    let bytes_out = TOTAL_BYTES_OUT.load(Ordering::Relaxed);
    let cache_hits = CACHE_HITS.load(Ordering::Relaxed);
    let cache_misses = CACHE_MISSES.load(Ordering::Relaxed);

    let mut out = String::new();

    out.push_str("# HELP ferrox_uptime_seconds Time since the server started\n");
    out.push_str("# TYPE ferrox_uptime_seconds gauge\n");
    out.push_str(&format!("ferrox_uptime_seconds {}\n\n", uptime));

    out.push_str("# HELP ferrox_requests_total Total number of requests handled\n");
    out.push_str("# TYPE ferrox_requests_total counter\n");
    out.push_str(&format!("ferrox_requests_total {}\n\n", total));

    out.push_str("# HELP ferrox_response_bytes_total Total bytes sent in response bodies\n");
    out.push_str("# TYPE ferrox_response_bytes_total counter\n");
    out.push_str(&format!("ferrox_response_bytes_total {}\n\n", bytes_out));

    out.push_str("# HELP ferrox_requests_by_status_total Requests grouped by HTTP status code\n");
    out.push_str("# TYPE ferrox_requests_by_status_total counter\n");
    {
        let counts = status_counts().read().unwrap();
        let mut entries: Vec<_> = counts.iter().collect();
        entries.sort_by_key(|(status, _)| **status);
        for (status, count) in entries {
            out.push_str(&format!(
                "ferrox_requests_by_status_total{{status=\"{}\"}} {}\n",
                status, count
            ));
        }
    }
    out.push('\n');

    out.push_str("# HELP ferrox_requests_by_route_total Requests grouped by route\n");
    out.push_str("# TYPE ferrox_requests_by_route_total counter\n");
    {
        let counts = route_counts().read().unwrap();
        let mut entries: Vec<_> = counts.iter().collect();
        entries.sort_by_key(|(route, _)| *route);
        for (route, count) in entries {
            out.push_str(&format!(
                "ferrox_requests_by_route_total{{route=\"{}\"}} {}\n",
                route, count
            ));
        }
    }
    out.push('\n');

    out.push_str("# HELP ferrox_cache_hits_total Static file cache hits\n");
    out.push_str("# TYPE ferrox_cache_hits_total counter\n");
    out.push_str(&format!("ferrox_cache_hits_total {}\n\n", cache_hits));

    out.push_str("# HELP ferrox_cache_misses_total Static file cache misses\n");
    out.push_str("# TYPE ferrox_cache_misses_total counter\n");
    out.push_str(&format!("ferrox_cache_misses_total {}\n", cache_misses));

    out
}
