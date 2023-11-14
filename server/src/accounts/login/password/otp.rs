use crate::accounts::login::password::Password;
use otpauth::TOTP;
use std::future::Future;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait OneTimePasswordStore: Send + Sync {
  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send;
  fn secret(&self, username: &str) -> impl Future<Output = Vec<String>> + Send;
}

impl<T: OneTimePasswordStore> Password for T {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      if self.secret(&username).await.is_empty() {
        None
      } else {
        Some(username)
      }
    }
  }

  fn lock_account(&self, username: &str, locked: bool) -> impl Future<Output = Option<bool>> + Send {
    OneTimePasswordStore::lock_account(self, username, locked)
  }

  fn validate(self: &Self, username: String, password: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      let Ok(code) = u32::from_str(&password) else { return None };
      let Ok(timestamp) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return None;
      };
      if self.secret(&username).await.into_iter().any(|secret| TOTP::new(secret).verify(code, 30, timestamp.as_secs())) {
        Some(username)
      } else {
        None
      }
    }
  }
}
