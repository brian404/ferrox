use crc32fast::Hasher;

/// Computes a strong ETag from file contents - a CRC32 hash formatted
/// as a quoted hex string per RFC 7232. Only used on the cached path
/// (content already in memory); streamed large files never compute
/// one, since that would mean reading the whole file up front just
/// to hash it, defeating the point of streaming.
pub fn compute(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    format!("\"{:08x}\"", hasher.finalize())
}

/// Checks an `If-None-Match` header value against a computed etag -
/// exact match, or a bare `*` matching anything.
pub fn if_none_match_matches(header_value: &str, etag: &str) -> bool {
    if header_value.trim() == "*" {
        return true;
    }
    header_value
        .split(',')
        .map(|s| s.trim())
        .any(|candidate| candidate == etag)
}
