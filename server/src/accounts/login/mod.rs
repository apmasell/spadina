pub mod openid;
pub mod password;

use crate::accounts::login::openid::ServerOpenIdConnect;
use crate::accounts::login::password::ServerPassword;
use crate::accounts::AuthResult;
use hyper::{body::Incoming, Request};
use spadina_core::net::server::auth::AuthScheme;
use std::future::Future;

pub trait Login: Send + Sync {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send;
  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send;
  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send;
  fn scheme(&self) -> AuthScheme;
}

pub enum LoginRequest<'a> {
  LockAccount(&'a str, bool),
  Invite,
}

pub enum LoginResponse {
  LockAccount(Option<bool>),
  Invite(Option<String>),
}
pub enum ServerLogin {
  Password(ServerPassword),
  OpenID(ServerOpenIdConnect),
}
impl Login for ServerLogin {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move {
      match self {
        ServerLogin::Password(l) => l.administration_request(request).await,
        ServerLogin::OpenID(l) => l.administration_request(request).await,
      }
    }
  }

  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      match self {
        ServerLogin::Password(l) => l.http_handle(req).await,
        ServerLogin::OpenID(l) => l.http_handle(req).await,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move {
      match self {
        ServerLogin::Password(l) => l.normalize_username(player).await,
        ServerLogin::OpenID(l) => l.normalize_username(player).await,
      }
    }
  }

  fn scheme(&self) -> AuthScheme {
    match self {
      ServerLogin::Password(l) => l.scheme(),
      ServerLogin::OpenID(l) => l.scheme(),
    }
  }
}
