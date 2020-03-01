struct FixedOTPs {
  secrets: std::collections::HashMap<String, String>,
}
/// Create a simple OTP store
pub(crate) fn new(secrets: std::collections::HashMap<String, String>) -> Result<Box<dyn crate::auth::AuthProvider>, String> {
  Ok(Box::new(FixedOTPs { secrets }))
}
#[async_trait::async_trait]
impl crate::auth::OTPStore for FixedOTPs {
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

  async fn secret(self: &Self, username: &str, _: &crate::database::Database) -> Vec<String> {
    self.secrets.get(username).cloned().into_iter().collect()
  }
}
