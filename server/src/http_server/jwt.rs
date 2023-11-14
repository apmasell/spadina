use crate::access::AccessManagement;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::LOCATION;
use hyper::{Response, StatusCode};
use jsonwebtoken::{DecodingKey, EncodingKey};
use spadina_core::resource::Resource;

pub struct KeyPair {
  encoding: EncodingKey,
  decoding: DecodingKey,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PlayerClaim<S: AsRef<str>> {
  pub exp: usize,
  pub name: S,
}

impl Default for KeyPair {
  fn default() -> Self {
    let mut jwt_secret = [0; 32];
    openssl::rand::rand_bytes(&mut jwt_secret).expect("Failed to generate JWT");
    KeyPair { encoding: EncodingKey::from_secret(&jwt_secret), decoding: DecodingKey::from_secret(&jwt_secret) }
  }
}
pub fn expiry_time(duration_secs: u64) -> usize {
  (std::time::SystemTime::now() + std::time::Duration::from_secs(duration_secs)).duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as usize
}
pub fn decode_jwt<C: serde::de::DeserializeOwned>(data: &str, auth: &AccessManagement) -> Result<C, hyper::http::Result<Response<Full<Bytes>>>> {
  jsonwebtoken::decode::<C>(data, &auth.jwt_key.decoding, &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256))
    .map(|token| token.claims)
    .map_err(|e| {
      eprintln!("Failed to decode encryption: {}", e);
      Response::builder().status(StatusCode::BAD_REQUEST).body("JWT is invalid".into())
    })
}
pub fn encode_jwt<C: serde::Serialize>(claim: &C, auth: &AccessManagement) -> Result<String, jsonwebtoken::errors::Error> {
  jsonwebtoken::encode(&jsonwebtoken::Header::default(), claim, &auth.jwt_key.encoding)
}
pub fn encode_jwt_response<C: serde::Serialize>(claim: &C, auth: &AccessManagement) -> hyper::http::Result<Response<Full<Bytes>>> {
  let token = match encode_jwt(claim, auth) {
    Ok(token) => token,
    Err(e) => {
      eprintln!("Failed to encode JWT: {}", e);
      return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default());
    }
  };
  match serde_json::to_vec(&token) {
    Ok(token) => Response::builder().status(StatusCode::OK).body(token.into()),
    Err(e) => {
      eprintln!("Failed to serialize JWT: {}", e);
      Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default())
    }
  }
}
pub fn encode_jwt_redirect<S: serde::Serialize + AsRef<str>>(
  claim: &PlayerClaim<S>,
  auth: &AccessManagement,
) -> hyper::http::Result<Response<Full<Bytes>>> {
  match encode_jwt(claim, auth) {
    Ok(token) => Response::builder()
      .status(StatusCode::TEMPORARY_REDIRECT)
      .header(LOCATION, Resource::Login { player: claim.name.as_ref(), server: auth.server_name.as_ref(), token: &token }.to_string())
      .body(Default::default()),

    Err(e) => {
      eprintln!("Failed to encode JWT: {}", e);
      Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default())
    }
  }
}
