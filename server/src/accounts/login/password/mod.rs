pub mod db_otp;
pub mod fixed_otp;
pub mod fixed_password;
pub mod otp;
pub mod php_bb;
pub mod uru;

use crate::accounts::login::{Login, LoginRequest, LoginResponse};
use crate::accounts::AuthResult;
use http::{Method, Response, StatusCode};
use hyper::{body::Incoming, http, Request};
use spadina_core::net::server::auth::{AuthScheme, PasswordRequest};
use spadina_core::net::server::PASSWORD_AUTH_PATH;
use std::future::Future;

pub enum ServerPassword {
  DatabaseOneTimePassword(db_otp::DatabaseOneTimePasswords),
  FixedOneTimePassword(fixed_otp::FixedOneTimePassword),
  FixedPassword(fixed_password::FixedPasswords),
  PhpBB(php_bb::PhpBB),
  Uru(uru::UruDatabase),
}

pub trait Password: Send + Sync {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send;
  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send;
  fn validate(&self, username: String, password: String) -> impl Future<Output = Option<String>> + Send;
}

impl<T> Login for T
where
  T: Password,
{
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move {
      match request {
        LoginRequest::LockAccount(account, locked) => LoginResponse::LockAccount(self.lock_account(account, locked).await),
        LoginRequest::Invite => LoginResponse::Invite(None),
      }
    }
  }

  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      match (req.method(), req.uri().path()) {
        (&Method::POST, PASSWORD_AUTH_PATH) => match crate::http_server::aggregate::<PasswordRequest<String>>(req).await {
          Err(response) => AuthResult::Page(response),
          Ok(request) => match self.validate(request.username, request.password).await {
            Some(username) => AuthResult::SendToken(username),
            None => AuthResult::Page(Response::builder().status(StatusCode::UNAUTHORIZED).body("Invalid username or password".into())),
          },
        },
        _ => AuthResult::NotHandled,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move { self.check_and_normalize(player).await.ok_or(()) }
  }

  fn scheme(self: &Self) -> AuthScheme {
    AuthScheme::Password
  }
}

impl Password for ServerPassword {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      match self {
        ServerPassword::DatabaseOneTimePassword(p) => p.check_and_normalize(username).await,
        ServerPassword::FixedOneTimePassword(p) => p.check_and_normalize(username).await,
        ServerPassword::FixedPassword(p) => p.check_and_normalize(username).await,
        ServerPassword::PhpBB(p) => p.check_and_normalize(username).await,
        ServerPassword::Uru(p) => p.check_and_normalize(username).await,
      }
    }
  }

  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move {
      match self {
        ServerPassword::DatabaseOneTimePassword(p) => p.lock_account(username, locked).await,
        ServerPassword::FixedOneTimePassword(p) => p.lock_account(username, locked).await,
        ServerPassword::FixedPassword(p) => p.lock_account(username, locked).await,
        ServerPassword::PhpBB(p) => p.lock_account(username, locked).await,
        ServerPassword::Uru(p) => p.lock_account(username, locked).await,
      }
    }
  }

  fn validate(&self, username: String, password: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      match self {
        ServerPassword::DatabaseOneTimePassword(p) => p.validate(username, password).await,
        ServerPassword::FixedOneTimePassword(p) => p.validate(username, password).await,
        ServerPassword::FixedPassword(p) => p.validate(username, password).await,
        ServerPassword::PhpBB(p) => p.validate(username, password).await,
        ServerPassword::Uru(p) => p.validate(username, password).await,
      }
    }
  }
}
