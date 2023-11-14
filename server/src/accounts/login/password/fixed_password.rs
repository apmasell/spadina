use crate::accounts::login::password::Password;
use std::collections::BTreeMap;
use std::future::Future;

pub struct FixedPasswords(BTreeMap<String, String>);
impl From<BTreeMap<String, String>> for FixedPasswords {
  fn from(value: BTreeMap<String, String>) -> Self {
    FixedPasswords(value)
  }
}
impl FromIterator<(String, String)> for FixedPasswords {
  fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
    FixedPasswords(iter.into_iter().collect())
  }
}
impl Password for FixedPasswords {
  fn check_and_normalize(&self, username: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      if self.0.contains_key(&username) {
        Some(username)
      } else {
        None
      }
    }
  }

  fn lock_account(&self, _username: &str, _locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move { None }
  }

  fn validate(&self, username: String, password: String) -> impl Future<Output = Option<String>> + Send {
    async move {
      if self.0.get(&username).map(|p| p == &password).unwrap_or(false) {
        Some(username)
      } else {
        None
      }
    }
  }
}
