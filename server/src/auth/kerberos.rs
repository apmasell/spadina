struct KerberosAuth(String);

#[cfg(feature = "kerberos")]
#[async_trait::async_trait]
impl super::AuthProvider for KerberosAuth {
  fn scheme(self: &Self) -> puzzleverse_core::AuthScheme {
    puzzleverse_core::AuthScheme::Kerberos
  }
  async fn invite(&self, _: &str) -> Option<String> {
    None
  }
  async fn is_locked(&self, _: &str) -> puzzleverse_core::AccountLockState {
    puzzleverse_core::AccountLockState::Unknown
  }
  async fn lock(&self, _: &str, _: bool) -> bool {
    false
  }
  async fn handle(&self, req: http::Request<hyper::Body>) -> super::AuthResult {
    use bytes::Buf;
    use cross_krb5::K5ServerCtx;
    match (req.method(), req.uri().path()) {
      (&http::Method::GET, puzzleverse_core::net::KERBEROS_AUTH_PATH) => {
        super::AuthResult::Page(http::Response::builder().status(http::StatusCode::OK).body(self.0.clone().into()))
      }
      (&http::Method::POST, puzzleverse_core::net::KERBEROS_AUTH_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          eprintln!("Failed to aggregate body: {}", e);
          super::AuthResult::Failure
        }
        Ok(whole_body) => match cross_krb5::ServerCtx::new(cross_krb5::AcceptFlags::empty(), Some(&self.0)).map(|c| c.step(whole_body.chunk())) {
          Err(e) | Ok(Err(e)) => {
            eprintln!("Kerberos error: {}", e);
            super::AuthResult::Failure
          }
          Ok(Ok(cross_krb5::Step::Continue(_))) => {
            eprintln!("Kerberos needs more data?");
            super::AuthResult::Failure
          }
          Ok(Ok(cross_krb5::Step::Finished((mut context, _)))) => match context.client() {
            Ok(mut principal) => match principal.find('@') {
              Some(pos) => {
                principal.truncate(pos);
                super::AuthResult::SendToken(principal)
              }
              None => {
                eprintln!("Kerberos principal is malformed: {}", &principal);
                super::AuthResult::Failure
              }
            },
            Err(e) => {
              eprintln!("Kerberos error: {}", e);
              super::AuthResult::Failure
            }
          },
        },
      },
      _ => super::AuthResult::NotHandled,
    }
  }
}

#[cfg(feature = "kerberos")]
pub fn new(principal: String) -> std::sync::Arc<dyn super::AuthProvider> {
  std::sync::Arc::new(KerberosAuth(principal))
}
