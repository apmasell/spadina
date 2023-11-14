use crate::accounts::login::password::Password;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types;
use phpbb_pwhash::{check_hash, CheckHashResult};
use std::error::Error;
use std::future::Future;

pub enum PhpBB {
  Postgresql(Pool<ConnectionManager<PgConnection>>),
  #[cfg(feature = "mysql")]
  MySql(Pool<ConnectionManager<MysqlConnection>>),
}
/// Create a phpBB-backed store
impl PhpBB {
  pub fn new_postgres(database_url: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Ok(PhpBB::Postgresql(Pool::builder().build(manager)?))
  }
  #[cfg(feature = "mysql")]
  pub fn new_mysql(database_url: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    Ok(PhpBB::MySql(Pool::builder().build(manager)?))
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
    Ok(match self {
      PhpBB::Postgresql(pool) => {
        let mut db_connection = pool.get()?;
        diesel::sql_query("SELECT username_clean, user_password FROM phpbb_users WHERE user_type NOT IN (0, 3) AND username_clean = $1")
          .bind::<sql_types::Text, _>(username)
          .get_result::<Login>(&mut db_connection)
          .optional()?
      }
      #[cfg(feature = "mysql")]
      PhpBB::MySql(pool) => {
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
