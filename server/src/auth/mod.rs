mod db_oidc;
mod db_oidc_multiple;
mod db_otp;
mod fixed_otp;
mod fixed_password;
mod kerberos;
mod ldap;
mod php_bb;
mod uru;

/// The result of an attempt at authentication
pub enum AuthResult {
  /// The user should be denied access
  Failure,
  /// The user should be granted access by sending a JWT as a response
  SendToken(String),
  /// Send an arbitrary HTTP response to the client
  Page(Result<http::Response<hyper::Body>, http::Error>),
  /// The URL requested is not handled by this authentication provider
  NotHandled,
}

/// A pluggable authentication mechanism
#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
  /// Describe the authentication scheme to the client
  fn scheme(self: &Self) -> puzzleverse_core::AuthScheme;
  /// Create an invitation to this server
  async fn invite(&self, server_name: &str) -> Option<String>;
  /// Check if an account is locked.
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState;
  /// Lock (or unlock) a user account
  async fn lock(&self, username: &str, locked: bool) -> bool;
  /// Handle incoming HTTP requests that might be part of authentication
  async fn handle(self: &Self, req: http::Request<hyper::Body>) -> AuthResult;
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AuthConfiguration {
  DatabaseOTPs {
    connection: String,
  },
  #[cfg(feature = "kerberos")]
  Kerberos {
    principal: String,
  },
  LDAP {
    url: String,
    bind_dn: String,
    bind_pw: String,
    search_base: String,
    account_attr: String,
  },
  MultipleOpenIdConnect {
    providers: Vec<AuthOpenIdConnectConfiguration>,
    db_connection: String,
    registration: OpenIdRegistration,
  },
  OTPs {
    users: std::collections::HashMap<String, String>,
  },
  OpenIdConnect {
    provider: AuthOpenIdConnectEndpoint,
    client_id: String,
    client_secret: String,
    db_connection: String,
    registration: OpenIdRegistration,
  },
  Passwords {
    users: std::collections::HashMap<String, String>,
  },
  PhpBB {
    connection: String,
    database: AuthDatabase,
  },
  Uru {
    connection: String,
  },
}
impl AuthConfiguration {
  /// Parse the configuration string provided to the server into an authentication provider, if possible
  pub async fn load(self, server_name: &str) -> Result<std::sync::Arc<dyn AuthProvider>, String> {
    match self {
      AuthConfiguration::DatabaseOTPs { connection } => crate::auth::db_otp::new(connection),
      AuthConfiguration::LDAP { url, bind_dn, bind_pw, search_base, account_attr } => {
        crate::auth::ldap::new(url, bind_dn, bind_pw, search_base, account_attr).await
      }
      #[cfg(feature = "kerberos")]
      AuthConfiguration::Kerberos { principal } => Ok(kerberos::new(principal)),
      AuthConfiguration::MultipleOpenIdConnect { providers, db_connection, registration } => {
        let mut clients = std::collections::BTreeMap::new();
        for configuration in providers {
          let (client, issuer, name) = create_oidc_client(configuration.provider, configuration.client_id, configuration.client_secret, server_name)?;
          clients.insert(issuer, (name, client));
        }
        db_oidc_multiple::new(db_connection, clients, registration)
      }
      AuthConfiguration::OpenIdConnect { client_id, client_secret, provider, db_connection, registration } => {
        db_oidc::new(db_connection, create_oidc_client(provider, client_id, client_secret, server_name)?.0, registration)
      }
      AuthConfiguration::OTPs { users } => crate::auth::fixed_otp::new(users),
      AuthConfiguration::Passwords { users } => crate::auth::fixed_password::new(users),
      AuthConfiguration::PhpBB { connection, database } => match database {
        AuthDatabase::PostgreSQL => crate::auth::php_bb::new::<diesel::pg::PgConnection, _>(connection),
        #[cfg(feature = "mysql")]
        AuthDatabase::MySQL => crate::auth::php_bb::new::<diesel::mysql::MysqlConnection, _>(connection),
      },
      AuthConfiguration::Uru { connection } => crate::auth::uru::new(connection),
    }
  }
}
#[derive(serde::Serialize, serde::Deserialize)]
pub enum AuthDatabase {
  PostgreSQL,
  #[cfg(feature = "mysql")]
  MySQL,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AuthOpenIdConnectConfiguration {
  provider: AuthOpenIdConnectEndpoint,
  client_id: String,
  client_secret: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AuthOpenIdConnectEndpoint {
  Custom { url: String, name: String },
  Apple,
  Facebook,
  Google,
  LinkedIn,
  Microsoft { tenant: Option<String> },
}

impl AuthOpenIdConnectEndpoint {
  fn into_url_and_name(self) -> (String, String) {
    match self {
      AuthOpenIdConnectEndpoint::Custom { url, name } => (url, name),
      AuthOpenIdConnectEndpoint::Apple => ("https://appleid.apple.com/".to_string(), "Apple".to_string()),
      AuthOpenIdConnectEndpoint::Facebook => ("https://www.facebook.com/".to_string(), "Facebook".to_string()),
      AuthOpenIdConnectEndpoint::Google => ("https://accounts.google.com/".to_string(), "Google".to_string()),
      AuthOpenIdConnectEndpoint::LinkedIn => ("https://www.linkedin.com/".to_string(), "LinkedIn".to_string()),
      AuthOpenIdConnectEndpoint::Microsoft { tenant } => (
        format!(
          "https://login.microsoftonline.com/{}/v2.0/",
          match tenant {
            Some(s) => std::borrow::Cow::Owned(s),
            None => std::borrow::Cow::Borrowed("common"),
          }
        ),
        "Microsoft".to_owned(),
      ),
    }
  }
}

/// Create an authentication provider that deals with unencrypted usernames and passwords
#[async_trait::async_trait]
pub trait Password: Send + Sync {
  /// Check if the username and password provided are valid
  async fn check(self: &Self, username: &str, password: &str) -> bool;
  /// Check if an account is locked.
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState;
  /// Lock (or unlock) a user account
  async fn lock(&self, username: &str, locked: bool) -> bool;
}

#[async_trait::async_trait]
impl<T> AuthProvider for T
where
  T: Password,
{
  fn scheme(self: &Self) -> puzzleverse_core::AuthScheme {
    puzzleverse_core::AuthScheme::Password
  }

  async fn invite(&self, _: &str) -> Option<String> {
    None
  }
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    Password::is_locked(self, username).await
  }
  async fn lock(&self, username: &str, locked: bool) -> bool {
    Password::lock(self, username, locked).await
  }

  async fn handle(self: &Self, req: http::Request<hyper::Body>) -> AuthResult {
    use bytes::Buf;
    match (req.method(), req.uri().path()) {
      (&http::Method::POST, puzzleverse_core::net::PASSWORD_AUTH_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          eprintln!("Failed to aggregate body: {}", e);
          AuthResult::Failure
        }
        Ok(whole_body) => match serde_json::from_reader::<_, puzzleverse_core::PasswordRequest<String>>(whole_body.reader()) {
          Err(e) => AuthResult::Page(http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into())),
          Ok(data) => {
            if self.check(&data.username, &data.password).await {
              AuthResult::SendToken(data.username)
            } else {
              AuthResult::Page(http::Response::builder().status(http::StatusCode::UNAUTHORIZED).body("Invalid username or password".into()))
            }
          }
        },
      },
      _ => AuthResult::NotHandled,
    }
  }
}
/// Create an authentication provider that uses one-time passwords as authentication
#[async_trait::async_trait]
pub trait OTPStore: Send + Sync {
  /// Check if an account is locked.
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState;
  /// Lock (or unlock) a user account
  async fn lock(&self, username: &str, locked: bool) -> bool;
  /// Get the secrets for a user
  async fn secret(self: &Self, username: &str) -> Vec<String>;
}

