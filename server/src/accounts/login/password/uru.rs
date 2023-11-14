use crate::accounts::login::password::Password;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use sha1::Digest;
use std::error::Error;
use std::future::Future;

/// Access a Myst Online: Uru Live database for accounts
pub struct UruDatabase(Pool<ConnectionManager<PgConnection>>);
#[derive(diesel::QueryableByName, PartialEq, Debug)]
struct Login {
  #[diesel(sql_type = diesel::sql_types::Text)]
  pub login: String,
}
impl UruDatabase {
  pub fn new(database_url: String) -> Result<UruDatabase, Box<dyn Error + Send + Sync>> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Ok(UruDatabase(Pool::builder().build(manager)?))
  }
}
impl Password for UruDatabase {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      match self.0.get() {
        Ok(mut db_connection) => {
          match diesel::sql_query("SELECT \"Login\" AS login FROM \"Accounts\" WHERE \"Login\" = $1 AND GET_BIT(\"AcctFlags\"::bit(32), 16) = 0")
            .bind::<diesel::sql_types::Text, _>(&username)
            .get_result::<Login>(&mut db_connection)
            .optional()
          {
            Ok(None) => None,
            Ok(Some(Login { login })) => Some(login),
            Err(e) => {
              eprintln!("Failed to fetch Uru for {}: {}", &username, e);
              None
            }
          }
        }
        Err(e) => {
          eprintln!("Failed to get connection to fetch Uru for {}: {}", &username, e);
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
      match self.0.get() {
        Ok(mut db_connection) => {
          let mut digest = sha1::Sha1::new();
          digest.update(password.as_bytes());
          let hash = base16ct::lower::encode_string(digest.finalize().as_slice());

          match diesel::sql_query(
            "SELECT \"Login\" AS login FROM \"Accounts\" WHERE \"Login\" = $1 AND \"PassHash\" = $2 AND GET_BIT(\"AcctFlags\"::bit(32), 16) == 0",
          )
          .bind::<diesel::sql_types::Text, _>(&username)
          .bind::<diesel::sql_types::Text, _>(hash)
          .get_result::<Login>(&mut db_connection)
          .optional()
          {
            Ok(value) => value.map(|Login { login }| login),
            Err(e) => {
              eprintln!("Failed to fetch Uru for {}: {}", &username, e);
              None
            }
          }
        }
        Err(e) => {
          eprintln!("Failed to get connection to fetch Uru for {}: {}", &username, e);
          None
        }
      }
    }
  }
}
