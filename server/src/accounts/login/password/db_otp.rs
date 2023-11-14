use crate::accounts::db_auth::schema_otp::auth_otp::dsl as auth_otp_schema;
use crate::accounts::db_auth::OTP_MIGRATIONS;
use crate::accounts::login::password::otp::OneTimePasswordStore;
use crate::database::connect::DatabaseBackend;
use diesel::prelude::*;
use std::error::Error;
use std::future::Future;

pub struct DatabaseOneTimePasswords(DatabaseBackend);

impl TryFrom<DatabaseBackend> for DatabaseOneTimePasswords {
  type Error = Box<dyn Error + Send + Sync>;

  fn try_from(value: DatabaseBackend) -> Result<Self, Self::Error> {
    match &value {
      DatabaseBackend::SQLite(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OTP_MIGRATIONS)?;
      }
      #[cfg(feature = "postgres")]
      DatabaseBackend::PostgreSQL(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OTP_MIGRATIONS)?;
      }
      #[cfg(feature = "mysql")]
      DatabaseBackend::MySql(pool) => {
        use diesel_migrations::MigrationHarness;
        let mut db_connection = pool.get()?;
        db_connection.run_pending_migrations(OTP_MIGRATIONS)?;
      }
    }
    Ok(DatabaseOneTimePasswords(value))
  }
}
impl OneTimePasswordStore for DatabaseOneTimePasswords {
  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move {
      let result = match &self.0 {
        DatabaseBackend::SQLite(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return None;
          };
          diesel::update(auth_otp_schema::auth_otp.filter(auth_otp_schema::name.eq(username)))
            .set(auth_otp_schema::locked.eq(locked))
            .execute(&mut db_connection)
        }
        #[cfg(feature = "postgres")]
        DatabaseBackend::PostgreSQL(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return None;
          };
          diesel::update(auth_otp_schema::auth_otp.filter(auth_otp_schema::name.eq(username)))
            .set(auth_otp_schema::locked.eq(locked))
            .execute(&mut db_connection)
        }
        #[cfg(feature = "mysql")]
        DatabaseBackend::MySql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return None;
          };
          diesel::update(auth_otp_schema::auth_otp.filter(auth_otp_schema::name.eq(username)))
            .set(auth_otp_schema::locked.eq(locked))
            .execute(&mut db_connection)
        }
      };

      match result {
        Ok(results) => Some(results > 0),
        Err(e) => {
          eprintln!("Failed to set locks on OTPs for {}: {}", username, e);
          None
        }
      }
    }
  }

  fn secret(&self, username: &str) -> impl Future<Output = Vec<String>> + Send {
    async move {
      let result = match &self.0 {
        DatabaseBackend::SQLite(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return Vec::new();
          };
          auth_otp_schema::auth_otp
            .select(auth_otp_schema::code)
            .filter(auth_otp_schema::name.eq(username).and(auth_otp_schema::locked.eq(false)))
            .get_results(&mut db_connection)
            .optional()
        }
        #[cfg(feature = "postgres")]
        DatabaseBackend::PostgreSQL(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return Vec::new();
          };
          auth_otp_schema::auth_otp
            .select(auth_otp_schema::code)
            .filter(auth_otp_schema::name.eq(username).and(auth_otp_schema::locked.eq(false)))
            .get_results(&mut db_connection)
            .optional()
        }
        #[cfg(feature = "mysql")]
        DatabaseBackend::MySql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return Vec::new();
          };
          auth_otp_schema::auth_otp
            .select(auth_otp_schema::code)
            .filter(auth_otp_schema::name.eq(username).and(auth_otp_schema::locked.eq(false)))
            .get_results(&mut db_connection)
            .optional()
        }
      };
      match result {
        Ok(results) => results.unwrap_or_default(),
        Err(e) => {
          eprintln!("Failed to fetch OTPs for {}: {}", username, e);
          vec![]
        }
      }
    }
  }
}
