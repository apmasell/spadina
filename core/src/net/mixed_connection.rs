use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::UnixStream;

#[derive(Debug)]
#[pin_project::pin_project(project = MixedConnectionProjection)]
pub enum MixedConnection {
  Upgraded(#[pin] TokioIo<Upgraded>),
  Unix(#[pin] UnixStream),
}

impl AsyncRead for MixedConnection {
  fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
    match self.project() {
      MixedConnectionProjection::Upgraded(u) => u.poll_read(cx, buf),
      MixedConnectionProjection::Unix(u) => u.poll_read(cx, buf),
    }
  }
}

impl tokio::io::AsyncWrite for MixedConnection {
  fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    match self.project() {
      MixedConnectionProjection::Upgraded(u) => u.poll_write(cx, buf),
      MixedConnectionProjection::Unix(u) => u.poll_write(cx, buf),
    }
  }

  fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    match self.project() {
      MixedConnectionProjection::Upgraded(u) => u.poll_flush(cx),
      MixedConnectionProjection::Unix(u) => u.poll_flush(cx),
    }
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
    match self.project() {
      MixedConnectionProjection::Upgraded(u) => u.poll_shutdown(cx),
      MixedConnectionProjection::Unix(u) => u.poll_shutdown(cx),
    }
  }
}

impl From<Upgraded> for MixedConnection {
  fn from(value: Upgraded) -> Self {
    MixedConnection::Upgraded(TokioIo::new(value))
  }
}

impl From<UnixStream> for MixedConnection {
  fn from(value: UnixStream) -> Self {
    MixedConnection::Unix(value)
  }
}
