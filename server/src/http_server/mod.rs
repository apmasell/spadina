use crate::accounts::login::Login;
use crate::accounts::AuthResult;
use crate::database::Database;
use crate::directory::Directory;
use crate::peer;
use crate::socket_entity::open_websocket;
use futures::future::BoxFuture;
use futures::FutureExt;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::header::{CONTENT_TYPE, ETAG};
use hyper::http;
use hyper::http::{Request, Response, StatusCode};
use hyper::service::Service;
use std::sync::Arc;

pub mod calendar;
pub mod jwt;
mod public_key_login;
pub mod ssl;
pub mod websocket;

#[derive(Clone)]
pub(crate) struct WebServer {
  /// The authentication provider that can determine what users can get a JWT to log in
  database: Database,
  pub directory: Directory,
  registry: Arc<prometheus_client::registry::Registry>,
}

impl WebServer {
  pub fn new(directory: Directory, database: Database) -> Self {
    let mut registry = Default::default();
    crate::metrics::register(&mut registry);
    WebServer { database, directory, registry: Arc::new(registry) }
  }
}

impl Service<Request<Incoming>> for WebServer {
  type Response = Response<Full<Bytes>>;
  type Error = http::Error;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    let server = self.clone();
    async move {
      match (req.method(), req.uri().path()) {
        // For the root, provide an HTML PAGE with the web client
        (&http::Method::GET, "/") => Response::builder().header(CONTENT_TYPE, "text/html; charset=utf-8").body(crate::html::create_main().into()),
        (&http::Method::GET, "/metrics") => {
          let mut encoded = String::new();
          prometheus_client::encoding::text::encode(&mut encoded, &server.registry).unwrap();
          Response::builder().header(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8").body(encoded.into())
        }
        (&http::Method::GET, "/peers") => {
          #[derive(serde::Serialize)]
          struct Labels<'a> {
            #[serde(rename = "__spadina_discovering_instance")]
            instance: &'a str,
          }
          #[derive(serde::Serialize)]
          struct ServiceDiscovery<'a> {
            targets: Vec<Arc<str>>,
            labels: Labels<'a>,
          }
          let result = match server.directory.peers().await {
            Ok(targets) => targets.await.map_err(|_| ()).and_then(|targets| {
              serde_json::to_vec(&[ServiceDiscovery { targets, labels: Labels { instance: &server.directory.access_management.server_name } }])
                .map_err(|_| ())
            }),
            Err(()) => Err(()),
          };
          match result {
            Ok(buffer) => Response::builder().header(CONTENT_TYPE, "application/json").body(buffer.into()),
            Err(()) => Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body("".into()),
          }
        }
        // Describe what authentication scheme is used
        (&http::Method::GET, spadina_core::net::server::AUTH_METHOD_PATH) => {
          let scheme = server.directory.access_management.accounts.scheme();
          match serde_json::to_string(&scheme) {
            Err(e) => {
              crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
              eprintln!("Failed to serialise authentication scheme: {}", e);
              Response::builder().status(StatusCode::BAD_REQUEST).body(Full::new(Bytes::new()))
            }
            Ok(auth_json) => Response::builder().status(StatusCode::OK).header(CONTENT_TYPE, "application/json").body(auth_json.into()),
          }
        }
        (&http::Method::GET, spadina_core::net::server::CALENDAR_PATH) => {
          calendar::build_calendar(req.uri().query(), &server.database, &server.directory)
        }
        (&http::Method::GET, "/spadina.svg") => etag_request("image/svg+xml", include_bytes!("../../../spadina.svg"), req),
        // Deliver the webclient
        #[cfg(feature = "wasm-client")]
        (&http::Method::GET, "/spadina-client_bg.wasm") => etag_request("application/wasm", include_bytes!("../spadina-client_bg.wasm"), req),
        #[cfg(feature = "wasm-client")]
        (&http::Method::GET, "/spadina-client.js") => etag_request("text/javascript", include_bytes!("../spadina-client.js"), req),
        // Handle a new player connection by upgrading to a web socket
        (&http::Method::GET, spadina_core::net::server::CLIENT_V1_PATH) => open_websocket::<crate::client::Client>(req, &server),
        // Handle a new server connection by upgrading to a web socket
        (&http::Method::GET, peer::net::PATH_FINISH) => open_websocket::<peer::Peer>(req, &server),
        (&http::Method::POST, spadina_core::net::server::CLIENT_KEY_PATH) => public_key_login::handle(req, &server).await,
        // Handle a request by a peer server for a connection back
        (&http::Method::POST, peer::net::PATH_START) => peer::handshake::handle(req, &server).await,
        // For other URLs, see if the authentication mechanism is prepared to deal with them
        _ => match server.directory.access_management.accounts.http_handle(req).await {
          AuthResult::Failure => {
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body("Internal server error during authentication".into())
          }
          AuthResult::NotHandled => Response::builder().status(StatusCode::NOT_FOUND).body("Not Found".into()),
          AuthResult::Page(page) => page,
          AuthResult::SendToken(name) => {
            jwt::encode_jwt_response(&jwt::PlayerClaim { exp: jwt::expiry_time(3600), name }, &server.directory.access_management)
          }
          AuthResult::RedirectToken(name) => {
            jwt::encode_jwt_redirect(&jwt::PlayerClaim { exp: jwt::expiry_time(3600), name }, &server.directory.access_management)
          }
        },
      }
    }
    .boxed()
  }
}

fn etag_request(content_type: &'static str, contents: &'static [u8], req: Request<Incoming>) -> http::Result<Response<Full<Bytes>>> {
  if req.headers().get("If-None-Match").map(|tag| tag.to_str().ok()).flatten().map(|v| v == git_version::git_version!()).unwrap_or(false) {
    Response::builder().status(StatusCode::NOT_MODIFIED).body(Default::default())
  } else {
    Response::builder()
      .status(StatusCode::OK)
      .header(CONTENT_TYPE, content_type)
      .header(ETAG, git_version::git_version!())
      .body(Bytes::from_static(contents).into())
  }
}
pub async fn aggregate<T: serde::de::DeserializeOwned>(req: Request<Incoming>) -> Result<T, http::Result<Response<Full<Bytes>>>> {
  match req.into_body().collect().await {
    Err(e) => {
      crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
      eprintln!("Failed to aggregate body: {}", e);
      Err(Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into()))
    }
    Ok(whole_body) => match serde_json::from_reader::<_, T>(&*whole_body.to_bytes()) {
      Err(e) => Err(Response::builder().status(StatusCode::BAD_REQUEST).body(e.to_string().into())),
      Ok(data) => Ok(data),
    },
  }
}
