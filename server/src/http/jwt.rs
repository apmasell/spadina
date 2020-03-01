#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PlayerClaim<S: AsRef<str>> {
  pub exp: usize,
  pub name: S,
}
pub fn create_jwt() -> (jsonwebtoken::EncodingKey, jsonwebtoken::DecodingKey) {
  let mut jwt_secret = [0; 32];
  openssl::rand::rand_bytes(&mut jwt_secret).expect("Failed to generate JWT");
  (jsonwebtoken::EncodingKey::from_secret(&jwt_secret), jsonwebtoken::DecodingKey::from_secret(&jwt_secret))
}
pub fn expiry_time(duration_secs: u64) -> usize {
  (std::time::SystemTime::now() + std::time::Duration::from_secs(duration_secs)).duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as usize
}
