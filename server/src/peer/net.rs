pub const PATH_START: &str = "/api/server/start/v1";
pub const PATH_FINISH: &str = "/api/server/finish/v1";
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerClaim<S: AsRef<str>> {
  pub exp: usize,
  pub name: S,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerHttpRequestBody<S: AsRef<str>> {
  pub token: S,
  pub server: S,
}
