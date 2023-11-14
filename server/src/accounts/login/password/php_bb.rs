use crate::accounts::login::password::Password;
use crate::database::connect::DatabaseBackend;
use diesel::prelude::*;
use diesel::sql_types;
use phpbb_pwhash::{check_hash, CheckHashResult};
use std::error::Error;
use std::future::Future;

pub struct PhpBB(DatabaseBackend);
/// Create a phpBB-backed store
impl From<DatabaseBackend> for PhpBB {
  fn from(value: DatabaseBackend) -> Self {
    PhpBB(value)
  }
}
#[derive(diesel::QueryableByName, PartialEq, Debug)]
struct Login {
  #[diesel(sql_type = diesel::sql_types::Text)]
  pub username_clean: String,
  #[diesel(sql_type = diesel::sql_types::Text)]
  pub user_password: String,
}

impl PhpBB {
  fn get_login(&self, username: &str) -> Result<Option<Login>, Box<dyn Error + Send + Sync>> {
    Ok(match &self.0 {
      DatabaseBackend::SQLite(pool) => {
        let mut db_connection = pool.get()?;
        diesel::sql_query("SELECT username_clean, user_password FROM phpbb_users WHERE user_type NOT IN (0, 3) AND username_clean = ?")
          .bind::<sql_types::Text, _>(username)
          .get_result::<Login>(&mut db_connection)
          .optional()?
      }
      #[cfg(feature = "postgres")]
      DatabaseBackend::PostgreSQL(pool) => {
        let mut db_connection = pool.get()?;
        diesel::sql_query("SELECT username_clean, user_password FROM phpbb_users WHERE user_type NOT IN (0, 3) AND username_clean = $1")
          .bind::<sql_types::Text, _>(username)
          .get_result::<Login>(&mut db_connection)
          .optional()?
      }
      #[cfg(feature = "mysql")]
      DatabaseBackend::MySql(pool) => {
        let mut db_connection = pool.get()?;
        diesel::sql_query("SELECT username_clean, user_password FROM phpbb_users WHERE user_type NOT IN (0, 3) AND username_clean = ?")
          .bind::<sql_types::Text, _>(username)
          .get_result::<Login>(&mut db_connection)
          .optional()?
      }
    })
  }
}

impl Password for PhpBB {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      match self.get_login(&username) {
        Ok(None) => None,
        Ok(Some(Login { username_clean, .. })) => Some(username_clean),
        Err(e) => {
          eprintln!("Failed to check phpBB password for {}: {}", &username, e);
          None
        }
      }
    }
  }

  fn lock_account(&self, _username: &str, _locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move { None }
  }

  fn validate(&self, username: String, password: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      match self.get_login(&username) {
        Ok(None) => None,
        Ok(Some(Login { username_clean, user_password })) => {
          if check_hash(&user_password, &password) == CheckHashResult::Valid {
            Some(username_clean)
          } else {
            None
          }
        }
        Err(e) => {
          eprintln!("Failed to check phpBB password for {}: {}", &username, e);
          None
        }
      }
    }
  }
}
