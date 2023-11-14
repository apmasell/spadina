use crate::accounts::login::{Login, LoginRequest, LoginResponse};
use crate::accounts::policy::{Policy, PolicyRequest};
use crate::accounts::AuthResult;
use crate::http_server::aggregate;
use bb8::{Pool, PooledConnection, RunError};
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use ldap3::{LdapError, LdapResult, SearchEntry, SearchResult};
use serde::{Deserialize, Serialize};
use spadina_core::net::server::auth::{AuthScheme, PasswordRequest};
use spadina_core::net::server::AUTH_METHOD_PATH;
use spadina_core::UpdateResult;
use std::future::Future;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LightweightDirectoryConfiguration {
  account_attr: String,
  admin_query: Option<String>,
  bind_dn: String,
  bind_pw: String,
  create_query: Option<String>,
  search_base: String,
  server_url: String,
  user_query: String,
}
pub struct LightweightDirectory {
  account_attr: String,
  admin_query: Option<String>,
  bind_dn: String,
  bind_pw: String,
  connection: Pool<LDAPConnectionManager<String>>,
  create_query: Option<String>,
  search_base: String,
  user_query: String,
}

struct LDAPConnectionManager<T: AsRef<str>>(T);
impl<T: AsRef<str> + Sized + Sync + Send + 'static> bb8::ManageConnection for LDAPConnectionManager<T> {
  type Connection = ldap3::Ldap;
  type Error = LdapError;

  async fn connect(&self) -> Result<Self::Connection, Self::Error> {
    let (connection, ldap) = ldap3::LdapConnAsync::new(self.0.as_ref()).await?;
    ldap3::drive!(connection);
    Ok(ldap)
  }

  async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
    conn.extended(ldap3::exop::WhoAmI).await?;
    Ok(())
  }

  fn has_broken(&self, conn: &mut Self::Connection) -> bool {
    conn.is_closed()
  }
}

impl LightweightDirectory {
  /// Access a Myst Online: Uru Live database for accounts
  pub async fn new(configuration: LightweightDirectoryConfiguration) -> Result<Self, LdapError> {
    Ok(LightweightDirectory {
      connection: Pool::builder().build(LDAPConnectionManager(configuration.server_url)).await?,
      account_attr: configuration.account_attr,
      admin_query: configuration.admin_query,
      bind_dn: configuration.bind_dn,
      bind_pw: configuration.bind_pw,
      create_query: configuration.create_query,
      search_base: configuration.search_base,
      user_query: configuration.user_query,
    })
  }
  async fn bind(
    &self,
    connection: &mut PooledConnection<'_, LDAPConnectionManager<String>>,
    username: &str,
    password: &str,
  ) -> Result<(), LdapError> {
    connection.simple_bind(username, password).await.and_then(|result| result.success())?;
    Ok(())
  }
  async fn query_username(
    &self,
    username: &str,
    query: &str,
  ) -> Result<(Option<String>, PooledConnection<LDAPConnectionManager<String>>), RunError<LdapError>> {
    let mut connection = self.connection.get().await?;
    self.bind(&mut connection, &self.bind_dn, &self.bind_pw).await?;
    let result = connection
      .search(
        &self.search_base,
        ldap3::Scope::Subtree,
        &format!("(&({}={})({}))", &self.account_attr, ldap3::ldap_escape(username), query),
        vec!["cn", "dn", &self.account_attr],
      )
      .await
      .map(|SearchResult(results, _)| {
        results
          .into_iter()
          .next()
          .map(|entry| SearchEntry::construct(entry).attrs.remove(&self.account_attr))
          .flatten()
          .map(|attr| attr.into_iter().next())
          .flatten()
      })?;
    Ok((result, connection))
  }
}
impl Login for LightweightDirectory {
  fn administration_request(&self, request: LoginRequest) -> impl Future<Output = LoginResponse> + Send {
    async move {
      match request {
        LoginRequest::LockAccount(_, _) => LoginResponse::LockAccount(None),
        LoginRequest::Invite => LoginResponse::Invite(None),
      }
    }
  }

  fn http_handle(&self, req: Request<Incoming>) -> impl Future<Output = AuthResult> + Send {
    async move {
      match (req.method(), req.uri().path()) {
        (&Method::GET, AUTH_METHOD_PATH) => match aggregate::<PasswordRequest<String>>(req).await {
          Err(response) => AuthResult::Page(response),
          Ok(request) => match self.query_username(&request.username, &self.user_query).await {
            Err(e) => {
              eprintln!("Failed to access LDAP: {:?}", e);
              return AuthResult::Page(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default()));
            }
            Ok((None, _)) => AuthResult::Page(Response::builder().status(StatusCode::FORBIDDEN).body("Unknown username.".into())),
            Ok((Some(username), mut connection)) => match connection.simple_bind(&username, &request.password).await {
              Ok(LdapResult { rc, .. }) => {
                if rc == 0 {
                  AuthResult::SendToken(username)
                } else {
                  AuthResult::Page(Response::builder().status(StatusCode::FORBIDDEN).body("Invalid password.".into()))
                }
              }
              Err(e) => {
                eprintln!("Failed to access LDAP: {:?}", e);
                AuthResult::Page(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default()))
              }
            },
          },
        },
        _ => AuthResult::NotHandled,
      }
    }
  }

  fn normalize_username(&self, player: String) -> impl Future<Output = Result<String, ()>> + Send {
    async move {
      let (result, _) = self.query_username(&player, &self.user_query).await.map_err(|e| {
        eprintln!("Failed to access LDAP: {:?}", e);
      })?;
      result.ok_or(())
    }
  }

  fn scheme(&self) -> AuthScheme {
    AuthScheme::Password
  }
}
impl Policy for LightweightDirectory {
  fn can_create(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move {
      match self.create_query.as_ref() {
        None => true,
        Some(create_query) => match self.query_username(&player, create_query).await {
          Err(e) => {
            eprintln!("Failed to access LDAP: {:?}", e);
            false
          }
          Ok((None, _)) => false,
          Ok((Some(_), _)) => true,
        },
      }
    }
  }

  fn is_administrator(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move {
      match self.admin_query.as_ref() {
        None => true,
        Some(admin_query) => match self.query_username(&player, admin_query).await {
          Err(e) => {
            eprintln!("Failed to access LDAP: {:?}", e);
            false
          }
          Ok((None, _)) => false,
          Ok((Some(_), _)) => true,
        },
      }
    }
  }

  fn request(&self, _request: PolicyRequest) -> impl Future<Output = UpdateResult> + Send {
    async move { UpdateResult::NotAllowed }
  }
}
