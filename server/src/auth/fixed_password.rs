struct FixedPasswords {
  secrets: std::collections::HashMap<String, String>,
}
#[async_trait::async_trait]
impl crate::auth::Password for FixedPasswords {
  async fn check(self: &Self, username: &str, password: &str, _: &crate::database::Database) -> bool {
    self.secrets.get(username).map(|p| p == password).unwrap_or(false)
  }

  async fn is_locked(&self, username: &str, _: &crate::database::Database) -> spadina_core::access::AccountLockState {
    if self.secrets.contains_key(username) {
      spadina_core::access::AccountLockState::PermanentlyUnlocked
    } else {
      spadina_core::access::AccountLockState::Unknown
    }
  }

  async fn lock(&self, _: &str, _: bool, _: &crate::database::Database) -> spadina_core::UpdateResult {
    spadina_core::UpdateResult::NotAllowed
  }
}
/// Create a simple password store
pub(crate) fn new(secrets: std::collections::HashMap<String, String>) -> Result<Box<dyn crate::auth::AuthProvider>, String> {
  Ok(Box::new(FixedPasswords { secrets }))
}
