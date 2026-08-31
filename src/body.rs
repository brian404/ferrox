use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Frame};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, ReadBuf};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type ResponseBody = BoxBody<Bytes, BoxError>;

pub fn full_body(bytes: impl Into<Bytes>) -> ResponseBody {
    Full::from(bytes.into())
        .map_err(|never: std::convert::Infallible| match never {})
        .boxed()
}

// Streams a file straight from disk in fixed-size chunks instead of
// reading it fully into memory first - memory use per request stays
// constant regardless of file size.
pub fn file_body(file: File) -> ResponseBody {
    FileBody {
        file,
        buf: vec![0u8; 64 * 1024],
    }
    .map_err(BoxError::from)
    .boxed()
}

struct FileBody {
    file: File,
    buf: Vec<u8>,
}

impl Body for FileBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let mut read_buf = ReadBuf::new(&mut this.buf);
        match Pin::new(&mut this.file).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n == 0 {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(Frame::data(Bytes::copy_from_slice(&this.buf[..n])))))
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