#[async_trait::async_trait]
impl<T> Password for T
where
  T: OTPStore,
{
  async fn check(self: &Self, username: &str, password: &str) -> bool {
    match password.parse::<u32>() {
      Ok(code) => self.secret(&username).await.drain(..).any(|secret| {
        otpauth::TOTP::new(secret).verify(code, 30, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
      }),
      _ => false,
    }
  }

  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    OTPStore::is_locked(self, username).await
  }

  async fn lock(&self, username: &str, locked: bool) -> bool {
    OTPStore::lock(self, username, locked).await
  }
}
/// Create an authentication provider that deals with unencrypted usernames and passwords
#[async_trait::async_trait]
pub trait OpenIdConnectProvider: Send + Sync {
  type RegistrationTarget: serde::Serialize + serde::de::DeserializeOwned + Send + Sync;
  /// Check if the username and password provided are valid
  async fn client(&self, username: &str) -> Option<&openidconnect::core::CoreClient>;
  async fn is_available(&self, username: &str) -> bool;
  /// Check if an account is locked.
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState;
  /// Lock (or unlock) a user account
  async fn lock(&self, username: &str, locked: bool) -> bool;
  async fn register(&self, username: &str, selector: &Self::RegistrationTarget, subject: &str) -> Result<(), String>;
  fn registration(&self) -> OpenIdRegistration;
  async fn registration_client(&self, selector: &Self::RegistrationTarget) -> Option<&openidconnect::core::CoreClient>;
  fn registration_targets(&self) -> Box<dyn Iterator<Item = (String, Self::RegistrationTarget)> + '_>;

