use crate::accounts::db_auth::schema_otp::authotp::dsl as auth_otp_schema;
use crate::accounts::db_auth::OTP_MIGRATIONS;
use crate::accounts::login::password::otp::OneTimePasswordStore;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use std::error::Error;
use std::future::Future;

pub enum DatabaseOneTimePasswords {
  Postgresql(Pool<ConnectionManager<PgConnection>>),
  #[cfg(feature = "mysql")]
  MySql(Pool<ConnectionManager<MysqlConnection>>),
}

impl DatabaseOneTimePasswords {
  pub fn new_postgres(db_url: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
    let pool = {
      let manager = ConnectionManager::<PgConnection>::new(db_url);
      Pool::builder().build(manager)?
    };
    use diesel_migrations::MigrationHarness;
    let mut db_connection = pool.get()?;
    db_connection.run_pending_migrations(OTP_MIGRATIONS)?;
    Ok(DatabaseOneTimePasswords::Postgresql(pool))
  }
  #[cfg(feature = "mysql")]
  pub fn new_mysql(db_url: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
    let pool = {
      let manager = ConnectionManager::<MysqlConnection>::new(db_url);
      Pool::builder().build(manager)?
    };
    use diesel_migrations::MigrationHarness;
    let mut db_connection = pool.get()?;
    db_connection.run_pending_migrations(OTP_MIGRATIONS)?;
    Ok(DatabaseOneTimePasswords::MySql(pool))
  }
}
impl OneTimePasswordStore for DatabaseOneTimePasswords {
  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move {
      let result = match self {
        DatabaseOneTimePasswords::Postgresql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return None;
          };
          diesel::update(auth_otp_schema::authotp.filter(auth_otp_schema::name.eq(username)))
            .set(auth_otp_schema::locked.eq(locked))
            .execute(&mut db_connection)
        }
        #[cfg(feature = "mysql")]
        DatabaseOneTimePasswords::MySql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return None;
          };
          diesel::update(auth_otp_schema::authotp.filter(auth_otp_schema::name.eq(username)))
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
      let result = match self {
        DatabaseOneTimePasswords::Postgresql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return Vec::new();
          };
          auth_otp_schema::authotp
            .select(auth_otp_schema::code)
            .filter(auth_otp_schema::name.eq(username).and(auth_otp_schema::locked.eq(false)))
            .get_results(&mut db_connection)
            .optional()
        }
        DatabaseOneTimePasswords::MySql(pool) => {
          let Ok(mut db_connection) = pool.get() else {
            return Vec::new();
          };
          auth_otp_schema::authotp
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
