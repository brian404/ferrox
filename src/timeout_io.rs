use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{sleep, Instant, Sleep};

/// Wraps a stream and enforces a read-stall timeout: if no bytes are
/// successfully read within `timeout` of the last successful read (or
/// connection start), the read errors out with TimedOut instead of
/// hanging forever. Writes are never delayed by this.
///
/// This targets Slowloris-style attacks specifically - a client that
/// opens a connection and trickles in a byte every so often to tie up
/// a connection slot indefinitely. It resets on ANY progress, so a
/// genuinely slow-but-active client (a real upload on a bad
/// connection, a WebSocket sending periodic frames) is unaffected;
/// only a connection making essentially no progress at all gets cut.
/// This is a first-layer mitigation, not a complete one - a patient
/// attacker sending exactly one byte just under the timeout interval
/// forever would still evade it; a full defense would also enforce a
/// minimum throughput, not just "any progress at all".
pub struct ReadTimeoutStream<S> {
    inner: S,
    timeout: Duration,
    sleep: Pin<Box<Sleep>>,
}

impl<S> ReadTimeoutStream<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            sleep: Box::pin(sleep(timeout)),
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ReadTimeoutStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                this.sleep.as_mut().reset(Instant::now() + this.timeout);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => match this.sleep.as_mut().poll(cx) {
                Poll::Ready(()) => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "connection read timed out (no data received in time)",
                ))),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ReadTimeoutStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
