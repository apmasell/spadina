use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthPublicKey<S: AsRef<str>> {
  pub name: S,
  pub nonce: S,
  pub signature: Vec<u8>,
}
/// Authentication mechanisms that the client and server can use
///
/// A client should perform a `GET` request on a server's `/auth` endpoint to get a JSON-encoded version of this struct detailing which authentication scheme to use
#[derive(Serialize, Deserialize, Debug)]
pub enum AuthScheme {
  /// Kerberos/GSSAPI authentication
  Kerberos,
  /// OpenIdConnect authentication using a remote server. The client should send a request to `/oidc?user=`_user_.
  OpenIdConnect,
  /// Simple username and password authentication. The client should send a JSON-serialised version of [PasswordRequest] to the `/password` endpoint
  Password,
}
/// The information provided by the server to do OpenID Connect authentication
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenIdConnectInformation {
  /// The URL the user should be directed to in order to complete authentication
  pub authorization_url: String,
  /// A token that the client should use to pick up the JWT once authentication is complete
  pub request_id: String,
}
/// The data structure for performing a password-authenticated request
#[derive(Serialize, Deserialize, Debug)]
pub struct PasswordRequest<T: AsRef<str>> {
  /// The player's login name
  pub username: T,
  /// The player's raw password; it is the client's responsibility to ensure the channel is encrypted or warn the player
  pub password: T,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublicKey<S: AsRef<str>> {
  pub fingerprint: S,
  pub created: chrono::DateTime<chrono::Utc>,
  pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn compute_fingerprint(der: &[u8]) -> String {
  use sha3::Digest;
  let mut fingerprint = sha3::Sha3_256::new();
  fingerprint.update(der);
  hex::encode(fingerprint.finalize())
}
