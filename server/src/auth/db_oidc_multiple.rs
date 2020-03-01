use diesel::prelude::*;
type ClientValue = (String, openidconnect::core::CoreClient);
struct DatabaseMultipleOpenIdConnect {
  clients: std::collections::BTreeMap<String, ClientValue>,
  registration: super::OpenIdRegistration,
}
/// Create an OpenID Connect store that can use multiple providers
pub(crate) fn new(
  clients: std::collections::BTreeMap<String, (String, openidconnect::core::CoreClient)>,
  registration: super::OpenIdRegistration,
) -> Result<Box<dyn crate::auth::AuthProvider>, String> {
  crate::auth::OpenIdConnect::new(DatabaseMultipleOpenIdConnect { clients, registration })
}

#[async_trait::async_trait]
impl<'a> crate::auth::OpenIdConnectProvider for DatabaseMultipleOpenIdConnect {
  type RegistrationTarget = String;

  async fn client(self: &Self, username: &str, connection: &crate::database::Database) -> Option<&openidconnect::core::CoreClient> {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    match connection.get() {
      Ok(mut db_connection) => {
        match auth_oidc_schema::authoidc
          .select(auth_oidc_schema::issuer)
          .filter(auth_oidc_schema::name.eq(username))
          .load_iter::<Option<String>, diesel::connection::DefaultLoadingMode>(&mut db_connection)
          .and_then(|mut v| v.next().unwrap_or(Err(diesel::result::Error::NotFound)))
        {
          Ok(results) => results.map(|issuer| self.clients.get(&issuer)).flatten().map(|(_, c)| c),
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

  async fn is_available(&self, username: &str, connection: &crate::database::Database) -> bool {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    match connection.get() {
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

  async fn is_locked(&self, username: &str, connection: &crate::database::Database) -> spadina_core::access::AccountLockState {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    match connection.get() {
      Ok(mut db_connection) => {
        match auth_oidc_schema::authoidc
          .select(auth_oidc_schema::locked)
          .filter(auth_oidc_schema::name.eq(username))
          .get_results::<bool>(&mut db_connection)
        {
          Ok(results) => {
            if results.iter().all(|&x| x) {
              spadina_core::access::AccountLockState::Locked
            } else {
              spadina_core::access::AccountLockState::Unlocked
            }
          }
          Err(diesel::result::Error::NotFound) => spadina_core::access::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
            spadina_core::access::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        spadina_core::access::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, username: &str, locked: bool, connection: &crate::database::Database) -> spadina_core::UpdateResult {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    match connection.get() {
      Ok(mut db_connection) => match diesel::update(auth_oidc_schema::authoidc.filter(auth_oidc_schema::name.eq(username)))
        .set(auth_oidc_schema::locked.eq(locked))
        .execute(&mut db_connection)
      {
        Ok(results) => {
          if results > 0 {
            spadina_core::UpdateResult::Success
          } else {
            spadina_core::UpdateResult::InternalError
          }
        }
        Err(e) => {
          eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
          spadina_core::UpdateResult::InternalError
        }
      },
      Err(e) => {
        eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
        spadina_core::UpdateResult::InternalError
      }
    }
  }

  async fn register(
    &self,
    username: &str,
    issuer: &Self::RegistrationTarget,
    subject: &str,
    connection: &crate::database::Database,
  ) -> Result<(), String> {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    let mut db_connection = connection.get().map_err(|e| format!("Failed to get connection to fetch OpenID information for {}: {}", username, e))?;
    diesel::insert_into(auth_oidc_schema::authoidc)
      .values((auth_oidc_schema::name.eq(username), auth_oidc_schema::issuer.eq(issuer), auth_oidc_schema::subject.eq(subject)))
      .execute(&mut db_connection)
      .map_err(|e| format!("Failed to create record for {}: {}", username, e))
      .and_then(|v| if v > 0 { Ok(()) } else { Err(format!("Record for {} was not update", username)) })
  }

  fn registration(&self) -> super::OpenIdRegistration {
    self.registration
  }

  async fn registration_client(&self, issuer: &Self::RegistrationTarget, _: &crate::database::Database) -> Option<&openidconnect::core::CoreClient> {
    self.clients.get(issuer.as_str()).map(|(_, c)| c)
  }

  fn registration_targets(&self) -> Box<dyn Iterator<Item = (String, Self::RegistrationTarget)> + '_> {
    Box::new(self.clients.iter().map(|(issuer, (name, _))| (format!("Register with {}", name), issuer.clone())))
  }

  async fn validate(self: &Self, username: &str, subject: &str, connection: &crate::database::Database) -> bool {
    use crate::database::schema::authoidc::dsl as auth_oidc_schema;
    match connection.get() {
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
