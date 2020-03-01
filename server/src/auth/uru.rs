use diesel::prelude::*;
struct UruDatabase {
  connection: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>,
}
/// Access a Myst Online: Uru Live database for accounts
pub(crate) fn new(database_url: String) -> Result<Box<dyn crate::auth::AuthProvider>, String> {
  let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new(database_url);
  Ok(Box::new(UruDatabase {
    connection: diesel::r2d2::Pool::builder().build(manager).map_err(|e| format!("Failed to create Uru database connection: {}", e))?,
  }))
}
#[async_trait::async_trait]
impl crate::auth::Password for UruDatabase {
  async fn check(self: &Self, username: &str, password: &str, _: &crate::database::Database) -> bool {
    match self.connection.get() {
      Ok(mut db_connection) => {
        #[derive(diesel::QueryableByName, PartialEq, Debug)]
        struct UruPassword {
          #[diesel(sql_type = diesel::sql_types::Text)]
          pub pass_hash: String,
        }
        match diesel::sql_query(
          "SELECT \"PassHash\" as pass_hash FROM \"Accounts\" WHERE \"Login\" = $1 AND GET_BIT(\"AcctFlags\"::bit(32), 16) == 0",
        )
        .bind::<diesel::sql_types::Text, _>(username)
        .load::<UruPassword>(&mut db_connection)
        {
          Ok(results) => {
            use sha1::Digest;
            let mut digest = sha1::Sha1::new();
            digest.update(password.as_bytes());
            let hash = base16ct::lower::encode_string(digest.finalize().as_slice());
            results.iter().any(|h| h.pass_hash == hash)
          }
          Err(diesel::result::Error::NotFound) => false,
          Err(e) => {
            eprintln!("Failed to fetch Uru for {}: {}", username, e);
            false
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch Uru for {}: {}", username, e);
        false
      }
    }
  }

  async fn is_locked(&self, username: &str, _: &crate::database::Database) -> spadina_core::access::AccountLockState {
    match self.connection.get() {
      Ok(mut db_connection) => {
        #[derive(diesel::QueryableByName, PartialEq, Debug)]
        struct IsLocked {
          #[diesel(sql_type = diesel::sql_types::Bool)]
          pub locked: bool,
        }
        match diesel::sql_query("SELECT GET_BIT(\"AcctFlags\"::bit(32), 16) <> 0 as locked FROM \"Accounts\" WHERE \"Login\" = $1")
          .bind::<diesel::sql_types::Text, _>(username)
          .load::<IsLocked>(&mut db_connection)
        {
          Ok(results) => results
            .get(0)
            .map(|r| {
              if r.locked {
                spadina_core::access::AccountLockState::PermanentlyLocked
              } else {
                spadina_core::access::AccountLockState::PermanentlyUnlocked
              }
            })
            .unwrap_or(spadina_core::access::AccountLockState::Unknown),
          Err(diesel::result::Error::NotFound) => spadina_core::access::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to fetch Uru for {}: {}", username, e);
            spadina_core::access::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch Uru for {}: {}", username, e);
        spadina_core::access::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, _: &str, _: bool, _: &crate::database::Database) -> spadina_core::UpdateResult {
    spadina_core::UpdateResult::NotAllowed
  }
}
