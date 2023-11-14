use crate::accounts::db_auth::schema_oidc::auth_oidc::dsl as auth_oidc_schema;
use crate::accounts::db_auth::OIDC_MIGRATIONS;
use crate::accounts::login::openid::OpenIdConnectProvider;
use crate::accounts::AuthResult;
use crate::database::connect::DatabaseBackend;
use diesel::prelude::*;
use openidconnect::core::CoreClient;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;

pub struct OpenIdClient {
  pub name: String,
  pub client: CoreClient,
}

pub struct DatabaseOpenIdConnect {
  pool: DatabaseBackend,
  clients: BTreeMap<String, OpenIdClient>,
}
/// Create an OpenID Connect store that can use multiple providers
impl DatabaseOpenIdConnect {
  pub fn new(pool: DatabaseBackend, clients: BTreeMap<String, OpenIdClient>) -> Result<Self, Box<dyn Error + Send + Sync>> {
    match &pool {
      DatabaseBackend::SQLite(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OIDC_MIGRATIONS)?;
      }
      #[cfg(feature = "postgres")]
      DatabaseBackend::PostgreSQL(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OIDC_MIGRATIONS)?;
      }
      #[cfg(feature = "mysql")]
      DatabaseBackend::MySql(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OIDC_MIGRATIONS)?;
      }
    }
    Ok(DatabaseOpenIdConnect { pool, clients })
  }
}
impl OpenIdConnectProvider for DatabaseOpenIdConnect {
  type Callback = String;

  fn client_for(&self, username: &str) -> impl Future<Output = Option<&CoreClient>> + Send {
    async move {
      let result = match &self.pool {
        DatabaseBackend::SQLite(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(auth_oidc_schema::issuer)
            .filter(auth_oidc_schema::name.eq(username).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<String>(&mut db_connection)
            .optional()
        }),
        #[cfg(feature = "postgres")]
        DatabaseBackend::PostgreSQL(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(auth_oidc_schema::issuer)
            .filter(auth_oidc_schema::name.eq(username).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<String>(&mut db_connection)
            .optional()
        }),
        #[cfg(feature = "mysql")]
        DatabaseBackend::MySql(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(auth_oidc_schema::issuer)
            .filter(auth_oidc_schema::name.eq(username).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<String>(&mut db_connection)
            .optional()
        }),
      };
      match result {
        Ok(Ok(None)) => None,
        Ok(Ok(Some(issuer))) => self.clients.get(&issuer).map(|c| &c.client),
        Ok(Err(e)) => {
          eprintln!("Failed to fetch OpenId information for {}: {}", username, e);
          None
        }
        Err(e) => {
          eprintln!("Failed to get connection to fetch OpenID information for {}: {}", username, e);
          None
        }
      }
    }
  }

  fn client_for_active<'a>(&'a self, request: &Self::Callback) -> impl Future<Output = Option<&'a CoreClient>> + Send {
    self.client_for(&*request)
  }

  fn start_login(&self, player: String) -> impl Future<Output = Self::Callback> + Send {
    async move { player }
  }

  fn finish_login(&self, callback: Self::Callback, subject: &str) -> impl Future<Output = AuthResult> + Send {
    async move {
      let result = match &self.pool {
        DatabaseBackend::SQLite(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(diesel::dsl::count_star())
            .filter(auth_oidc_schema::name.eq(&callback).and(auth_oidc_schema::subject.eq(subject)).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<i64>(&mut db_connection)
        }),
        #[cfg(feature = "postgres")]
        DatabaseBackend::PostgreSQL(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(diesel::dsl::count_star())
            .filter(auth_oidc_schema::name.eq(&callback).and(auth_oidc_schema::subject.eq(subject)).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<i64>(&mut db_connection)
        }),
        #[cfg(feature = "mysql")]
        DatabaseBackend::MySql(pool) => pool.get().map(|mut db_connection| {
          auth_oidc_schema::auth_oidc
            .select(diesel::dsl::count_star())
            .filter(auth_oidc_schema::name.eq(&callback).and(auth_oidc_schema::subject.eq(subject)).and(auth_oidc_schema::locked.eq(false)))
            .get_result::<i64>(&mut db_connection)
        }),
      };
      match result {
        Ok(Ok(count)) => {
          if count > 0 {
            AuthResult::RedirectToken(callback)
          } else {
            AuthResult::Failure
          }
        }
        Ok(Err(e)) => {
          eprintln!("Failed to fetch OpenId information for {}: {}", &callback, e);
          AuthResult::Failure
        }
        Err(e) => {
          eprintln!("Failed to get connection to fetch OpenID information for {}: {}", &callback, e);
          AuthResult::Failure
        }
      }
    }
  }
}
