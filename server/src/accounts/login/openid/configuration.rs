use crate::accounts::login::openid::db_oidc::OpenIdClient;
use crate::accounts::login::openid::OIDC_AUTH_RETURN_PATH;
use std::error::Error;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct OIConnectConfiguration {
  provider: OIConnectEndpoint,
  client_id: String,
  client_secret: String,
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum WellKnown {
  Apple,
  Facebook,
  Google,
  LinkedIn,
  Microsoft,
  MicrosoftTenant(String),
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(untagged)]
pub enum OIConnectEndpoint {
  WellKnown(WellKnown),
  Custom { url: String, name: String },
}

#[derive(Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum OpenIdRegistration {
  Closed,
  Invite,
  Open,
}

impl OIConnectConfiguration {
  pub async fn create_oidc_client(self, server_name: &str) -> Result<(String, OpenIdClient), Box<dyn Error + Send + Sync>> {
    let (url, name) = self.provider.into_url_and_name();
    let provider_metadata = openidconnect::core::CoreProviderMetadata::discover_async(
      openidconnect::IssuerUrl::new(url).map_err(|e| format!("Failed to get OpenID Connect provider URL: {}", e))?,
      openidconnect::reqwest::async_http_client,
    )
    .await
    .map_err(|e| format!("Failed to get OpenID Connect provider information: {:?}", e))?;
    let issuer = provider_metadata.issuer().url().to_string();
    let client = openidconnect::core::CoreClient::from_provider_metadata(
      provider_metadata,
      openidconnect::ClientId::new(self.client_id),
      Some(openidconnect::ClientSecret::new(self.client_secret)),
    )
    .set_redirect_uri(
      openidconnect::RedirectUrl::new(format!("https://{}/{}", server_name, OIDC_AUTH_RETURN_PATH))
        .map_err(|e| format!("Failed to create OpenID callback URL: {}", e))?,
    );
    Ok((issuer, OpenIdClient { name, client }))
  }
}

impl OIConnectEndpoint {
  fn into_url_and_name(self) -> (String, String) {
    match self {
      OIConnectEndpoint::Custom { url, name } => (url, name),
      OIConnectEndpoint::WellKnown(WellKnown::Apple) => ("https://appleid.apple.com".to_string(), "Apple".to_string()),
      OIConnectEndpoint::WellKnown(WellKnown::Facebook) => ("https://www.facebook.com".to_string(), "Facebook".to_string()),
      OIConnectEndpoint::WellKnown(WellKnown::Google) => ("https://accounts.google.com".to_string(), "Google".to_string()),
      OIConnectEndpoint::WellKnown(WellKnown::LinkedIn) => ("https://www.linkedin.com".to_string(), "LinkedIn".to_string()),
      OIConnectEndpoint::WellKnown(WellKnown::Microsoft) => {
        ("https://login.microsoftonline.com/9188040d-6c67-4c5b-b112-36a304b66dad/v2.0".to_string(), "Microsoft".to_owned())
      }
      OIConnectEndpoint::WellKnown(WellKnown::MicrosoftTenant(tenant)) => {
        (format!("https://login.microsoftonline.com/{}/v2.0", tenant), "Microsoft".to_owned())
      }
    }
  }
}
