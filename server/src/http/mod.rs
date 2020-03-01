use futures::FutureExt;

pub mod jwt;
pub mod ssl;
pub mod websocket;
pub(crate) struct WebServer {
  /// The authentication provider that can determine what users can get a JWT to log in
  authnz: std::sync::Arc<crate::access::AuthNZ>,
  database: std::sync::Arc<crate::database::Database>,
  directory: std::sync::Arc<crate::destination::Directory>,
  jwt_nonce_decoding_key: jsonwebtoken::DecodingKey,
  jwt_nonce_encoding_key: jsonwebtoken::EncodingKey,
  registry: prometheus_client::registry::Registry,
}

impl WebServer {
  pub fn new(
    authnz: std::sync::Arc<crate::access::AuthNZ>,
    directory: std::sync::Arc<crate::destination::Directory>,
    database: std::sync::Arc<crate::database::Database>,
  ) -> Self {
    let (jwt_nonce_encoding_key, jwt_nonce_decoding_key) = jwt::create_jwt();
    let mut registry = Default::default();
    crate::metrics::register(&mut registry);
    WebServer { authnz, directory, jwt_nonce_decoding_key, jwt_nonce_encoding_key, database, registry }
  }
  async fn handle_http_request(self: std::sync::Arc<WebServer>, req: http::Request<hyper::Body>) -> Result<http::Response<hyper::Body>, http::Error> {
    match (req.method(), req.uri().path()) {
      // For the root, provide an HTML PAGE with the web client
      (&http::Method::GET, "/") => {
        http::Response::builder().header("Content-Type", "text/html; charset=utf-8").body(crate::html::create_main().into())
      }
      (&http::Method::GET, "/metrics") => {
        let mut encoded = String::new();
        prometheus_client::encoding::text::encode(&mut encoded, &self.registry).unwrap();
        http::Response::builder().header(hyper::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8").body(encoded.into())
      }
      (&http::Method::GET, "/peers") => {
        #[derive(serde::Serialize)]
        struct Labels<'a> {
          #[serde(rename = "__spadina_discovering_instance")]
          instance: &'a str,
        }
        #[derive(serde::Serialize)]
        struct ServiceDiscovery<'a> {
          targets: Vec<std::sync::Arc<str>>,
          labels: Labels<'a>,
        }
        let targets = self.directory.peers();
        match serde_json::to_vec(&[ServiceDiscovery { targets, labels: Labels { instance: &self.authnz.server_name } }]) {
          Ok(buffer) => http::Response::builder().header(hyper::header::CONTENT_TYPE, "application/json").body(buffer.into()),
          Err(e) => http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(e.to_string().into()),
        }
      }
      (&http::Method::GET, spadina_core::net::ACCESS_PATH) => {
        match self.authnz.access.read("access_url", |access| serde_json::to_vec(access)).await {
          Ok(buffer) => http::Response::builder().header(hyper::header::CONTENT_TYPE, "application/json").body(buffer.into()),
          Err(e) => http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(e.to_string().into()),
        }
      }
      // Describe what authentication scheme is used
      (&http::Method::GET, spadina_core::net::AUTH_METHOD_PATH) => {
        let scheme = self.authnz.authentication.scheme();
        match serde_json::to_string(&scheme) {
          Err(e) => {
            crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
            eprintln!("Failed to serialise authentication scheme: {}", e);
            http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(hyper::Body::empty())
          }
          Ok(auth_json) => http::Response::builder().status(http::StatusCode::OK).header("Content-Type", "application/json").body(auth_json.into()),
        }
      }
      (&http::Method::GET, spadina_core::net::CALENDAR_PATH) => {
        let filter = match req.uri().query() {
          Some(query) => match serde_urlencoded::from_str::<spadina_core::net::CalendarQuery<String>>(query) {
            Ok(query) => {
              let mut filters: Vec<_> = query
                .realms
                .into_iter()
                .map(|spadina_core::realm::LocalRealmTarget { owner, asset }| {
                  crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::NamedAsset { owner, asset })
                })
                .collect();
              if query.in_directory {
                filters.push(crate::database::realm_scope::RealmListScope::InDirectory);
              }

              Ok((filters, if query.id.is_empty() { None } else { Some(query.id) }))
            }
            Err(e) => Err(e),
          },
          None => Ok((Vec::new(), None)),
        };
        match filter {
          Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
          Ok((filters, calendar_id)) => {
            fn add_time(start: &spadina_core::communication::AnnouncementTime, event: &mut icalendar::Event) {
              match start {
                spadina_core::communication::AnnouncementTime::Until(date) => {
                  event.starts(*date);
                }
                spadina_core::communication::AnnouncementTime::Starts(start, minutes) => {
                  event.starts(*start).ends(*start + chrono::Duration::minutes(*minutes as i64));
                }
              }
            }
            use icalendar::Component;
            use icalendar::EventLike;
            let mut calendar = icalendar::Calendar::new().name(&format!("Events for {} Spadina", &self.authnz.server_name)).done();

            let loggedin = match calendar_id.as_ref() {
              None => false,
              Some(calendar_id) => match self.database.calendar_check(calendar_id.as_slice()) {
                Ok(value) => value,
                Err(e) => {
                  eprintln!("Failed to check calendar ID: {}", e);
                  false
                }
              },
            };

            for announcement in self.authnz.announcements.read() {
              if announcement.public || loggedin {
                let mut event = icalendar::Event::new();
                event.summary(&announcement.title);
                event.description(&announcement.body);
                add_time(&announcement.when, &mut event);
                if let Some(target) = &announcement.realm {
                  event.url(&target.as_ref().globalize(self.authnz.server_name.as_ref()).to_url());
                }
                calendar.push(event.done());
              }
            }
            if !filters.is_empty() || calendar_id.is_some() {
              match self.database.realm_announcements_fetch_all(
                crate::database::realm_scope::RealmListScope::Any(filters),
                calendar_id,
                &self.authnz.server_name,
              ) {
                Ok(announcements) => {
                  for (realm, announcement) in announcements {
                    let mut event = icalendar::Event::new();
                    event.summary(&announcement.title);
                    event.description(&announcement.body);
                    add_time(&announcement.when, &mut event);
                    event.url(&realm.as_ref().into_absolute(&self.authnz.server_name).to_url());
                    calendar.push(event.done());
                  }
                }
                Err(e) => {
                  eprintln!("Failed to get realm announcements for calendar: {}", e);
                }
              }
            }

            http::Response::builder().header("Content-Type", "text/calendar").body(calendar.to_string().into())
          }
        }
      }
      (&http::Method::GET, "/leaderboard") => match self.database.leaderboard() {
        Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
        Ok(leaderboard) => {
          http::Response::builder().header("Content-Type", "application/json").body(serde_json::to_vec(&leaderboard).unwrap().into())
        }
      },
      (&http::Method::GET, "/spadina.svg") => etag_request("image/svg+xml", include_bytes!("../../../spadina.svg"), req),
      // Deliver the webclient
      #[cfg(feature = "wasm-client")]
      (&http::Method::GET, "/spadina-client_bg.wasm") => etag_request("application/wasm", include_bytes!("../spadina-client_bg.wasm"), req),
      #[cfg(feature = "wasm-client")]
      (&http::Method::GET, "/spadina-client.js") => etag_request("text/javascript", include_bytes!("../spadina-client.js"), req),
      // Handle a new player connection by upgrading to a web socket
      (&http::Method::GET, spadina_core::net::CLIENT_V1_PATH) => self.open_websocket::<crate::client::Client>(req),
      // Handle a new server connection by upgrading to a web socket
      (&http::Method::GET, crate::peer::net::PATH_FINISH) => self.open_websocket::<crate::peer::Peer>(req),
      (&http::Method::POST, spadina_core::net::CLIENT_NONCE_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
          eprintln!("Failed to aggregate body: {}", e);
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
        }
        Ok(whole_body) => {
          use bytes::buf::Buf;
          match serde_json::from_reader::<_, String>(whole_body.reader())
            .map_err(|e| e.to_string())
            .and_then(|s| s.parse().map_err(|e: spadina_core::player::PlayerIdentifierError| e.to_string()))
          {
            Ok(spadina_core::player::PlayerIdentifier::Local(name)) => match jsonwebtoken::encode(
              &jsonwebtoken::Header::default(),
              &jwt::PlayerClaim { exp: jwt::expiry_time(30), name },
              &self.jwt_nonce_encoding_key,
            ) {
              Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
              Err(e) => {
                eprintln!("Failed to encode JWT as nonce: {}", e);
                http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(hyper::Body::empty())
              }
            },
            Ok(spadina_core::player::PlayerIdentifier::Remote { .. }) => {
              http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Player server provided in login".into())
            }
            Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
          }
        }
      },
      (&http::Method::POST, spadina_core::net::CLIENT_KEY_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
          eprintln!("Failed to aggregate body: {}", e);
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
        }
        Ok(whole_body) => {
          use bytes::buf::Buf;
          match serde_json::from_reader::<_, spadina_core::auth::AuthPublicKey<String>>(whole_body.reader()) {
            Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
            Ok(data) => {
              match jsonwebtoken::decode::<jwt::PlayerClaim<String>>(
                &data.nonce,
                &self.jwt_nonce_decoding_key,
                &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
              ) {
                Ok(player_claim) => {
                  match self.database.public_key_get(&player_claim.claims.name) {
                    Ok(public_keys) => {
                      for (name, der) in public_keys {
                        if let Ok(pkey) = openssl::pkey::PKey::public_key_from_der(der.as_slice()) {
                          if let Ok(mut verifier) = openssl::sign::Verifier::new(openssl::hash::MessageDigest::sha256(), &pkey) {
                            if let Err(e) = verifier.update(&data.nonce.as_bytes()) {
                              eprintln!("Signature verification error: {}", e);
                              continue;
                            }
                            if verifier.verify(&data.signature).unwrap_or(false) {
                              if let Err(e) = self.database.public_key_touch(&player_claim.claims.name, &name) {
                                eprintln!("Failed to touch public key {}: {}", &name, e);
                              }
                              return match jsonwebtoken::encode(
                                &jsonwebtoken::Header::default(),
                                &jwt::PlayerClaim { exp: jwt::expiry_time(3600), name: player_claim.claims.name },
                                &self.authnz.jwt_encoding_key,
                              ) {
                                Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
                                Err(e) => {
                                  crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
                                  eprintln!("Error generation JWT: {}", e);
                                  http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body("Failed to generate token".into())
                                }
                              };
                            }
                          }
                        }
                      }
                    }
                    Err(e) => {
                      eprintln!("Failed to fetch public keys during authentication: {}", e);
                    }
                  }
                  http::Response::builder().status(http::StatusCode::FORBIDDEN).body("No matching key".into())
                }
                Err(e) => {
                  eprintln!("Failed to decode encryption: {}", e);
                  http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Nonce is corrupt".into())
                }
              }
            }
          }
        }
      },
      // Handle a request by a peer server for a connection back
      (&http::Method::POST, crate::peer::net::PATH_START) => {
        match (spadina_core::capabilities::capabilities_from_header(&req), hyper::body::aggregate(req).await) {
          (_, Err(e)) => {
            crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
            eprintln!("Failed to aggregate body: {}", e);
            http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
          }
          (Err(e), _) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
          (Ok(capabilities), Ok(whole_body)) => {
            use bytes::buf::Buf;
            match serde_json::from_reader::<_, crate::peer::net::PeerHttpRequestBody<String>>(whole_body.reader()) {
              Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
              Ok(data) => {
                let peer_labels = crate::metrics::PeerLabel { peer: crate::shstr::ShStr::from(data.server) };
                match spadina_core::net::parse_server_name(peer_labels.peer.as_str()) {
                  Some(peer_name) => {
                    if self
                      .authnz
                      .banned_peers
                      .read("handle_post", |bans| {
                        bans.iter().all(|b| match b {
                          spadina_core::access::BannedPeer::Peer(b) => b == &peer_name,
                          spadina_core::access::BannedPeer::Domain(domain) => spadina_core::net::has_domain_suffix(domain, &peer_name),
                        })
                      })
                      .await
                    {
                      http::Response::builder().status(http::StatusCode::FORBIDDEN).body("Access denied".into())
                    } else {
                      match peer_labels.peer.as_str().parse::<http::uri::Authority>() {
                        Ok(authority) => {
                          match hyper::Uri::builder()
                            .scheme(http::uri::Scheme::HTTPS)
                            .path_and_query(crate::peer::net::PATH_FINISH)
                            .authority(authority)
                            .build()
                          {
                            Ok(uri) => {
                              let directory = self.directory.clone();
                              tokio::spawn(async move {
                                use rand::RngCore;
                                let connector = hyper_tls::HttpsConnector::new();
                                let client = hyper::client::Client::builder().build::<_, hyper::Body>(connector);

                                match client
                                  .request(
                                    spadina_core::capabilities::add_header(hyper::Request::get(uri).version(http::Version::HTTP_11))
                                      .header(http::header::CONNECTION, "upgrade")
                                      .header(http::header::SEC_WEBSOCKET_VERSION, "13")
                                      .header(http::header::SEC_WEBSOCKET_PROTOCOL, "spadina")
                                      .header(http::header::UPGRADE, "websocket")
                                      .header(http::header::SEC_WEBSOCKET_KEY, format!("spadina{}", &mut rand::thread_rng().next_u64()))
                                      .header(http::header::AUTHORIZATION, format!("Bearer {}", data.token))
                                      .body(hyper::Body::empty())
                                      .unwrap(),
                                  )
                                  .await
                                {
                                  Err(e) => {
                                    crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                                    eprintln!("Failed callback to {}: {}", &peer_labels.peer, e)
                                  }
                                  Ok(response) => {
                                    if response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
                                      match hyper::upgrade::on(response).await {
                                        Ok(upgraded) => {
                                          let socket = tokio_tungstenite::WebSocketStream::from_raw_socket(
                                            upgraded.into(),
                                            tokio_tungstenite::tungstenite::protocol::Role::Server,
                                            None,
                                          )
                                          .await;
                                          directory.peer(&peer_name, |peer| peer.finish_connection(socket, capabilities).boxed()).await;
                                        }
                                        Err(e) => {
                                          crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                                          eprintln!("Failed to connect to {}: {}", &peer_labels.peer, e);
                                        }
                                      }
                                    } else {
                                      crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                                      let status = response.status();
                                      match hyper::body::aggregate(response).await {
                                        Err(e) => eprintln!("Failed to connect to {} {}: {}", &peer_labels.peer, status, e),
                                        Ok(buf) => eprintln!(
                                          "Failed to connect to {} {}: {}",
                                          &peer_labels.peer,
                                          status,
                                          std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),
                                        ),
                                      }
                                    }
                                  }
                                }
                              });
                              http::Response::builder().status(http::StatusCode::OK).body("Will do".into())
                            }
                            Err(e) => {
                              crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                              http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(format!("{}", e).into())
                            }
                          }
                        }
                        Err(e) => {
                          crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                          http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(format!("{}", e).into())
                        }
                      }
                    }
                  }
                  None => {
                    crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
                    http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Bad server name".into())
                  }
                }
              }
            }
          }
        }
      }
      // For other URLs, see if the authentication mechanism is prepared to deal with them
      _ => match self.authnz.authentication.handle(req, &self.database).await {
        crate::auth::AuthResult::Failure => {
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body("Internal server error during authentication".into())
        }
        crate::auth::AuthResult::NotHandled => http::Response::builder().status(http::StatusCode::NOT_FOUND).body("Not Found".into()),
        crate::auth::AuthResult::Page(page) => page,
        crate::auth::AuthResult::SendToken(user_name) => match user_name.parse() {
          Ok(spadina_core::player::PlayerIdentifier::Local(user_name)) => {
            match jsonwebtoken::encode(
              &jsonwebtoken::Header::default(),
              &jwt::PlayerClaim { exp: jwt::expiry_time(3600), name: user_name },
              &self.authnz.jwt_encoding_key,
            ) {
              Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
              Err(e) => {
                crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
                eprintln!("Error generation JWT: {}", e);
                http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body("Failed to generate token".into())
              }
            }
          }
          _ => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Invalid user name".into()),
        },
      },
    }
  }
  fn open_websocket<W: websocket::WebSocketClient>(&self, req: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, http::Error> {
    // Check whether they provided a valid Authorization: Bearer header or token= URL parameter
    match req
      .headers()
      .get(http::header::AUTHORIZATION)
      .map(|h| match h.to_str() {
        Ok(value) => Some(std::borrow::Cow::Borrowed(value)),
        Err(_) => None,
      })
      .flatten()
      .or_else(|| {
        req.uri().query().map(|q| form_urlencoded::parse(q.as_bytes()).filter(|(name, _)| name == "token").map(|(_, value)| value).next()).flatten()
      })
      .map(|value| {
        if value.starts_with("Bearer ") {
          match jsonwebtoken::decode::<W::Claim>(
            &value[7..],
            &(*self).authnz.jwt_decoding_key,
            &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
          ) {
            Err(e) => {
              crate::metrics::BAD_JWT.get_or_create(&()).inc();
              eprintln!("JWT decoding failure: {}", e);
              None
            }
            Ok(data) => Some(data.claims),
          }
        } else {
          None
        }
      })
      .flatten()
    {
      Some(token_contents) => {
        let is_http_11 = req.version() == http::Version::HTTP_11;
        let is_upgrade = req.headers().get(http::header::CONNECTION).map_or(false, |v| websocket::connection_has(v, "upgrade"));
        let is_websocket_upgrade =
          req.headers().get(http::header::UPGRADE).and_then(|v| v.to_str().ok()).map_or(false, |v| v.eq_ignore_ascii_case("websocket"));
        let is_websocket_version_13 =
          req.headers().get(http::header::SEC_WEBSOCKET_VERSION).and_then(|v| v.to_str().ok()).map_or(false, |v| v == "13");
        if !is_http_11 || !is_upgrade || !is_websocket_upgrade || !is_websocket_version_13 {
          crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
          return http::Response::builder()
            .status(http::StatusCode::UPGRADE_REQUIRED)
            .header(http::header::SEC_WEBSOCKET_VERSION, "13")
            .body("Expected Upgrade to WebSocket version 13".into());
        }
        match (req.headers().get(http::header::SEC_WEBSOCKET_KEY), spadina_core::capabilities::capabilities_from_header(&req)) {
          (Some(value), Ok(capabilities)) => {
            let accept = websocket::convert_key(value.as_bytes());
            let directory = self.directory.clone();
            tokio::spawn(async move {
              match hyper::upgrade::on(req).await {
                Err(e) => {
                  crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
                  eprintln!("Upgrade error: {}", e);
                }
                Ok(upgraded) => {
                  W::accept(
                    &directory,
                    token_contents,
                    capabilities,
                    tokio_tungstenite::WebSocketStream::from_raw_socket(
                      upgraded.into(),
                      tokio_tungstenite::tungstenite::protocol::Role::Server,
                      None,
                    )
                    .await,
                  )
                  .await
                }
              }
            });

            http::Response::builder()
              .status(http::StatusCode::SWITCHING_PROTOCOLS)
              .header(http::header::UPGRADE, "websocket")
              .header(http::header::CONNECTION, "upgrade")
              .header(http::header::SEC_WEBSOCKET_ACCEPT, &accept)
              .body(hyper::Body::empty())
          }
          _ => {
            crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
            http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Websocket key is not in header".into())
          }
        }
      }
      None => {
        crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
        http::Response::builder().status(http::StatusCode::UNAUTHORIZED).body("Invalid or missing token. Please authenticate first".into())
      }
    }
  }
}

fn etag_request(
  content_type: &'static str,
  contents: &'static [u8],
  req: http::Request<hyper::Body>,
) -> Result<http::Response<hyper::Body>, http::Error> {
  if req.headers().get("If-None-Match").map(|tag| tag.to_str().ok()).flatten().map(|v| v == git_version::git_version!()).unwrap_or(false) {
    http::Response::builder().status(304).body(hyper::Body::empty())
  } else {
    http::Response::builder()
      .status(http::StatusCode::OK)
      .header("Content-Type", content_type)
      .header("ETag", git_version::git_version!())
      .body(hyper::body::Bytes::from_static(contents).into())
  }
}
