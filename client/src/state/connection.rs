use futures::SinkExt;
use spadina_core::net::ToWebMessage;
pub type AuthKey = std::sync::Arc<(String, openssl::pkey::PKey<openssl::pkey::Private>, Vec<u8>)>;
#[pin_project::pin_project(project = ConnectionStateProjection)]
pub enum ConnectionState {
  Idle,
  Active(#[pin] tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>),
}
#[derive(Debug)]
pub enum ServerRequest {
  TryLogin { insecure: bool, player: String, server: String, key: Option<AuthKey> },
  LoginPassword { insecure: bool, player: String, password: String, server: String },
  LoginSocket { path: std::path::PathBuf, player: String, is_superuser: bool },
  Deliver(spadina_core::ClientRequest<String>),
}
#[derive(Clone, Debug)]
pub enum Auth {
  Auto,
  PublicKey(AuthKey),
  Password(String),
}

async fn open_websocket(insecure: bool, authority: &http::uri::Authority, token: String) -> Result<ConnectionState, String> {
  eprintln!("Got authorization token: {}", &token);
  match hyper::Uri::builder()
    .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
    .path_and_query(spadina_core::net::CLIENT_V1_PATH)
    .authority(authority.clone())
    .build()
  {
    Ok(uri) => {
      let connector = hyper_tls::HttpsConnector::new();
      let client = hyper::Client::builder().build::<_, hyper::Body>(connector);
      use rand::RngCore;

      match client
        .request(
          spadina_core::capabilities::add_header(hyper::Request::get(uri))
            .version(http::Version::HTTP_11)
            .header(http::header::CONNECTION, "upgrade")
            .header(http::header::SEC_WEBSOCKET_VERSION, "13")
            .header(http::header::SEC_WEBSOCKET_PROTOCOL, "spadina")
            .header(http::header::UPGRADE, "websocket")
            .header(http::header::SEC_WEBSOCKET_KEY, format!("pv{}", (&mut rand::thread_rng()).next_u64()))
            .header(http::header::AUTHORIZATION, format!("Bearer {}", token))
            .body(hyper::Body::empty())
            .unwrap(),
        )
        .await
      {
        Err(e) => Err(e.to_string()),
        Ok(response) => {
          if response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
            match hyper::upgrade::on(response).await {
              Ok(upgraded) => Ok(ConnectionState::Active(
                tokio_tungstenite::WebSocketStream::from_raw_socket(
                  spadina_core::net::IncomingConnection::Upgraded(upgraded),
                  tokio_tungstenite::tungstenite::protocol::Role::Client,
                  None,
                )
                .await,
              )),
              Err(e) => Err(e.to_string()),
            }
          } else {
            use bytes::buf::Buf;
            let status = response.status();
            match hyper::body::aggregate(response).await {
              Err(e) => Err(format!("Failed to connect to {} {}: {}", authority, status, e)),
              Ok(buf) => {
                Err(format!("Failed to connect to {} {}: {}", authority, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
              }
            }
          }
        }
      }
    }
    Err(e) => Err(e.to_string()),
  }
}
async fn open_named_websocket(socket: std::path::PathBuf, player: String, is_superuser: bool) -> Result<ConnectionState, String> {
  match tokio::net::UnixStream::connect(&socket).await {
    Ok(socket) => match tokio_tungstenite::client_async(
      spadina_core::capabilities::add_header(hyper::Request::get(format!("{}/{}", player, is_superuser)))
        .version(http::Version::HTTP_11)
        .header(http::header::CONNECTION, "upgrade")
        .header(http::header::SEC_WEBSOCKET_VERSION, "13")
        .header(http::header::SEC_WEBSOCKET_PROTOCOL, "spadina")
        .header(http::header::UPGRADE, "websocket")
        .body(())
        .unwrap(),
      spadina_core::net::IncomingConnection::Unix(socket),
    )
    .await
    {
      Err(e) => Err(e.to_string()),
      Ok((socket, _)) => Ok(ConnectionState::Active(socket)),
    },
    Err(e) => Err(e.to_string()),
  }
}
#[cfg(feature = "kerberos")]
async fn make_kerberos_request(server: &str, insecure: bool, username: &str) -> Result<ConnectionState, String> {
  match server.parse::<http::uri::Authority>() {
    Ok(authority) => {
      let server_principal = match hyper::Uri::builder()
        .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
        .path_and_query(spadina_core::net::KERBEROS_PRINCIPAL_PATH)
        .authority(authority.clone())
        .build()
      {
        Ok(uri) => {
          let connector = hyper_tls::HttpsConnector::new();
          let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

          match client.request(hyper::Request::get(uri).body(hyper::Body::empty()).unwrap()).await {
            Err(e) => Err(e.to_string()),
            Ok(response) => {
              use bytes::buf::Buf;
              if response.status() == http::StatusCode::OK {
                std::str::from_utf8(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk())
                  .map_err(|e| e.to_string())
                  .map(|s| s.to_owned())
              } else {
                let status = response.status();
                match hyper::body::aggregate(response).await {
                  Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                  Ok(buf) => {
                    Err(format!("Failed to connect to {} {}: {}", server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                  }
                }
              }
            }
          }
        }
        Err(e) => Err(e.to_string()),
      }?;
      let at_index = server_principal.find('@').ok_or(format!("Server principal is malformed: {}", server_principal))?;
      let player_principal = format!("{}@{}", username, &server_principal[at_index..]);
      let (_, kerberos_token) = cross_krb5::ClientCtx::new(cross_krb5::InitiateFlags::empty(), Some(&player_principal), &server_principal, None)
        .map_err(|e| e.to_string())?;

      let mut token_data = Vec::new();
      token_data.copy_from_slice(&*kerberos_token);

      let token: String = match hyper::Uri::builder()
        .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
        .path_and_query(spadina_core::net::KERBEROS_AUTH_PATH)
        .authority(authority.clone())
        .build()
      {
        Ok(uri) => {
          let connector = hyper_tls::HttpsConnector::new();
          let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

          match client.request(hyper::Request::post(uri).body(token_data.into()).unwrap()).await {
            Err(e) => Err(e.to_string()),
            Ok(response) => {
              use bytes::buf::Buf;
              if response.status() == http::StatusCode::OK {
                std::str::from_utf8(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk())
                  .map_err(|e| e.to_string())
                  .map(|s| s.to_string())
              } else {
                let status = response.status();
                match hyper::body::aggregate(response).await {
                  Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                  Ok(buf) => {
                    Err(format!("Failed to connect to {} {}: {}", server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                  }
                }
              }
            }
          }
        }
        Err(e) => Err(e.to_string()),
      }?;
      open_websocket(insecure, &authority, token).await
    }
    Err(e) => Err(e.to_string()),
  }
}
#[cfg(not(feature = "kerberos"))]
async fn make_kerberos_request(server: &str, insecure: bool, username: &str) -> Result<ConnectionState, String> {
  Err(std::borrow::Cow::Borrowed(
    "This client was not built with Kerberos support but the server requires it. Please download a client with Kerberos support built-in.",
  ))
}
async fn make_openid_request(server: &str, insecure: bool, username: &str) -> Result<ConnectionState, String> {
  match server.parse::<http::uri::Authority>() {
    Ok(authority) => {
      let response: spadina_core::auth::OpenIdConnectInformation = match hyper::Uri::builder()
        .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
        .path_and_query(format!(
          "{}?{}",
          spadina_core::net::OIDC_AUTH_START_PATH,
          form_urlencoded::Serializer::new(String::new()).append_pair("player", username).finish()
        ))
        .authority(authority.clone())
        .build()
      {
        Ok(uri) => {
          let connector = hyper_tls::HttpsConnector::new();
          let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

          match client.request(hyper::Request::get(uri).body(hyper::Body::empty()).unwrap()).await {
            Err(e) => Err(e.to_string()),
            Ok(response) => {
              use bytes::buf::Buf;
              if response.status() == http::StatusCode::OK {
                serde_json::from_slice(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk()).map_err(|e| e.to_string())
              } else {
                let status = response.status();
                match hyper::body::aggregate(response).await {
                  Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                  Ok(buf) => {
                    Err(format!("Failed to connect to {} {}: {}", server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                  }
                }
              }
            }
          }
        }
        Err(e) => Err(e.to_string()),
      }?;
      webbrowser::open(&response.authorization_url).map_err(|e| format!("Failed to open web browser: {}", e))?;
      let token: String = match hyper::Uri::builder()
        .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
        .path_and_query(format!(
          "{}?{}",
          spadina_core::net::OIDC_AUTH_FINISH_PATH,
          form_urlencoded::Serializer::new(String::new()).append_pair("request_id", &response.request_id).finish()
        ))
        .authority(authority.clone())
        .build()
      {
        Ok(uri) => {
          let connector = hyper_tls::HttpsConnector::new();
          let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

          match client.request(hyper::Request::get(uri).body(hyper::Body::empty()).unwrap()).await {
            Err(e) => Err(e.to_string()),
            Ok(response) => {
              use bytes::buf::Buf;
              if response.status() == http::StatusCode::OK {
                std::str::from_utf8(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk())
                  .map_err(|e| e.to_string())
                  .map(|s| s.to_string())
              } else {
                let status = response.status();
                match hyper::body::aggregate(response).await {
                  Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                  Ok(buf) => {
                    Err(format!("Failed to connect to {} {}: {}", server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                  }
                }
              }
            }
          }
        }
        Err(e) => Err(e.to_string()),
      }?;
      open_websocket(insecure, &authority, token).await
    }
    Err(e) => Err(e.to_string()),
  }
}
impl ConnectionState {
  pub(crate) async fn process(
    &mut self,
    request: ServerRequest,
    server_name: &std::sync::Arc<std::sync::Mutex<String>>,
  ) -> Result<bool, std::borrow::Cow<'static, str>> {
    Ok(match request {
      ServerRequest::TryLogin { insecure, server, player, key } => {
        async fn make_request(server: &str, insecure: bool) -> Result<spadina_core::auth::AuthScheme, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(spadina_core::net::AUTH_METHOD_PATH)
                .authority(authority.clone())
                .build()
              {
                Ok(uri) => {
                  let connector = hyper_tls::HttpsConnector::new();
                  let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

                  match client.request(hyper::Request::get(uri).body(hyper::Body::empty()).unwrap()).await {
                    Err(e) => Err(e.to_string()),
                    Ok(response) => {
                      use bytes::buf::Buf;
                      if response.status() == http::StatusCode::OK {
                        serde_json::from_slice(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk()).map_err(|e| e.to_string())
                      } else {
                        let status = response.status();
                        match hyper::body::aggregate(response).await {
                          Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                          Ok(buf) => Err(format!(
                            "Failed to connect to {} {}: {}",
                            server,
                            status,
                            std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),
                          )),
                        }
                      }
                    }
                  }
                }
                Err(e) => Err(e.to_string()),
              }
            }
            Err(e) => Err(e.to_string()),
          }
        }
        async fn try_private_key(server: &str, insecure: bool, player: &str, key: AuthKey) -> Option<ConnectionState> {
          use bytes::buf::Buf;
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(spadina_core::net::CLIENT_NONCE_PATH)
                .authority(authority.clone())
                .build()
              {
                Ok(uri) => {
                  let connector = hyper_tls::HttpsConnector::new();
                  let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

                  match client.request(hyper::Request::post(&uri).body(serde_json::to_vec(player).ok()?.into()).unwrap()).await {
                    Err(e) => {
                      eprintln!("Failed to fetch public key nonce: {}", e);
                      None
                    }
                    Ok(response) => match response.status() {
                      http::StatusCode::OK => {
                        let nonce = hyper::body::aggregate(response).await.ok()?;
                        let mut signer = openssl::sign::Signer::new(openssl::hash::MessageDigest::sha256(), &key.1).ok()?;
                        signer.update(nonce.chunk()).ok()?;
                        match client
                          .request(
                            hyper::Request::post(uri)
                              .body(
                                serde_json::to_vec(&spadina_core::auth::AuthPublicKey {
                                  name: key.0.as_str(),
                                  nonce: std::str::from_utf8(nonce.chunk()).ok()?,
                                  signature: signer.sign_to_vec().ok()?,
                                })
                                .ok()?
                                .into(),
                              )
                              .ok()?,
                          )
                          .await
                        {
                          Err(e) => {
                            eprintln!("Failed to do public key authentication: {}", e);
                            None
                          }
                          Ok(response) => match response.status() {
                            http::StatusCode::OK => open_websocket(
                              insecure,
                              &authority,
                              std::str::from_utf8(hyper::body::aggregate(response).await.ok()?.chunk()).ok()?.to_string(),
                            )
                            .await
                            .ok(),
                            status => {
                              format!("Failed to do public key authentication to {}: {}", server, status);
                              None
                            }
                          },
                        }
                      }
                      status => {
                        format!("Failed to connect to {}: {}", server, status);
                        None
                      }
                    },
                  }
                }
                Err(e) => {
                  eprintln!("Failed to construct URL: {}", e);
                  None
                }
              }
            }
            Err(e) => {
              eprintln!("Failed to construct URL: {}", e);
              None
            }
          }
        }
        let mut socket_path = std::path::PathBuf::new();
        socket_path.push(&server);
        socket_path.push(format!("spadina-{}.socket", &player));
        if std::fs::metadata(&socket_path).is_ok() {
          match tokio::net::UnixStream::connect(socket_path).await {
            Ok(socket) => {
              *self = ConnectionState::Active(
                tokio_tungstenite::WebSocketStream::from_raw_socket(
                  spadina_core::net::IncomingConnection::Unix(socket),
                  tokio_tungstenite::tungstenite::protocol::Role::Client,
                  None,
                )
                .await,
              );
              false
            }
            Err(e) => Err(std::borrow::Cow::Owned(e.to_string()))?,
          }
        } else {
          let scheme = make_request(&server, insecure).await?;
          let connection = match key {
            Some(key) => try_private_key(&server, insecure, &player, key).await,
            None => None,
          };
          {
            let mut server_name = server_name.lock().unwrap();
            server_name.clear();
            server_name.push_str(&server);
          }
          if let Some(connection) = connection {
            *self = connection;
            false
          } else {
            match scheme {
              spadina_core::auth::AuthScheme::Kerberos => {
                *self = make_kerberos_request(&server, insecure, &player).await?;
                false
              }
              spadina_core::auth::AuthScheme::OpenIdConnect => {
                *self = make_openid_request(&server, insecure, &player).await?;
                false
              }
              spadina_core::auth::AuthScheme::Password => true,
            }
          }
        }
      }

      ServerRequest::LoginPassword { insecure, server, player, password } => {
        async fn make_request(server: &str, insecure: bool, username: &str, password: &str) -> Result<ConnectionState, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              let token: String = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(spadina_core::net::PASSWORD_AUTH_PATH)
                .authority(authority.clone())
                .build()
              {
                Ok(uri) => {
                  let connector = hyper_tls::HttpsConnector::new();
                  let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

                  match client
                    .request(
                      hyper::Request::post(uri)
                        .body(hyper::Body::from(
                          serde_json::to_vec(&spadina_core::auth::PasswordRequest { username, password }).map_err(|e| e.to_string())?,
                        ))
                        .unwrap(),
                    )
                    .await
                  {
                    Err(e) => Err(e.to_string()),
                    Ok(response) => {
                      use bytes::buf::Buf;
                      if response.status() == http::StatusCode::OK {
                        std::str::from_utf8(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk())
                          .map_err(|e| e.to_string())
                          .map(|s| s.to_string())
                      } else {
                        let status = response.status();
                        match hyper::body::aggregate(response).await {
                          Err(e) => Err(format!("Failed to connect to {} {}: {}", server, status, e)),
                          Ok(buf) => Err(format!(
                            "Failed to connect to {} {}: {}",
                            server,
                            status,
                            std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),
                          )),
                        }
                      }
                    }
                  }
                }
                Err(e) => Err(e.to_string()),
              }?;
              open_websocket(insecure, &authority, token).await
            }
            Err(e) => Err(e.to_string()),
          }
        }
        *self = make_request(&server, insecure, &player, &password).await?;
        false
      }
      ServerRequest::LoginSocket { path, player, is_superuser } => {
        *self = open_named_websocket(path, player, is_superuser).await?;
        false
      }
      ServerRequest::Deliver(request) => {
        if let ConnectionState::Active(connection) = self {
          connection.send(request.as_wsm()).await.map_err(|e| e.to_string())?;
        }
        false
      }
    })
  }
  pub(crate) async fn deliver(&mut self, request: spadina_core::ClientRequest<String>) {
    if let ConnectionState::Active(connection) = self {
      if let Err(e) = connection.send(request.as_wsm()).await {
        eprintln!("Error sending request to server: {}", e);
      }
    }
  }
}
impl futures::Stream for ConnectionState {
  type Item = spadina_core::ClientResponse<String>;
  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    match self.project() {
      ConnectionStateProjection::Idle => std::task::Poll::Pending,
      ConnectionStateProjection::Active(connection) => match connection.poll_next(cx) {
        std::task::Poll::Pending => std::task::Poll::Pending,
        std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
        std::task::Poll::Ready(Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(value)))) => match rmp_serde::from_slice(&value) {
          Ok(v) => std::task::Poll::Ready(Some(v)),
          Err(e) => {
            eprintln!("Failed to decode message from server. Mismatched protocols?: {}", e);
            std::task::Poll::Ready(None)
          }
        },
        std::task::Poll::Ready(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(value)))) => match serde_json::from_str(&value) {
          Ok(v) => std::task::Poll::Ready(Some(v)),
          Err(e) => {
            eprintln!("Failed to decode message from server. Mismatched protocols?: {}", e);
            std::task::Poll::Ready(None)
          }
        },
        std::task::Poll::Ready(Some(Ok(_))) => std::task::Poll::Ready(Some(spadina_core::ClientResponse::NoOperation)),
        std::task::Poll::Ready(Some(Err(e))) => {
          eprintln!("Error in connection: {}", e);
          std::task::Poll::Ready(None)
        }
      },
    }
  }
}
