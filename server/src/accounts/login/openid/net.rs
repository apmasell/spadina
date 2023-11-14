#[serde_with::serde_as]
#[derive(serde::Deserialize)]
pub struct OpenIdRegistrationRequest<T: serde::de::DeserializeOwned> {
  #[serde_as(as = "serde_with::json::JsonString")]
  client_type: T,
  invitation: Option<String>,
  player: String,
}
