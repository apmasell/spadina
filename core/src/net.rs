pub const AUTH_METHOD_PATH: &str = "/api/auth/method";
pub const CLIENT_KEY_PATH: &str = "/api/client/key";
pub const CLIENT_NONCE_PATH: &str = "/api/client/nonce";
pub const CLIENT_V1_PATH: &str = "/api/client/v1";
pub const KERBEROS_AUTH_PATH: &str = "/api/auth/kerberos";
pub const KERBEROS_PRINCIPAL_PATH: &str = "/api/auth/kerberos-principal";
pub const OIDC_AUTH_FINISH_PATH: &str = "/api/auth/oidc/finish";
pub const OIDC_AUTH_START_PATH: &str = "/api/auth/oidc/start";
pub const PASSWORD_AUTH_PATH: &str = "/api/auth/password";

#[pin_project::pin_project(project = IncomingConnectionProjection)]
pub enum IncomingConnection {
  Upgraded(#[pin] hyper::upgrade::Upgraded),
  Unix(#[pin] tokio::net::UnixStream),
}
impl tokio::io::AsyncRead for IncomingConnection {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_read(cx, buf),
      IncomingConnectionProjection::Unix(u) => u.poll_read(cx, buf),
    }
  }
}
impl tokio::io::AsyncWrite for IncomingConnection {
  fn poll_write(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_write(cx, buf),
      IncomingConnectionProjection::Unix(u) => u.poll_write(cx, buf),
    }
  }

  fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_flush(cx),
      IncomingConnectionProjection::Unix(u) => u.poll_flush(cx),
    }
  }

  fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_shutdown(cx),
      IncomingConnectionProjection::Unix(u) => u.poll_shutdown(cx),
    }
  }
}
impl From<hyper::upgrade::Upgraded> for IncomingConnection {
  fn from(value: hyper::upgrade::Upgraded) -> Self {
    IncomingConnection::Upgraded(value)
  }
}
impl From<tokio::net::UnixStream> for IncomingConnection {
  fn from(value: tokio::net::UnixStream) -> Self {
    IncomingConnection::Unix(value)
  }
}
