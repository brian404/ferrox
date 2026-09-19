use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

pub const MIN_COMPRESS_SIZE: usize = 256;

pub fn is_compressible(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.starts_with("application/javascript")
        || content_type.starts_with("application/json")
        || content_type.starts_with("application/xml")
        || content_type == "image/svg+xml"
}

pub fn accepts_gzip(accept_encoding: Option<&str>) -> bool {
    accept_encoding
        .map(|v| {
            v.split(',')
                .any(|t| t.trim().split(';').next().unwrap_or("").eq_ignore_ascii_case("gzip"))
        })
        .unwrap_or(false)
}

pub fn gzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}
