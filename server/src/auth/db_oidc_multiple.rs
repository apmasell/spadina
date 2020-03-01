use diesel::prelude::*;
type ClientValue = (String, openidconnect::core::CoreClient);
struct DatabaseMultipleOpenIdConnect {
  clients: std::collections::BTreeMap<String, ClientValue>,
  connection: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>,
  registration: super::OpenIdRegistration,
}
/// Create an OpenID Connect store that can use multiple providers
pub fn new(
  database_url: String,
  clients: std::collections::BTreeMap<String, (String, openidconnect::core::CoreClient)>,
  registration: super::OpenIdRegistration,
) -> Result<std::sync::Arc<dyn crate::auth::AuthProvider>, String> {
  let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new(database_url);
  crate::auth::OpenIdConnect::new(DatabaseMultipleOpenIdConnect {
    clients,
    connection: diesel::r2d2::Pool::builder().build(manager).map_err(|e| format!("Failed to create OpenID Connect database connection: {}", e))?,
    registration,
  })
}

#[async_trait::async_trait]
impl<'a> crate::auth::OpenIdConnectProvider for DatabaseMultipleOpenIdConnect {
  type RegistrationTarget = String;

  async fn client(self: &Self, username: &str) -> Option<&openidconnect::core::CoreClient> {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match auth_oidc_schema::authoidc
          .select(auth_oidc_schema::issuer)
          .filter(auth_oidc_schema::name.eq(username))
          .get_results::<Option<String>>(&mut db_connection)
        {
          Ok(results) => results.into_iter().next().flatten().map(|issuer| self.clients.get(&issuer)).flatten().map(|(_, c)| c),
          Err(diesel::result::Error::NotFound) => None,
          Err(e) => {
            eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
            None
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        None
      }
    }
  }

  async fn is_available(&self, username: &str) -> bool {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    match self.connection.get() {
      Ok(mut db_connection) => match auth_oidc_schema::authoidc
        .select(diesel::dsl::count_star())
        .filter(auth_oidc_schema::name.eq(username))
        .get_results::<i64>(&mut db_connection)
      {
        Ok(results) => results.get(0).map(|&v| v == 0).unwrap_or(true),
        Err(diesel::result::Error::NotFound) => false,
        Err(e) => {
          eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
          false
        }
      },
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        false
      }
    }
  }

  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match auth_oidc_schema::authoidc
          .select(auth_oidc_schema::locked)
          .filter(auth_oidc_schema::name.eq(username))
          .get_results::<bool>(&mut db_connection)
        {
          Ok(results) => {
            if results.iter().all(|&x| x) {
              puzzleverse_core::AccountLockState::Locked
            } else {
              puzzleverse_core::AccountLockState::Unlocked
            }
          }
          Err(diesel::result::Error::NotFound) => puzzleverse_core::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
            puzzleverse_core::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        puzzleverse_core::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, username: &str, locked: bool) -> bool {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    match self.connection.get() {
      Ok(mut db_connection) => match diesel::update(auth_oidc_schema::authoidc.filter(auth_oidc_schema::name.eq(username)))
        .set(auth_oidc_schema::locked.eq(locked))
        .execute(&mut db_connection)
      {
        Ok(results) => results > 0,
        Err(e) => {
          eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
          false
        }
      },
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        false
      }
    }
  }

  async fn register(&self, username: &str, issuer: &Self::RegistrationTarget, subject: &str) -> Result<(), String> {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    let mut db_connection =
      self.connection.get().map_err(|e| format!("Failed to get connection to fetch OpenID information for {}: {}", username, e))?;
    diesel::insert_into(auth_oidc_schema::authoidc)
      .values((auth_oidc_schema::name.eq(username), auth_oidc_schema::issuer.eq(issuer), auth_oidc_schema::subject.eq(subject)))
      .execute(&mut db_connection)
      .map_err(|e| format!("Failed to create record for {}: {}", username, e))
      .and_then(|v| if v > 0 { Ok(()) } else { Err(format!("Record for {} was not update", username)) })
  }

  fn registration(&self) -> super::OpenIdRegistration {
    self.registration
  }

  async fn registration_client(&self, issuer: &Self::RegistrationTarget) -> Option<&openidconnect::core::CoreClient> {
    self.clients.get(issuer.as_str()).map(|(_, c)| c)
  }

  fn registration_targets(&self) -> Box<dyn Iterator<Item = (String, Self::RegistrationTarget)> + '_> {
    Box::new(self.clients.iter().map(|(issuer, (name, _))| (format!("Register with {}", name), issuer.clone())))
  }

  async fn validate(self: &Self, username: &str, subject: &str) -> bool {
    use crate::schema::authoidc::dsl as auth_oidc_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match auth_oidc_schema::authoidc
          .select(diesel::dsl::count_star())
          .filter(auth_oidc_schema::name.eq(username).and(auth_oidc_schema::subject.eq(subject)))
          .get_results::<i64>(&mut db_connection)
        {
          Ok(results) => results.get(0).map(|&v| v > 0).unwrap_or(false),
          Err(diesel::result::Error::NotFound) => false,
          Err(e) => {
            eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
            false
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        false
      }
    }
  }
}
