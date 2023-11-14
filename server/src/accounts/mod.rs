pub mod configuration;
pub mod db_auth;
pub mod db_policy;
pub mod ldap;
pub mod login;
pub mod policy;

use crate::accounts::db_policy::DatabaseBackedPolicy;
use crate::accounts::ldap::LightweightDirectory;
use crate::accounts::login::{Login, LoginRequest, LoginResponse, ServerLogin};
use crate::accounts::policy::{Policy, PolicyRequest};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{body::Incoming, http, Request};
use spadina_core::net::server::auth::AuthScheme;
use spadina_core::UpdateResult;
use std::future::Future;

/// The result of an attempt at authentication
pub enum AuthResult {
  /// The user should be denied access
  Failure,
  /// The user should be granted access by sending a JWT as a response
  SendToken(String),
  RedirectToken(String),
  /// Send an arbitrary HTTP response to the client
  Page(Result<http::Response<Full<Bytes>>, http::Error>),
  /// The URL requested is not handled by this authentication provider
  NotHandled,
}

pub enum ServerAccounts {
  Login(ServerLogin, DatabaseBackedPolicy),
  LDAP(LightweightDirectory),
}

impl Login for ServerAccounts {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move {
      match self {
        ServerAccounts::Login(l, _) => l.administration_request(request).await,
        ServerAccounts::LDAP(l) => l.administration_request(request).await,
      }
    }
  }

  fn http_handle(self: &Self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      match self {
        ServerAccounts::Login(l, _) => l.http_handle(req).await,
        ServerAccounts::LDAP(l) => l.http_handle(req).await,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move {
      match self {
        ServerAccounts::Login(l, _) => l.normalize_username(player).await,
        ServerAccounts::LDAP(l) => l.normalize_username(player).await,
      }
    }
  }

  fn scheme(&self) -> AuthScheme {
    match self {
      ServerAccounts::Login(l, _) => l.scheme(),
      ServerAccounts::LDAP(l) => l.scheme(),
    }
  }
}

impl Policy for ServerAccounts {
  fn can_create(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move {
      match self {
        ServerAccounts::Login(_, p) => p.can_create(player).await,
        ServerAccounts::LDAP(p) => p.can_create(player).await,
      }
    }
  }

  fn is_administrator(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move {
      match self {
        ServerAccounts::Login(_, p) => p.is_administrator(player).await,
        ServerAccounts::LDAP(p) => p.is_administrator(player).await,
      }
    }
  }

  fn request(&self, request: PolicyRequest) -> impl Future<Output = UpdateResult> + Send {
    async move {
      match self {
        ServerAccounts::Login(_, p) => p.request(request).await,
        ServerAccounts::LDAP(p) => p.request(request).await,
      }
    }
  }
}
