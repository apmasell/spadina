use diesel::prelude::*;
struct DatabaseOTPs {
  connection: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>,
}
/// Create a simple OTP store
pub fn new(database_url: String) -> Result<std::sync::Arc<dyn crate::auth::AuthProvider>, String> {
  let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new(database_url);
  Ok(std::sync::Arc::new(DatabaseOTPs {
    connection: diesel::r2d2::Pool::builder().build(manager).map_err(|e| format!("Failed to create OTP database connection: {}", e))?,
  }))
}
#[async_trait::async_trait]
impl crate::auth::OTPStore for DatabaseOTPs {
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    use crate::schema::authotp::dsl as auth_otp_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match auth_otp_schema::authotp
          .select(auth_otp_schema::locked)
          .filter(auth_otp_schema::name.eq(username))
          .get_results::<bool>(&mut db_connection)
        {
          Ok(results) => {
            if results.iter().all(|&v| v) {
              puzzleverse_core::AccountLockState::Locked
            } else {
              puzzleverse_core::AccountLockState::Unlocked
            }
          }
          Err(diesel::result::Error::NotFound) => puzzleverse_core::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to fetch OTPs for {}: {}", username, e);
            puzzleverse_core::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OTPs for {}: {}", username, e);
        puzzleverse_core::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, username: &str, locked: bool) -> bool {
    use crate::schema::authotp::dsl as auth_otp_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match diesel::update(auth_otp_schema::authotp.filter(auth_otp_schema::name.eq(username)))
          .set(auth_otp_schema::locked.eq(locked))
          .execute(&mut db_connection)
        {
          Ok(results) => results > 0,
          Err(e) => {
            eprintln!("Failed to update locked OTPs for {}: {}", username, e);
            false
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to locking OTPs for {}: {}", username, e);
        false
      }
    }
  }

  async fn secret(self: &Self, username: &str) -> Vec<String> {
    use crate::schema::authotp::dsl as auth_otp_schema;
    match self.connection.get() {
      Ok(mut db_connection) => {
        match auth_otp_schema::authotp
          .select(auth_otp_schema::code)
          .filter(auth_otp_schema::name.eq(username).and(auth_otp_schema::locked.eq(false)))
          .get_results(&mut db_connection)
        {
          Ok(results) => results,
          Err(diesel::result::Error::NotFound) => vec![],
          Err(e) => {
            eprintln!("Failed to fetch OTPs for {}: {}", username, e);
            vec![]
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OTPs for {}: {}", username, e);
        vec![]
      }
    }
  }
}
