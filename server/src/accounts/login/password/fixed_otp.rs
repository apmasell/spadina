use crate::accounts::login::password::otp::OneTimePasswordStore;
use std::collections::BTreeMap;
use std::future::Future;

pub struct FixedOneTimePassword(BTreeMap<String, String>);
impl From<BTreeMap<String, String>> for FixedOneTimePassword {
  fn from(value: BTreeMap<String, String>) -> Self {
    FixedOneTimePassword(value)
  }
}
impl FromIterator<(String, String)> for FixedOneTimePassword {
  fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
    FixedOneTimePassword(iter.into_iter().collect())
  }
}
impl OneTimePasswordStore for FixedOneTimePassword {
  fn lock_account(&self, _username: &str, _locked: bool) -> impl Future<Output = Option<bool>> + Send {
    async move { None }
  }

  fn secret(&self, username: &str) -> impl Future<Output = Vec<String>> + Send {
    async move { self.0.get(username).cloned().into_iter().collect() }
  }
}
