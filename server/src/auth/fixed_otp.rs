struct FixedOTPs {
  secrets: std::collections::HashMap<String, String>,
}
/// Create a simple OTP store
pub fn new(secrets: std::collections::HashMap<String, String>) -> Result<std::sync::Arc<dyn crate::auth::AuthProvider>, String> {
  Ok(std::sync::Arc::new(FixedOTPs { secrets }))
}
#[async_trait::async_trait]
impl crate::auth::OTPStore for FixedOTPs {
  async fn is_locked(&self, username: &str) -> puzzleverse_core::AccountLockState {
    if self.secrets.contains_key(username) {
      puzzleverse_core::AccountLockState::PermanentlyUnlocked
    } else {
      puzzleverse_core::AccountLockState::Unknown
    }
  }

  async fn lock(&self, _: &str, _: bool) -> bool {
    false
  }

  async fn secret(self: &Self, username: &str) -> Vec<String> {
    self.secrets.get(username).cloned().into_iter().collect()
  }
}
