use crate::accounts::login::Login;
use crate::http_server::{aggregate, jwt, WebServer};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::CONTENT_TYPE;
use hyper::{body::Incoming, Request, Response, StatusCode};
use spadina_core::net::server::auth::AuthPublicKey;

pub async fn handle(req: Request<Incoming>, web_server: &WebServer) -> hyper::http::Result<Response<Full<Bytes>>> {
  let AuthPublicKey { player, fingerprint } = match aggregate::<AuthPublicKey<String>>(req).await {
    Err(response) => return response,
    Ok(request) => request,
  };
  let player = match web_server.directory.access_management.accounts.normalize_username(player).await {
    Ok(player) => player,
    Err(()) => return Response::builder().status(StatusCode::FORBIDDEN).body("Invalid user name".into()),
  };
  let der = match web_server.database.public_key_get(&player, &fingerprint) {
    Ok(Some(der)) => der,
    Ok(None) => return Response::builder().status(StatusCode::NOT_FOUND).body(Default::default()),
    Err(e) => {
      eprintln!("Failed to fetch public keys during authentication: {}", e);
      return Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default());
    }
  };
  let pkey = match openssl::pkey::PKey::public_key_from_der(der.as_slice()) {
    Ok(pkey) => pkey,
    Err(e) => return Response::builder().status(StatusCode::UNPROCESSABLE_ENTITY).body(format!("Certificate is invalid: {}", e).into()),
  };

  let encrypter = match openssl::encrypt::Encrypter::new(&pkey) {
    Ok(encrypter) => encrypter,
    Err(e) => return Response::builder().status(StatusCode::UNPROCESSABLE_ENTITY).body(format!("Failed to build encrypter: {}", e).into()),
  };
  let token = match jwt::encode_jwt(&jwt::PlayerClaim { exp: jwt::expiry_time(3600), name: &player }, &web_server.directory.access_management)
    .map_err(|e| {
      eprintln!("Failed to encode JWT: {}", e);
      Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Default::default())
    }) {
    Ok(token) => token,
    Err(e) => return e,
  };
  let buffer_len = match encrypter.encrypt_len(token.as_bytes()) {
    Ok(buffer_len) => buffer_len,
    Err(e) => return Response::builder().status(StatusCode::UNPROCESSABLE_ENTITY).body(format!("Failed to encrypt: {}", e).into()),
  };
  let mut output = vec![0; buffer_len];
  match encrypter.encrypt(token.as_bytes(), &mut output) {
    Ok(_) => Response::builder().status(StatusCode::OK).header(CONTENT_TYPE, "application/octet-stream").body(output.into()),
    Err(e) => Response::builder().status(StatusCode::UNPROCESSABLE_ENTITY).body(format!("Failed to encrypt: {}", e).into()),
  }
}
