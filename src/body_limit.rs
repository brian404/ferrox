use bytes::Bytes;
use hyper::body::{Body, Frame, Incoming};
use hyper::HeaderMap;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Requests larger than this are rejected before being forwarded
/// upstream - protects both ferrox and the backend from unbounded
/// request bodies. Chosen as a reasonable default for typical JSON/
/// form API payloads; raise it if the backend genuinely needs larger
/// uploads.
pub const MAX_REQUEST_BODY_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

#[derive(Debug)]
pub struct BodyTooLarge;

impl std::fmt::Display for BodyTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "request body exceeded the {} byte limit",
            MAX_REQUEST_BODY_SIZE
        )
    }
}

impl std::error::Error for BodyTooLarge {}

/// Wraps an incoming request body and enforces MAX_REQUEST_BODY_SIZE
/// as it's actually read - the defense-in-depth layer for bodies with
/// no Content-Length (chunked transfer-encoding) or one that
/// understates the real size. The common case - an honestly-declared
/// oversized Content-Length - is rejected earlier, before any of the
/// body is read at all, via `content_length_exceeds_limit`.
pub struct LimitedBody {
    inner: Incoming,
    seen: u64,
}

impl LimitedBody {
    pub fn new(inner: Incoming) -> Self {
        Self { inner, seen: 0 }
    }
}

impl Body for LimitedBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.seen += data.len() as u64;
                    if this.seen > MAX_REQUEST_BODY_SIZE {
                        return Poll::Ready(Some(Err(Box::new(BodyTooLarge))));
                    }
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(Box::new(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Checks a request's declared Content-Length against the limit
/// without reading any of the body. Returns true if the request
/// should be rejected outright.
pub fn content_length_exceeds_limit(headers: &HeaderMap) -> bool {
    headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|len| len > MAX_REQUEST_BODY_SIZE)
        .unwrap_or(false)
}
