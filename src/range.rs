/// Parses a `Range: bytes=...` header against a known file size. Only
/// single ranges are supported (covers the overwhelming majority of
/// real-world Range use - video/audio seeking, download managers,
/// `curl -C`). A multi-range request, or no Range header at all,
/// falls back to serving the full file with 200 - a spec-valid
/// response when a server doesn't support partial content for a
/// given request.
pub enum RangeOutcome {
    Full,
    Satisfiable(u64, u64),
    Unsatisfiable,
}

pub fn parse_range(header: Option<&str>, file_size: u64) -> RangeOutcome {
    let Some(header) = header else {
        return RangeOutcome::Full;
    };

    let Some(spec) = header.strip_prefix("bytes=") else {
        return RangeOutcome::Full;
    };

    if spec.contains(',') {
        return RangeOutcome::Full;
    }

    let spec = spec.trim();

    if file_size == 0 {
        return RangeOutcome::Unsatisfiable;
    }

    if let Some(suffix_len) = spec.strip_prefix('-') {
        let Ok(n) = suffix_len.parse::<u64>() else {
            return RangeOutcome::Full;
        };
        if n == 0 {
            return RangeOutcome::Unsatisfiable;
        }
        let start = file_size.saturating_sub(n);
        return RangeOutcome::Satisfiable(start, file_size - 1);
    }

    let mut parts = spec.splitn(2, '-');
    let start_str = parts.next().unwrap_or("");
    let end_str = parts.next().unwrap_or("");

    let Ok(start) = start_str.parse::<u64>() else {
        return RangeOutcome::Full;
    };

    if start >= file_size {
        return RangeOutcome::Unsatisfiable;
    }

    if end_str.is_empty() {
        return RangeOutcome::Satisfiable(start, file_size - 1);
    }

    let Ok(end) = end_str.parse::<u64>() else {
        return RangeOutcome::Full;
    };

    if end < start {
        return RangeOutcome::Unsatisfiable;
    }

    let end = std::cmp::min(end, file_size - 1);
    RangeOutcome::Satisfiable(start, end)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_range_header_returns_full() {
        assert!(matches!(parse_range(None, 1000), RangeOutcome::Full));
    }

    #[test]
    fn non_bytes_unit_returns_full() {
        assert!(matches!(parse_range(Some("items=0-10"), 1000), RangeOutcome::Full));
    }

    #[test]
    fn simple_start_end_range() {
        match parse_range(Some("bytes=0-99"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 0);
                assert_eq!(end, 99);
            }
            _ => panic!("expected Satisfiable"),
        }
    }

    #[test]
    fn open_ended_range() {
        match parse_range(Some("bytes=500-"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 500);
                assert_eq!(end, 999);
            }
            _ => panic!("expected Satisfiable"),
        }
    }

    #[test]
    fn suffix_range() {
        match parse_range(Some("bytes=-100"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 900);
                assert_eq!(end, 999);
            }
            _ => panic!("expected Satisfiable"),
        }
    }

    #[test]
    fn suffix_range_larger_than_file_clamps_to_start() {
        match parse_range(Some("bytes=-5000"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 0);
                assert_eq!(end, 999);
            }
            _ => panic!("expected Satisfiable"),
        }
    }

    #[test]
    fn suffix_range_of_zero_is_unsatisfiable() {
        assert!(matches!(parse_range(Some("bytes=-0"), 1000), RangeOutcome::Unsatisfiable));
    }

    #[test]
    fn end_beyond_file_size_clamps() {
        match parse_range(Some("bytes=0-999999"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 0);
                assert_eq!(end, 999);
            }
            _ => panic!("expected Satisfiable"),
        }
    }

    #[test]
    fn start_beyond_file_size_is_unsatisfiable() {
        assert!(matches!(parse_range(Some("bytes=1000-"), 1000), RangeOutcome::Unsatisfiable));
        assert!(matches!(parse_range(Some("bytes=5000-6000"), 1000), RangeOutcome::Unsatisfiable));
    }

    #[test]
    fn end_before_start_is_unsatisfiable() {
        assert!(matches!(parse_range(Some("bytes=500-100"), 1000), RangeOutcome::Unsatisfiable));
    }

    #[test]
    fn multi_range_falls_back_to_full() {
        assert!(matches!(parse_range(Some("bytes=0-99,200-299"), 1000), RangeOutcome::Full));
    }

    #[test]
    fn malformed_range_falls_back_to_full() {
        assert!(matches!(parse_range(Some("bytes=abc-def"), 1000), RangeOutcome::Full));
    }

    #[test]
    fn empty_file_with_range_is_unsatisfiable() {
        assert!(matches!(parse_range(Some("bytes=0-10"), 0), RangeOutcome::Unsatisfiable));
    }

    #[test]
    fn empty_file_without_range_is_full() {
        assert!(matches!(parse_range(None, 0), RangeOutcome::Full));
    }

    #[test]
    fn last_byte_only() {
        match parse_range(Some("bytes=999-999"), 1000) {
            RangeOutcome::Satisfiable(start, end) => {
                assert_eq!(start, 999);
                assert_eq!(end, 999);
            }
            _ => panic!("expected Satisfiable"),
        }
    }
}
