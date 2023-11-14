use crate::accounts::login::openid::db_oidc::DatabaseOpenIdConnect;
use crate::accounts::login::{Login, LoginRequest, LoginResponse};
use crate::accounts::AuthResult;
use chrono::{DateTime, Duration, Utc};
use http::{Method, Response};
use hyper::header::LOCATION;
use hyper::{body::Incoming, http, Request};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient};
use openidconnect::{AuthorizationCode, TokenResponse};
use openidconnect::{CsrfToken, Nonce, PkceCodeChallenge, PkceCodeVerifier, Scope};
use spadina_core::net::server::auth::AuthScheme;
use spadina_core::net::OIDC_AUTH_START_PATH;
use std::collections::BTreeMap;
use std::future::Future;
use tokio::sync::Mutex;

pub mod configuration;
pub mod db_oidc;
pub mod net;
pub const OIDC_AUTH_RETURN_PATH: &str = "/api/auth/oidc/finish";

pub trait OpenIdConnectProvider: Send + Sync {
  type Callback: Send + Sync;
  /// Check if the username and password provided are valid
  fn client_for(&self, username: &str) -> impl Future<Output = Option<&CoreClient>> + Send;
  fn client_for_active<'a>(&'a self, request: &Self::Callback) -> impl Future<Output = Option<&'a CoreClient>> + Send;
  fn start_login(&self, player: String) -> impl Future<Output = Self::Callback> + Send;
  fn finish_login(&self, callback: Self::Callback, subject: &str) -> impl Future<Output = AuthResult> + Send;
}

pub struct OpenIdConnect<Provider: OpenIdConnectProvider> {
  provider: Provider,
  active_requests: Mutex<BTreeMap<String, OpenIdConnectServerState<Provider::Callback>>>,
}
pub struct OpenIdConnectServerState<T> {
  expires: DateTime<Utc>,
  nonce: Nonce,
  verifier: PkceCodeVerifier,
  callback: T,
}
pub enum ServerOpenIdConnect {
  Database(OpenIdConnect<DatabaseOpenIdConnect>),
}

impl<T: OpenIdConnectProvider> From<T> for OpenIdConnect<T> {
  fn from(provider: T) -> Self {
    OpenIdConnect { provider, active_requests: Mutex::new(BTreeMap::new()) }
  }
}

impl<T: OpenIdConnectProvider> Login for OpenIdConnect<T> {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move { todo!() }
  }

  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      fn query_parameter<'a>(name: &str, req: &'a Request<Incoming>) -> Option<std::borrow::Cow<'a, str>> {
        req.uri().query().map(|q| form_urlencoded::parse(q.as_bytes()).filter(|(n, _)| n == name).map(|(_, value)| value).next()).flatten()
      }
      match (req.method(), req.uri().path()) {
        (&Method::GET, OIDC_AUTH_START_PATH) => match query_parameter("player", &req) {
          None => AuthResult::Failure,
          Some(player) => match self.provider.client_for(&*player).await {
            None => AuthResult::Failure,
            Some(client) => {
              let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
              let (auth_url, csrf_token, nonce) = client
                .authorize_url(CoreAuthenticationFlow::AuthorizationCode, CsrfToken::new_random, Nonce::new_random)
                .add_scope(Scope::new("openid".to_string()))
                .add_scope(Scope::new("profile".to_string()))
                .set_pkce_challenge(pkce_challenge)
                .url();
              let now = Utc::now();
              let player = player.into_owned();
              let mut servers = self.active_requests.lock().await;
              servers.retain(|_, s| s.expires >= now);
              servers.insert(
                csrf_token.secret().clone(),
                OpenIdConnectServerState {
                  verifier: pkce_verifier,
                  nonce,
                  expires: now + Duration::minutes(10),
                  callback: self.provider.start_login(player).await,
                },
              );
              AuthResult::Page(
                Response::builder().status(http::StatusCode::TEMPORARY_REDIRECT).header(LOCATION, auth_url.to_string()).body("".into()),
              )
            }
          },
        },
        (&Method::GET, OIDC_AUTH_RETURN_PATH) => {
          let now = Utc::now();
          let (Some(csrf_token), Some(code)) = (query_parameter("state", &req), query_parameter("code", &req)) else {
            return AuthResult::Failure;
          };
          let Some(OpenIdConnectServerState { callback, nonce, verifier, .. }) = ({
            let mut servers = self.active_requests.lock().await;
            servers.retain(|_, p| p.expires >= now);
            servers.remove(&*csrf_token)
          }) else {
            return AuthResult::Failure;
          };
          let Some(client) = self.provider.client_for_active(&callback).await else {
            return AuthResult::Failure;
          };
          let auth = match client
            .exchange_code(AuthorizationCode::new(code.into_owned()))
            .set_pkce_verifier(verifier)
            .request_async(openidconnect::reqwest::async_http_client)
            .await
          {
            Ok(auth) => auth,
            Err(e) => return AuthResult::Page(Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into())),
          };
          match auth.id_token().map(|id| id.claims(&client.id_token_verifier(), &nonce)) {
                Some(Ok(id_token)) =>
                   self.provider.finish_login(callback, &*id_token.subject()) .await,
                Some(Err(e)) =>
                    AuthResult::Page(Response::builder().status(http::StatusCode::FORBIDDEN).body(format!("Failed to validate OpenId Connect claim: {:?}", e).into())),
                None =>
                    AuthResult::Page(Response::builder().status(http::StatusCode::BAD_REQUEST).body("The Spadina server has be connected to a non-OpenID Connect-enable OAuth server. Contact your server administrator. If you are the server administrator, choose a different OpenID server or adjust it to enable OpenID Connect.".into()))
            }
        }

        _ => AuthResult::NotHandled,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move {
      if self.provider.client_for(&player).await.is_some() {
        Ok(player)
      } else {
        Err(())
      }
    }
  }

  fn scheme(self: &Self) -> AuthScheme {
    AuthScheme::OpenIdConnect
  }
}

impl Login for ServerOpenIdConnect {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move {
      match self {
        ServerOpenIdConnect::Database(o) => o.administration_request(request).await,
      }
    }
  }

  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      match self {
        ServerOpenIdConnect::Database(o) => o.http_handle(req).await,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move {
      match self {
        ServerOpenIdConnect::Database(o) => o.normalize_username(player).await,
      }
    }
  }

  fn scheme(&self) -> AuthScheme {
    match self {
      ServerOpenIdConnect::Database(o) => o.scheme(),
    }
  }
}