  /// Checks if the provided remote user ID (subject) matches the local user ID
  async fn validate(&self, username: &str, subject: &str) -> bool;
}

#[derive(Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OpenIdRegistration {
  Closed,
  Invite,
  Open,
}

struct OpenIdConnect<T: OpenIdConnectProvider> {
  provider: T,
  players: tokio::sync::Mutex<std::collections::BTreeMap<String, OpenIdConnectPlayerState>>,
  invitations: Option<tokio::sync::Mutex<std::collections::BTreeSet<String>>>,
  registrations: tokio::sync::Mutex<std::collections::BTreeMap<String, (chrono::DateTime<chrono::Utc>, String)>>,
  servers: tokio::sync::Mutex<std::collections::BTreeMap<String, OpenIdConnectServerState<T::RegistrationTarget>>>,
}

struct OpenIdConnectPlayerState {
  expires: chrono::DateTime<chrono::Utc>,
  player: String,
  receiver: tokio::sync::oneshot::Receiver<bool>,
}

struct OpenIdConnectServerState<T> {
  expires: chrono::DateTime<chrono::Utc>,
  nonce: openidconnect::Nonce,
  pkce_verifier: openidconnect::PkceCodeVerifier,
  callback: OpenIdCallback<T>,
}

enum OpenIdCallback<T> {
  Login { player: String, sender: tokio::sync::oneshot::Sender<bool> },
  Register { client_type: T, player: String, invitation: String },
}

#[serde_with::serde_as]
#[derive(serde::Deserialize)]
struct OpenIdRegistrationRequest<T>
where
  for<'d> T: serde::Deserialize<'d>,
{
  #[serde_as(as = "serde_with::json::JsonString")]
  client_type: T,
  invitation: Option<String>,
  player: String,
}

impl<T: OpenIdConnectProvider + 'static> OpenIdConnect<T> {
  pub(crate) fn new(provider: T) -> Result<std::sync::Arc<dyn AuthProvider>, String> {
    Ok(std::sync::Arc::new(OpenIdConnect {
      players: tokio::sync::Mutex::new(std::collections::BTreeMap::new()),
      invitations: if provider.registration() == OpenIdRegistration::Invite {
        Some(tokio::sync::Mutex::new(std::collections::BTreeSet::new()))
      } else {
        None
      },
      registrations: tokio::sync::Mutex::new(std::collections::BTreeMap::new()),
      servers: tokio::sync::Mutex::new(std::collections::BTreeMap::new()),
      provider,
    }))
  }
}

