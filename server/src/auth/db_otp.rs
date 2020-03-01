use diesel::prelude::*;
struct DatabaseOTPs;
/// Create a simple OTP store
pub(crate) fn new() -> Result<Box<dyn crate::auth::AuthProvider>, String> {
  Ok(Box::new(DatabaseOTPs))
}
#[async_trait::async_trait]
impl crate::auth::OTPStore for DatabaseOTPs {
  async fn is_locked(&self, username: &str, connection: &crate::database::Database) -> spadina_core::access::AccountLockState {
    use crate::database::schema::authotp::dsl as auth_otp_schema;
    match connection.get() {
      Ok(mut db_connection) => {
        match auth_otp_schema::authotp
          .select(auth_otp_schema::locked)
          .filter(auth_otp_schema::name.eq(username))
          .get_results::<bool>(&mut db_connection)
        {
          Ok(results) => {
            if results.iter().all(|&v| v) {
              spadina_core::access::AccountLockState::Locked
            } else {
              spadina_core::access::AccountLockState::Unlocked
            }
          }
          Err(diesel::result::Error::NotFound) => spadina_core::access::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to fetch OTPs for {}: {}", username, e);
            spadina_core::access::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to fetch OTPs for {}: {}", username, e);
        spadina_core::access::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, username: &str, locked: bool, connection: &crate::database::Database) -> spadina_core::UpdateResult {
    use crate::database::schema::authotp::dsl as auth_otp_schema;
    match connection.get() {
      Ok(mut db_connection) => {
        match diesel::update(auth_otp_schema::authotp.filter(auth_otp_schema::name.eq(username)))
          .set(auth_otp_schema::locked.eq(locked))
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
            eprintln!("Failed to update locked OTPs for {}: {}", username, e);
            spadina_core::UpdateResult::InternalError
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to locking OTPs for {}: {}", username, e);
        spadina_core::UpdateResult::InternalError
      }
    }
  }

  async fn secret(self: &Self, username: &str, connection: &crate::database::Database) -> Vec<String> {
    use crate::database::schema::authotp::dsl as auth_otp_schema;
    match connection.get() {
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