#[async_trait::async_trait]
impl<T> AuthProvider for OpenIdConnect<T>
where
  T: OpenIdConnectProvider,
{
  fn scheme(self: &Self) -> puzzleverse_core::AuthScheme {
    puzzleverse_core::AuthScheme::OpenIdConnect
  }

  async fn invite(&self, server_name: &str) -> Option<String> {
    match self.provider.registration() {
      OpenIdRegistration::Closed => None,
      OpenIdRegistration::Open => Some(format!("https://{}/register", server_name)),
      OpenIdRegistration::Invite => match &self.invitations {
        Some(invitations) => {
          let mut invitation = [0 as u8; 32];
          if let Err(e) = openssl::rand::rand_bytes(&mut invitation) {
            eprintln!("Failed to generate random bytes: {}", e);
            return None;
          }
          let invitation = base64::encode(invitation);
          let url = format!("https://{}/register?invitation={}", server_name, &invitation);
          invitations.lock().await.insert(invitation);
          Some(url)
        }
        None => None,
      },
    }
  }

  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    self.provider.is_locked(username).await
  }

  async fn lock(&self, username: &str, locked: bool) -> bool {
    self.provider.lock(username, locked).await
  }
  async fn handle(self: &Self, req: http::Request<hyper::Body>) -> AuthResult {
    fn query_parameter<'a>(name: &str, req: &'a http::Request<hyper::Body>) -> Option<std::borrow::Cow<'a, str>> {
      req.uri().query().map(|q| form_urlencoded::parse(q.as_bytes()).filter(|(n, _)| n == name).map(|(_, value)| value).next()).flatten()
    }
    match (req.method(), req.uri().path()) {
      (&http::Method::GET, puzzleverse_core::net::OIDC_AUTH_START_PATH) => match query_parameter("player", &req) {
        None => AuthResult::Failure,
        Some(player) => match self.provider.client(&*player).await {
          None => AuthResult::Failure,
          Some(client) => {
            use sha3::Digest;
            let (pkce_challenge, pkce_verifier) = openidconnect::PkceCodeChallenge::new_random_sha256();
            let (auth_url, csrf_token, nonce) = client
              .authorize_url(
                openidconnect::core::CoreAuthenticationFlow::AuthorizationCode,
                openidconnect::CsrfToken::new_random,
                openidconnect::Nonce::new_random,
              )
              .add_scope(openidconnect::Scope::new("openid".to_owned()))
              .add_scope(openidconnect::Scope::new("profile".to_owned()))
              .set_pkce_challenge(pkce_challenge)
              .url();
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let expires = chrono::Utc::now() + chrono::Duration::minutes(10);
            let mut request_id = sha3::Sha3_512::new();
            request_id.update(csrf_token.secret().as_bytes());
            request_id.update(player.as_bytes());
            let request_id = hex::encode(request_id.finalize());
            let player = player.into_owned();
            self.servers.lock().await.insert(
              csrf_token.secret().clone(),
              OpenIdConnectServerState {
                pkce_verifier,
                nonce,
                expires: expires.clone(),
                callback: OpenIdCallback::Login { sender, player: player.clone() },
              },
            );
            self.players.lock().await.insert(request_id.clone(), OpenIdConnectPlayerState { receiver, expires, player });
            AuthResult::Page(http::Response::builder().status(http::StatusCode::OK).body(
              serde_json::to_vec(&puzzleverse_core::OpenIdConnectInformation { authorization_url: auth_url.to_string(), request_id }).unwrap().into(),
            ))
          }
        },
      },
      (&http::Method::GET, puzzleverse_core::net::OIDC_AUTH_FINISH_PATH) => match query_parameter("request_id", &req) {
        Some(request_id) => {
          let state = {
            let mut players = self.players.lock().await;
            let now = chrono::Utc::now();
            let dead: Vec<_> = players.iter().filter(|(_, p)| p.expires > now).map(|(k, _)| k.clone()).collect();
            for key in dead {
              players.remove(&key);
            }
            players.remove(&*request_id)
          };
          match state {
            None => AuthResult::Failure,
            Some(state) => match state.receiver.await {
              Ok(allowed) => {
                if allowed {
                  AuthResult::SendToken(state.player)
                } else {
                  AuthResult::Failure
                }
              }
              Err(_) => AuthResult::Failure,
            },
          }
        }
        None => AuthResult::Failure,
      },
      (&http::Method::GET, "/api/auth/oidc/auth") => match (query_parameter("state", &req), query_parameter("code", &req)) {
        (Some(csrf_token), Some(code)) => match {
          let mut servers = self.servers.lock().await;
          let now = chrono::Utc::now();
          let dead: Vec<_> = servers.iter().filter(|(_, p)| p.expires > now).map(|(k, _)| k.clone()).collect();
          for key in dead {
            servers.remove(&key);
          }
          servers.remove(&*csrf_token)
        } {
          None => AuthResult::Failure,
          Some(info) => {
            let client = match &info.callback {
              OpenIdCallback::Login { player, .. } => self.provider.client(player).await,
              OpenIdCallback::Register { client_type, .. } => self.provider.registration_client(client_type).await,
            };
            match client {
              None => AuthResult::Failure,
              Some(client) => {
                use openidconnect::TokenResponse;
                let nonce = info.nonce;
                let  message = match client
                    .exchange_code(openidconnect::AuthorizationCode::new(code.into_owned()))
                    .set_pkce_verifier(info.pkce_verifier)
                    .request_async(openidconnect::reqwest::async_http_client)
                    .await
                {
                  Ok(auth) => match auth.id_token().map(|id| id.claims(&client.id_token_verifier(), &nonce)) {
                    Some(Ok(id_token)) =>
                    match info.callback {
                      OpenIdCallback::Login {player, sender} => {
                        let (ok, message) = if self.provider.validate(&player, &*id_token.subject()).await {
                          (true, "You have been logged in! Switch back to your client.")
                        } else {
                          (false, "This is not the account associated with this player.")
                        };
                        // Ignore the error, because if the client has disconnected already, that's an acceptable failure mode
                        let _ = sender.send(ok);
                        std::borrow::Cow::Borrowed(message)
                      },
                      OpenIdCallback::Register{ player, invitation, client_type} =>
                        match self.provider.register(&player, &client_type, &*id_token.subject()).await {
                        Ok(()) => {
                          let mut registrations = self.registrations.lock().await;
                          registrations.remove(&player);
                          if let Some(invitations) = &self.invitations {
                            invitations.lock().await.remove(&invitation);
                            registrations.retain(|_, (_, v)| v != &invitation);
                          }
                          std::borrow::Cow::Owned(format!("The account for {} has been registered. Please login.", &player))
                        }
                        Err(e) => std::borrow::Cow::Owned(e),
                      }
                    }
                    Some(Err(e)) => std::borrow::Cow::Owned(format!("Failed to validate OpenId Connect claim: {:?}", e)),
                    None =>  std::borrow::Cow::Borrowed("The Puzzleverse server has be connected to a non-OpenID Connect-enable OAuth server. Contact your server administrator. If you are the server administrator, choose a different OpenID server or adjust it to enable OpenID Connect."),
                  },
                  Err(e) =>  std::borrow::Cow::Owned(e.to_string()),
                };
                AuthResult::Page(http::Response::builder().status(http::StatusCode::OK).body(crate::html::create_oauth_result(&*message).into()))
              }
            }
          }
        },
        _ => AuthResult::Failure,
      },
      (&http::Method::POST, "/register-next") => match hyper::body::aggregate(req).await {
        Ok(whole_body) => {
          use bytes::Buf;
          match serde_urlencoded::from_reader::<OpenIdRegistrationRequest<T::RegistrationTarget>, _>(whole_body.reader()) {
            Ok(request) => {
              let mut registrations = self.registrations.lock().await;
              let now = chrono::Utc::now();
              registrations.retain(|_, (t, _)| &now < t);
              let is_in_progress = match (&request.invitation, registrations.get(&request.player)) {
                (Some(requested_invitation), Some((_, active_invitation))) => requested_invitation != active_invitation,
                _ => false,
              };
              AuthResult::Page(if is_in_progress {
                http::Response::builder().status(http::StatusCode::OK).body(
                  crate::html::create_oauth_register(
                    request.invitation,
                    self.provider.registration_targets(),
                    Some("Registration already in progress for this player name."),
                  )
                  .into(),
                )
              } else {
                if match (self.invitations.as_ref(), request.invitation.as_ref()) {
                  (Some(invitations), Some(invitation)) => !invitations.lock().await.contains(invitation),
                  (None, None) => false,
                  _ => true,
                } {
                  http::Response::builder().status(http::StatusCode::OK).body("Invalid invitation.".into())
                } else if self.provider.is_available(&request.player).await {
                  match self.provider.registration_client(&request.client_type).await {
                    Some(client) => {
                      let (pkce_challenge, pkce_verifier) = openidconnect::PkceCodeChallenge::new_random_sha256();
                      let (auth_url, csrf_token, nonce) = client
                        .authorize_url(
                          openidconnect::core::CoreAuthenticationFlow::AuthorizationCode,
                          openidconnect::CsrfToken::new_random,
                          openidconnect::Nonce::new_random,
                        )
                        .add_scope(openidconnect::Scope::new("openid".to_owned()))
                        .add_scope(openidconnect::Scope::new("profile".to_owned()))
                        .set_pkce_challenge(pkce_challenge)
                        .url();
                      let invitation = request.invitation.unwrap_or_else(String::new);
                      registrations.insert(request.player.clone(), (chrono::Utc::now() + chrono::Duration::minutes(10), invitation.clone()));
                      self.servers.lock().await.insert(
                        csrf_token.secret().clone(),
                        OpenIdConnectServerState {
                          pkce_verifier,
                          nonce,
                          expires: chrono::Utc::now() + chrono::Duration::minutes(10),
                          callback: OpenIdCallback::Register { client_type: request.client_type, invitation, player: request.player },
                        },
                      );
                      http::Response::builder()
                        .status(http::StatusCode::SEE_OTHER)
                        .header("Location", auth_url.as_str())
                        .body(hyper::Body::empty().into())
                    }
                    None => http::Response::builder().status(http::StatusCode::OK).body(
                      crate::html::create_oauth_register(
                        request.invitation,
                        self.provider.registration_targets(),
                        Some("That registration is not available. Please choose a different one."),
                      )
                      .into(),
                    ),
                  }
                } else {
                  http::Response::builder().status(http::StatusCode::OK).body(
                    crate::html::create_oauth_register(
                      request.invitation,
                      self.provider.registration_targets(),
                      Some("Another player is using that name. Please choose a different one."),
                    )
                    .into(),
                  )
                }
              })
            }
            Err(e) => AuthResult::Page(
              http::Response::builder().status(http::StatusCode::OK).body(format!("Failed to get registration information: {}", e).into()),
            ),
          }
        }
        Err(e) => AuthResult::Page(
          http::Response::builder().status(http::StatusCode::OK).body(format!("Failed to read registration information: {}", e).into()),
        ),
      },
      (&http::Method::GET, "/register") => match self.provider.registration() {
        OpenIdRegistration::Closed => AuthResult::NotHandled,
        OpenIdRegistration::Open => AuthResult::Page(
          http::Response::builder()
            .status(http::StatusCode::OK)
            .body(crate::html::create_oauth_register(None::<&str>, self.provider.registration_targets(), None).into()),
        ),
        OpenIdRegistration::Invite => AuthResult::Page(
          http::Response::builder().status(http::StatusCode::OK).body(
            crate::html::create_oauth_register(
              Some(query_parameter("invitation", &req).unwrap_or(std::borrow::Cow::Borrowed("")).as_ref()),
              self.provider.registration_targets(),
              None,
            )
            .into(),
          ),
        ),
      },
      _ => AuthResult::NotHandled,
    }
  }
}

pub(crate) fn create_oidc_client(
  endpoint: AuthOpenIdConnectEndpoint,
  client_id: String,
  client_secret: String,
  server_name: &str,
) -> Result<(openidconnect::core::CoreClient, String, String), String> {
  let (url, name) = endpoint.into_url_and_name();
  let provider_metadata = openidconnect::core::CoreProviderMetadata::discover(
    &openidconnect::IssuerUrl::new(url).map_err(|e| format!("Failed to get OpenID Connect provider URL: {}", e))?,
    openidconnect::reqwest::http_client,
  )
  .map_err(|e| format!("Failed to get OpenID Connect provider information: {:?}", e))?;
  let issuer = provider_metadata.issuer().url().to_string();
  Ok((
    openidconnect::core::CoreClient::from_provider_metadata(
      provider_metadata,
      openidconnect::ClientId::new(client_id),
      Some(openidconnect::ClientSecret::new(client_secret)),
    )
    .set_redirect_uri(
      openidconnect::RedirectUrl::new(format!("http://{}/api/auth/oidc/auth", server_name))
        .map_err(|e| format!("Failed to create OpenID callback URL: {}", e))?,
    ),
    issuer,
    name,
  ))
}
