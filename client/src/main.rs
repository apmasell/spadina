mod altitude_mixer;
mod convert;
mod gradiator;
mod materials;
mod spray;
mod update_handler;

use bevy::prelude::{Component, Transform};
use futures::{SinkExt, StreamExt};
use puzzleverse_core::abs_difference;

use crate::convert::IntoBevy;

#[derive(Default)]
struct Cache {
  allowed_capabilities: Vec<&'static str>,
  bookmarks: std::collections::HashMap<puzzleverse_core::BookmarkType, std::collections::BTreeSet<String>>,
  direct_messages: std::collections::BTreeMap<String, DirectMessageInfo>,
  known_servers: std::collections::BTreeSet<String>,
  known_realms: std::collections::HashMap<puzzleverse_core::RealmSource, RealmInfo>,
  public_keys: std::collections::BTreeSet<String>,
}

#[derive(Clone)]
struct AssetManager(std::sync::Arc<dyn puzzleverse_core::asset_store::AssetStore>);

type AuthKey = std::sync::Arc<(String, openssl::pkey::PKey<openssl::pkey::Private>, Vec<u8>)>;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Configuration {
  accounts: Vec<ServerConfiguration>,
  client: String,
  private_key: String,
  public_key: String,
}

enum ConnectionState {
  Idle,
  Active {
    inbound: futures::stream::Map<
      futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<puzzleverse_core::net::IncomingConnection>>,
      fn(Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>) -> Option<puzzleverse_core::ClientResponse>,
    >,
    outbound: futures::stream::SplitSink<
      tokio_tungstenite::WebSocketStream<puzzleverse_core::net::IncomingConnection>,
      tokio_tungstenite::tungstenite::Message,
    >,
  },
}

#[derive(Default)]
struct CurrentAccess(
  std::collections::HashMap<puzzleverse_core::AccessTarget, (Vec<puzzleverse_core::AccessControl>, puzzleverse_core::AccessDefault)>,
);

struct DirectMessageInfo {
  messages: Vec<puzzleverse_core::DirectMessage>,
  last_viewed: chrono::DateTime<chrono::Utc>,
  last_message: chrono::DateTime<chrono::Utc>,
  location: puzzleverse_core::PlayerLocationState,
  draft: String,
}

enum InflightOperation {
  AccessChange(String),
  AssetCreation(String),
  DirectMessage(String),
  RealmCreation(String),
  RealmDeletion(String),
}

#[derive(Default)]
struct InflightRequests {
  id: std::sync::atomic::AtomicI32,
  outstanding: Vec<InflightRequest>,
}

struct InflightRequest {
  id: i32,
  created: chrono::DateTime<chrono::Utc>,
  operation: InflightOperation,
}
struct InteractionTarget {
  click: bool,
  key: puzzleverse_core::InteractionKey,
  point: puzzleverse_core::Point,
}

type Paths = std::collections::HashMap<puzzleverse_core::Point, Vec<puzzleverse_core::Point>>;
type PlatformDistances = std::collections::HashMap<(u32, u32), u32>;
struct PlayerName(String);

struct RealmInfo {
  last_updated: chrono::DateTime<chrono::Utc>,
  realms: Vec<puzzleverse_core::Realm>,
}

enum RealmSelector {
  Player,
  Local,
  Bookmarks,
  Remote(String),
  Url(String),
}

enum RealmState {
  Active { asset: String, name: String, realm: String, seed: i32, server: String, settings: puzzleverse_core::RealmSettings },
  Inactive,
}

enum ScreenState {
  Error(String),
  Busy(String),
  InTransit,
  Loading {
    assets: std::collections::BTreeSet<String>,
  },
  Lost(RealmSelector, Option<std::borrow::Cow<'static, str>>),
  PasswordLogin {
    error_message: Option<std::borrow::Cow<'static, str>>,
    insecure: bool,
    password: String,
    player: String,
    server: String,
  },
  Realm {
    clicked_realm_selector: Option<(Vec<puzzleverse_core::Action>, puzzleverse_core::InteractionKey, puzzleverse_core::Point, RealmSelector)>,
    confirm_delete: bool,
    direct_message_user: String,
    is_mine: bool,
    messages: Vec<puzzleverse_core::RealmMessage>,
    new_chat: Option<String>,
    paths: Paths,
    platform_distances: PlatformDistances,
    realm_asset: String,
    realm_id: String,
    realm_message: String,
    realm_name: String,
    realm_selector: Option<RealmSelector>,
    realm_server: String,
  },
  ServerSelection {
    error_message: Option<std::borrow::Cow<'static, str>>,
    insecure: bool,
    player: String,
    server: String,
  },
  Waiting,
}

struct ServerConnection {
  inbound_rx: std::sync::Mutex<std::sync::mpsc::Receiver<ServerResponse>>,
  outbound_tx: std::sync::Mutex<tokio::sync::mpsc::UnboundedSender<ServerRequest>>,
  task: tokio::task::JoinHandle<()>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ServerConfiguration {
  player: String,
  server: String,
}
#[derive(Clone)]
enum ServerRequest {
  CheckAuthMethods { insecure: bool, player: String, server: String, key: Option<AuthKey> },
  Deliver(puzzleverse_core::ClientRequest),
  PasswordLogin { insecure: bool, player: String, password: String, server: String },
  OpenIdConnectLogin { insecure: bool, player: String, server: String },
  KerberosLogin { insecure: bool, player: String, server: String },
}

enum ServerResponse {
  AuthMethod { insecure: bool, server: String, player: String, scheme: puzzleverse_core::AuthScheme },
  AuthMethodFailed { insecure: bool, server: String, player: String, error_message: std::borrow::Cow<'static, str> },
  AuthPasswordFailed { insecure: bool, server: String, player: String, password: String, error_message: std::borrow::Cow<'static, str> },
  Connected,
  Deliver(puzzleverse_core::ClientResponse),
  Disconnected,
}

enum StatusInfo {
  AcknowledgeFailure(String),
  RealmLink(puzzleverse_core::RealmTarget, String),
  TimeoutFailure(String, chrono::DateTime<chrono::Utc>),
  TimeoutSuccess(String, chrono::DateTime<chrono::Utc>),
}

#[derive(Default)]
struct StatusList {
  list: Vec<StatusInfo>,
}

struct Target(puzzleverse_core::Point);

fn decode_server_messages(
  input: Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>,
) -> Option<puzzleverse_core::ClientResponse> {
  match input {
    Ok(tokio_tungstenite::tungstenite::Message::Binary(value)) => match rmp_serde::from_read(std::io::Cursor::new(&value)) {
      Ok(v) => Some(v),
      Err(e) => {
        eprintln!("Failed to decode message from server. Mismatched protocols?: {}", e);
        None
      }
    },
    Ok(_) => None,
    Err(e) => {
      eprintln!("Failed to decode Web Socket packet: {}", e);
      None
    }
  }
}
struct AccessControlEditor {
  access_default: puzzleverse_core::AccessDefault,
  access_list: Vec<puzzleverse_core::AccessControl>,
}

impl std::ops::Deref for AssetManager {
  type Target = dyn puzzleverse_core::asset_store::AssetStore;
  fn deref(&self) -> &Self::Target {
    self.0.deref()
  }
}
impl AccessControlEditor {
  fn draw_ui(
    &mut self,
    ui: &mut bevy_egui::egui::Ui,
    name: &str,
    access_default: &mut puzzleverse_core::AccessDefault,
    access_list: &mut Vec<puzzleverse_core::AccessControl>,
  ) -> bool {
    enum RuleMutations {
      SetAllowed(usize),
      SetDenied(usize),
      Delete(usize),
    }
    let mut update = None;
    bevy_egui::egui::Grid::new(name).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
      for (index, entry) in self.access_list.iter_mut().enumerate() {
        ui.label(match entry {
          puzzleverse_core::AccessControl::AllowLocal(_) | puzzleverse_core::AccessControl::DenyLocal(_) => "All Players on This Server",
          puzzleverse_core::AccessControl::AllowServer(_, _) | puzzleverse_core::AccessControl::DenyServer(_, _) => "All Players on:",
          puzzleverse_core::AccessControl::AllowPlayer(_, _) | puzzleverse_core::AccessControl::DenyPlayer(_, _) => "Player:",
        });

        match entry {
          puzzleverse_core::AccessControl::AllowLocal(_) | puzzleverse_core::AccessControl::DenyLocal(_) => {
            ui.label("");
          }
          puzzleverse_core::AccessControl::AllowServer(server, _) | puzzleverse_core::AccessControl::DenyServer(server, _) => {
            ui.text_edit_singleline(server);
          }
          puzzleverse_core::AccessControl::AllowPlayer(player, _) | puzzleverse_core::AccessControl::DenyPlayer(player, _) => {
            ui.text_edit_singleline(player);
          }
        };

        let mut selected: usize = match entry {
          puzzleverse_core::AccessControl::AllowLocal(_)
          | puzzleverse_core::AccessControl::AllowPlayer(_, _)
          | puzzleverse_core::AccessControl::AllowServer(_, _) => 0,
          puzzleverse_core::AccessControl::DenyLocal(_)
          | puzzleverse_core::AccessControl::DenyPlayer(_, _)
          | puzzleverse_core::AccessControl::DenyServer(_, _) => 1,
        };
        if bevy_egui::egui::ComboBox::from_id_source((name, index))
          .show_index(ui, &mut selected, 2, |i| (if i == 0 { "Allow" } else { "Deny" }).to_owned())
          .changed()
        {
          update = Some(if selected == 0 { RuleMutations::SetAllowed(index) } else { RuleMutations::SetDenied(index) });
        }
        let ok = match entry {
          puzzleverse_core::AccessControl::AllowPlayer(player, _) | puzzleverse_core::AccessControl::DenyPlayer(player, _) => {
            puzzleverse_core::PlayerIdentifier::new(player.as_str(), None) != puzzleverse_core::PlayerIdentifier::Bad
          }
          puzzleverse_core::AccessControl::AllowServer(server, _) | puzzleverse_core::AccessControl::DenyServer(server, _) => {
            puzzleverse_core::parse_server_name(server.as_str()).is_some()
          }
          puzzleverse_core::AccessControl::AllowLocal(_) | puzzleverse_core::AccessControl::DenyLocal(_) => true,
        };
        ui.add(if ok {
          bevy_egui::egui::Label::new("")
        } else {
          bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new("Invalid").color(bevy_egui::egui::Color32::RED))
        });
        let timestamp = match entry {
          puzzleverse_core::AccessControl::AllowLocal(timestamp)
          | puzzleverse_core::AccessControl::AllowPlayer(_, timestamp)
          | puzzleverse_core::AccessControl::AllowServer(_, timestamp)
          | puzzleverse_core::AccessControl::DenyLocal(timestamp)
          | puzzleverse_core::AccessControl::DenyPlayer(_, timestamp)
          | puzzleverse_core::AccessControl::DenyServer(_, timestamp) => timestamp,
        };
        match timestamp {
          None => {
            ui.label("Forever");
          }
          Some(timeout) => {
            ui.label(format!("Expires at {} ({})", timeout.to_rfc3339(), chrono::Duration::minutes((chrono::Utc::now() - *timeout).num_minutes())));
          }
        }
        ui.vertical_centered(|ui| {
          if ui.button("Forever").clicked() {
            *timestamp = None;
          }
          if ui.button("1 Hour").clicked() {
            *timestamp = Some(chrono::Utc::now() + chrono::Duration::hours(1));
          }
          if ui.button("3 Hours").clicked() {
            *timestamp = Some(chrono::Utc::now() + chrono::Duration::hours(3));
          }
          if ui.button("1 Day").clicked() {
            *timestamp = Some(chrono::Utc::now() + chrono::Duration::days(1));
          }
          if ui.button("1 Week").clicked() {
            *timestamp = Some(chrono::Utc::now() + chrono::Duration::weeks(1));
          }
        });
        if ui.button("Delete").clicked() {
          update = Some(RuleMutations::Delete(index));
        }
      }
    });
    if let Some(update) = update {
      match update {
        RuleMutations::Delete(index) => {
          self.access_list.remove(index);
        }
        RuleMutations::SetAllowed(index) => {
          if let Some(rule) = self.access_list.get_mut(index) {
            replace_with::replace_with_or_abort(rule, |rule| match rule {
              puzzleverse_core::AccessControl::DenyPlayer(player, timeout) => puzzleverse_core::AccessControl::AllowPlayer(player, timeout),
              puzzleverse_core::AccessControl::DenyLocal(timeout) => puzzleverse_core::AccessControl::AllowLocal(timeout),
              puzzleverse_core::AccessControl::DenyServer(server, timeout) => puzzleverse_core::AccessControl::AllowServer(server, timeout),
              rule => rule,
            });
          }
        }
        RuleMutations::SetDenied(index) => {
          if let Some(rule) = self.access_list.get_mut(index) {
            replace_with::replace_with_or_abort(rule, |rule| match rule {
              puzzleverse_core::AccessControl::AllowPlayer(player, timeout) => puzzleverse_core::AccessControl::DenyPlayer(player, timeout),
              puzzleverse_core::AccessControl::AllowLocal(timeout) => puzzleverse_core::AccessControl::DenyLocal(timeout),
              puzzleverse_core::AccessControl::AllowServer(server, timeout) => puzzleverse_core::AccessControl::DenyServer(server, timeout),
              rule => rule,
            });
          }
        }
      }
    }
    let mut selected: usize = match self.access_default {
      puzzleverse_core::AccessDefault::Allow => 0,
      puzzleverse_core::AccessDefault::Deny => 1,
    };
    ui.label("Otherwise: ");
    if bevy_egui::egui::ComboBox::from_id_source((name, "default"))
      .show_index(ui, &mut selected, 2, |i| (if i == 0 { "Allow" } else { "Deny" }).to_owned())
      .changed()
    {
      self.access_default = if selected == 0 { puzzleverse_core::AccessDefault::Allow } else { puzzleverse_core::AccessDefault::Deny };
    }
    let mut save = false;
    ui.horizontal(|ui| {
      if ui.button("Revert").clicked() {
        self.access_list = access_list.clone();
        self.access_default = access_default.clone();
      }
      if ui.button("Save").clicked() {
        *access_list = self.access_list.clone();
        *access_default = self.access_default.clone();
        save = true;
      }
    });
    save
  }
}
impl ConnectionState {
  async fn process(&mut self, request: ServerRequest, response_stream: &mut std::sync::mpsc::Sender<ServerResponse>) {
    async fn open_websocket(insecure: bool, authority: &http::uri::Authority, token: String) -> Result<ConnectionState, String> {
      eprintln!("Got authorization token: {}", &token);
      match hyper::Uri::builder()
        .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
        .path_and_query(puzzleverse_core::net::CLIENT_V1_PATH)
        .authority(authority.clone())
        .build()
      {
        Ok(uri) => {
          let connector = hyper_tls::HttpsConnector::new();
          let client = hyper::Client::builder().build::<_, hyper::Body>(connector);
          use rand::RngCore;

          match client
            .request(
              hyper::Request::get(uri)
                .version(http::Version::HTTP_11)
                .header(http::header::CONNECTION, "upgrade")
                .header(http::header::SEC_WEBSOCKET_VERSION, "13")
                .header(http::header::SEC_WEBSOCKET_PROTOCOL, "puzzleverse")
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
                  Ok(upgraded) => {
                    use futures::prelude::*;
                    let (writer, reader) = tokio_tungstenite::WebSocketStream::from_raw_socket(
                      puzzleverse_core::net::IncomingConnection::Upgraded(upgraded),
                      tokio_tungstenite::tungstenite::protocol::Role::Client,
                      None,
                    )
                    .await
                    .split();
                    Ok(ConnectionState::Active { inbound: reader.map(decode_server_messages), outbound: writer })
                  }
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
    match request {
      ServerRequest::CheckAuthMethods { insecure, server, player, key } => {
        async fn make_request(server: &str, insecure: bool) -> Result<puzzleverse_core::AuthScheme, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(puzzleverse_core::net::AUTH_METHOD_PATH)
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
                .path_and_query(puzzleverse_core::net::CLIENT_NONCE_PATH)
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
                                serde_json::to_vec(&puzzleverse_core::AuthPublicKey {
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
            Err(e) => None,
          }
        }
        let mut socket_path = std::path::PathBuf::new();
        socket_path.push(&server);
        socket_path.push(format!("puzzleverse-{}.socket", &player));
        if std::fs::metadata(&socket_path).is_ok() {
          match tokio::net::UnixStream::connect(socket_path).await {
            Ok(socket) => {
              let (writer, reader) = tokio_tungstenite::WebSocketStream::from_raw_socket(
                puzzleverse_core::net::IncomingConnection::Unix(socket),
                tokio_tungstenite::tungstenite::protocol::Role::Client,
                None,
              )
              .await
              .split();
              *self = ConnectionState::Active { inbound: reader.map(decode_server_messages), outbound: writer };
              response_stream.send(ServerResponse::Connected).unwrap();
            }
            Err(e) => {
              response_stream
                .send(ServerResponse::AuthMethodFailed { insecure, server, player, error_message: std::borrow::Cow::Owned(e.to_string()) })
                .unwrap();
            }
          }
        } else {
          match make_request(&server, insecure).await {
            Ok(scheme) => {
              let auto = match key {
                Some(key) => try_private_key(&server, insecure, &player, key).await,
                None => None,
              };
              response_stream
                .send(match auto {
                  Some(state) => {
                    *self = state;
                    ServerResponse::Connected
                  }
                  None => ServerResponse::AuthMethod { insecure, server, player, scheme },
                })
                .unwrap()
            }
            Err(error_message) => response_stream
              .send(ServerResponse::AuthMethodFailed { insecure, server, player, error_message: std::borrow::Cow::Owned(error_message) })
              .unwrap(),
          }
        }
      }
      #[cfg(not(feature = "kerberos"))]
      ServerRequest::KerberosLogin { insecure, server, player } => response_stream
        .send(ServerResponse::AuthMethodFailed {
          insecure,
          server,
          player,
          error_message:
            "This client was not built with Kerberos support but the server requires it. Please download a client with Kerberos support built-in."
              .to_owned(),
        })
        .unwrap(),
      #[cfg(feature = "kerberos")]
      ServerRequest::KerberosLogin { insecure, server, player } => {
        async fn make_request(server: &str, insecure: bool, username: &str) -> Result<ConnectionState, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              let server_principal = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(puzzleverse_core::net::KERBEROS_PRINCIPAL_PATH)
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
              let at_index = server_principal.find('@').ok_or(format!("Server principal is malformed: {}", server_principal))?;
              let player_principal = format!("{}@{}", username, &server_principal[at_index..]);
              let (partial, kerberos_token) =
                cross_krb5::ClientCtx::new(cross_krb5::InitiateFlags::empty(), Some(&player_principal), &server_principal, None)
                  .map_err(|e| e.to_string())?;

              let mut token_data = Vec::new();
              token_data.copy_from_slice(&*kerberos_token);

              let token: String = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(puzzleverse_core::net::KERBEROS_AUTH_PATH)
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
        match make_request(&server, insecure, &player).await {
          Ok(connection) => {
            *self = connection;
            response_stream.send(ServerResponse::Connected).unwrap();
          }
          Err(error_message) => response_stream
            .send(ServerResponse::AuthMethodFailed { insecure, server, player, error_message: std::borrow::Cow::Owned(error_message) })
            .unwrap(),
        }
      }
      ServerRequest::OpenIdConnectLogin { insecure, server, player } => {
        async fn make_request(server: &str, insecure: bool, username: &str) -> Result<ConnectionState, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              let response: puzzleverse_core::OpenIdConnectInformation = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(format!(
                  "{}?{}",
                  puzzleverse_core::net::OIDC_AUTH_START_PATH,
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
              webbrowser::open(&response.authorization_url).map_err(|e| format!("Failed to open web browser: {}", e))?;
              let token: String = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(format!(
                  "{}?{}",
                  puzzleverse_core::net::OIDC_AUTH_FINISH_PATH,
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
        match make_request(&server, insecure, &player).await {
          Ok(connection) => {
            *self = connection;
            response_stream.send(ServerResponse::Connected).unwrap();
          }
          Err(error_message) => response_stream
            .send(ServerResponse::AuthMethodFailed { insecure, server, player, error_message: std::borrow::Cow::Owned(error_message) })
            .unwrap(),
        }
      }
      ServerRequest::PasswordLogin { insecure, server, player, password } => {
        async fn make_request(server: &str, insecure: bool, username: &str, password: &str) -> Result<ConnectionState, String> {
          match server.parse::<http::uri::Authority>() {
            Ok(authority) => {
              let token: String = match hyper::Uri::builder()
                .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
                .path_and_query(puzzleverse_core::net::PASSWORD_AUTH_PATH)
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
                          serde_json::to_vec(&puzzleverse_core::PasswordRequest { username, password }).map_err(|e| e.to_string())?,
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
        match make_request(&server, insecure, &player, &password).await {
          Ok(connection) => {
            *self = connection;
            response_stream.send(ServerResponse::Connected).unwrap();
          }
          Err(error_message) => response_stream
            .send(ServerResponse::AuthPasswordFailed { insecure, server, player, password, error_message: std::borrow::Cow::Owned(error_message) })
            .unwrap(),
        }
      }
      ServerRequest::Deliver(request) => {
        if let ConnectionState::Active { outbound, .. } = self {
          outbound.send(tokio_tungstenite::tungstenite::Message::Binary(rmp_serde::to_vec(&request).unwrap())).await.unwrap();
        }
      }
    }
  }
}

impl InflightRequests {
  fn finish(&mut self, id: i32) -> Option<InflightOperation> {
    match self.outstanding.iter().enumerate().filter(|(_, r)| r.id != id).map(|(i, _)| i).next() {
      Some(index) => Some(self.outstanding.remove(index).operation),
      None => None,
    }
  }
  fn push(&mut self, operation: InflightOperation) -> i32 {
    let id = self.id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    self.outstanding.push(InflightRequest { operation, id, created: chrono::Utc::now() });
    id
  }
}
impl bevy::ecs::component::Component for InteractionTarget {
  type Storage = bevy::ecs::component::SparseStorage;
}
impl bevy::ecs::component::Component for PlayerName {
  type Storage = bevy::ecs::component::SparseStorage;
}

impl bevy::ecs::component::Component for Target {
  type Storage = bevy::ecs::component::SparseStorage;
}

impl RealmSelector {
  fn draw_ui(
    &mut self,
    ui: &mut bevy_egui::egui::Ui,
    known_realms: &mut std::collections::HashMap<puzzleverse_core::RealmSource, RealmInfo>,

    server_requests: &mut bevy::ecs::event::EventWriter<ServerRequest>,
    request_for_realm: impl FnMut(puzzleverse_core::RealmTarget) -> puzzleverse_core::ClientRequest,
  ) {
    let mut selected = self.id();
    ui.horizontal(|ui| {
      bevy_egui::egui::ComboBox::from_id_source("realm_selector").show_index(ui, &mut selected, 5, |i| match i {
        0 => "Personal".to_string(),
        1 => "Bookmarks".to_string(),
        2 => "Local".to_string(),
        3 => "Remote".to_string(),
        4 => "URL".to_string(),
        _ => panic!("Impossible realm selection."),
      });
    });
    if selected != self.id() {
      *self = match selected {
        0 => RealmSelector::Player,
        1 => RealmSelector::Bookmarks,
        2 => RealmSelector::Local,
        3 => RealmSelector::Remote(String::new()),
        4 => RealmSelector::Url(String::new()),
        _ => panic!("Impossible realm selection."),
      };
      if let Some(refresh_request) = self.refresh_request() {
        if known_realms.get(&refresh_request).map(|info| chrono::Utc::now() - info.last_updated > chrono::Duration::minutes(1)).unwrap_or(true) {
          server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmsList(refresh_request)));
        }
      }
    }
    match self {
      RealmSelector::Player => {
        RealmSelector::show_list(ui, server_requests, request_for_realm, known_realms.get(&puzzleverse_core::RealmSource::Personal))
      }
      RealmSelector::Bookmarks => {
        RealmSelector::show_list(ui, server_requests, request_for_realm, known_realms.get(&puzzleverse_core::RealmSource::Bookmarks))
      }
      RealmSelector::Local => {
        RealmSelector::show_list(ui, server_requests, request_for_realm, known_realms.get(&puzzleverse_core::RealmSource::LocalServer))
      }
      RealmSelector::Remote(server) => {
        ui.horizontal(|ui| {
          let serverbox = ui.text_edit_singleline(server);
          let send = ui.button("⮨");
          if serverbox.changed() && server.ends_with('\n') || send.clicked() {
            use addr::parser::DomainName;
            if addr::psl::List.parse_domain_name(&server).is_ok() {
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmsList(puzzleverse_core::RealmSource::RemoteServer(
                server.clone(),
              ))));
            }
          }
        });
        RealmSelector::show_list(
          ui,
          server_requests,
          request_for_realm,
          known_realms.get(&puzzleverse_core::RealmSource::RemoteServer(server.clone())),
        )
      }
      RealmSelector::Url(url) => {
        let (urlbox, send) = ui.horizontal(|ui| (ui.text_edit_singleline(url), ui.button("⮨"))).inner;

        match url.parse::<puzzleverse_core::RealmTarget>() {
          Ok(target) => {
            let source = puzzleverse_core::RealmSource::Manual(target);
            RealmSelector::show_list(ui, server_requests, request_for_realm, known_realms.get(&source));
            if urlbox.changed() && url.ends_with('\n') || send.clicked() {
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmsList(source)));
            }
          }
          Err(e) => {
            ui.horizontal(|ui| {
              ui.add(bevy_egui::egui::Label::new(
                bevy_egui::egui::RichText::new(match e {
                  puzzleverse_core::RealmTargetParseError::BadPath => "Path is incorrect".to_string(),
                  puzzleverse_core::RealmTargetParseError::BadHost => "Host is incorrect".to_string(),
                  puzzleverse_core::RealmTargetParseError::BadSchema => "Only puzzleverse: URLs are supported".to_string(),
                  puzzleverse_core::RealmTargetParseError::UrlError(e) => e.to_string(),
                })
                .color(bevy_egui::egui::Color32::RED),
              ));
            });
          }
        }
      }
    }
  }
  fn id(&self) -> usize {
    match self {
      RealmSelector::Player => 0,
      RealmSelector::Bookmarks => 1,
      RealmSelector::Local => 2,
      RealmSelector::Remote(_) => 3,
      RealmSelector::Url(_) => 4,
    }
  }
  fn refresh_request(&self) -> Option<puzzleverse_core::RealmSource> {
    match self {
      RealmSelector::Player => Some(puzzleverse_core::RealmSource::Personal),
      RealmSelector::Bookmarks => Some(puzzleverse_core::RealmSource::Bookmarks),
      RealmSelector::Local => Some(puzzleverse_core::RealmSource::LocalServer),
      RealmSelector::Remote(hostname) => {
        use addr::parser::DomainName;
        if addr::psl::List.parse_domain_name(&hostname).is_ok() {
          Some(puzzleverse_core::RealmSource::RemoteServer(hostname.clone()))
        } else {
          None
        }
      }
      RealmSelector::Url(url) => url.parse::<puzzleverse_core::RealmTarget>().ok().map(puzzleverse_core::RealmSource::Manual),
    }
  }
  fn show_list(
    ui: &mut bevy_egui::egui::Ui,
    server_requests: &mut bevy::ecs::event::EventWriter<ServerRequest>,
    mut request_for_realm: impl FnMut(puzzleverse_core::RealmTarget) -> puzzleverse_core::ClientRequest,
    realms: Option<&RealmInfo>,
  ) {
    match realms {
      None => {
        ui.label("No matching realms.");
      }
      Some(realm_info) => {
        bevy_egui::egui::ScrollArea::vertical().id_source("realm_list").show(ui, |ui| {
          bevy_egui::egui::Grid::new("realm_list_grid").striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for realm in &realm_info.realms {
              if ui
                .add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(&realm.name).strong()))
                .on_hover_text(&realm.id)
                .on_hover_cursor(bevy_egui::egui::CursorIcon::PointingHand)
                .clicked()
              {
                server_requests.send(ServerRequest::Deliver(request_for_realm(match &realm.server {
                  None => puzzleverse_core::RealmTarget::LocalRealm(realm.id.clone()),
                  Some(server) => puzzleverse_core::RealmTarget::RemoteRealm { realm: realm.id.clone(), server: server.clone() },
                })));
              }
              ui.add(match &realm.server {
                Some(server) => bevy_egui::egui::Label::new(server.clone()),
                None => bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new("(Local)").color(bevy_egui::egui::Color32::GRAY)),
              });
              ui.label(realm.train.map(|t| format!("{}", t)).unwrap_or("".to_string()));
              ui.label(realm.accessed.map(|t| t.with_timezone(&chrono::Local).format("%c").to_string()).unwrap_or("".to_string()));
              ui.label(match realm.activity {
                puzzleverse_core::RealmActivity::Unknown => "???",
                puzzleverse_core::RealmActivity::Deserted => "🌵",
                puzzleverse_core::RealmActivity::Quiet => "🧑",
                puzzleverse_core::RealmActivity::Popular => "🧑💬",
                puzzleverse_core::RealmActivity::Busy => "🧑🧑💬",
                puzzleverse_core::RealmActivity::Crowded => "🧑💬🧑💬",
              });
              ui.end_row();
            }
          });
        });
      }
    }
  }
}

impl Default for RealmState {
  fn default() -> Self {
    RealmState::Inactive
  }
}

impl ServerConnection {
  fn new(runtime: &tokio::runtime::Runtime) -> Self {
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::unbounded_channel();
    let (mut inbound_tx, inbound_rx) = std::sync::mpsc::channel();
    ServerConnection {
      outbound_tx: std::sync::Mutex::new(outbound_tx),
      inbound_rx: std::sync::Mutex::new(inbound_rx),
      task: runtime.spawn(async move {
        let mut state = ConnectionState::Idle;
        loop {
          enum Event {
            Server(Option<puzzleverse_core::ClientResponse>),
            UserInterface(Option<ServerRequest>),
          }
          let event = match &mut state {
            ConnectionState::Idle => Event::UserInterface(outbound_rx.recv().await),
            ConnectionState::Active { inbound, .. } => {
              tokio::select! {
                output = outbound_rx.recv() => Event::UserInterface(output),
                Some(response) = inbound.next() => Event::Server(response)
              }
            }
          };
          match event {
            Event::UserInterface(None) => break,
            Event::UserInterface(Some(output)) => state.process(output, &mut inbound_tx).await,
            Event::Server(output) => inbound_tx
              .send(match output {
                Some(message) => ServerResponse::Deliver(message),
                None => ServerResponse::Disconnected,
              })
              .unwrap(),
          }
        }
      }),
    }
  }
}

async fn load_realm(
  asset_manager: AssetManager,
  asset: puzzleverse_core::asset::Asset,
) -> Result<puzzleverse_core::asset::AssetAnyRealm, puzzleverse_core::AssetError> {
  let async_asset_store = puzzleverse_core::asset_store::AsyncStore(asset_manager);
  puzzleverse_core::asset::AssetAnyRealm::load(asset, &async_asset_store).await.map(|(realm, _)| realm)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  let error_message = match self_update::backends::github::Update::configure()
    .repo_owner("apmasell")
    .repo_name("puzzleverse")
    .bin_name("puzzleverse-client")
    .show_download_progress(true)
    .current_version(self_update::cargo_crate_version!())
    .build()
    .unwrap()
    .update()
  {
    Ok(self_update::Status::UpToDate(_)) => None,
    Ok(self_update::Status::Updated(version)) => {
      println!("Updated to {}", version);
      None
    }
    Err(e) => Some(std::borrow::Cow::Owned(format!("Failed to update: {}", e))),
  };
  let dirs = directories::ProjectDirs::from("", "", "puzzleverse").unwrap();
  let mut login_file = std::path::PathBuf::new();
  login_file.extend(dirs.config_dir());
  login_file.push("client.json");
  let configuration = (if std::fs::metadata(&login_file).is_ok() {
    match std::fs::OpenOptions::new().read(true).open(&login_file) {
      Ok(login_handle) => match serde_json::from_reader::<_, Configuration>(login_handle) {
        Ok(config) => Some(config),
        Err(e) => {
          eprintln!("Failed to load configuration: {}", e);
          None
        }
      },
      Err(e) => {
        eprintln!("Failed to open configuration: {}", e);
        None
      }
    }
  } else {
    None
  })
  .unwrap_or_else(|| {
    let keys = openssl::ec::EcKey::generate(
      &openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::SECP256K1).expect("Unable to find elliptic curve group."),
    )
    .expect("Unable to generate encryption key");
    let mut buf = [0; 32];
    openssl::rand::rand_bytes(&mut buf).unwrap();
    Configuration {
      accounts: vec![],
      client: buf.iter().map(|b| format!("{:2X}", b)).collect(),
      private_key: String::from_utf8(keys.private_key_to_pem().expect("Failed to encoding private key")).expect("OpenSSL generate invalid output"),
      public_key: String::from_utf8(keys.public_key_to_pem().expect("Failed to encoding public key")).expect("OpenSSL generate invalid output"),
    }
  });
  serde_json::to_writer(std::fs::OpenOptions::new().write(true).open(&login_file).expect("Failed to write client configuration."), &configuration)
    .expect("Failed to encode client configuration");
  let mut insecure = false;
  {
    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("Puzzleverse Client");
    ap.refer(&mut insecure).add_option(&["-i", "--insecure"], argparse::StoreTrue, "Use HTTP instead HTTPS");
    ap.parse_args_or_exit();
  }
  let mut asset_directory = dirs.cache_dir().to_path_buf();
  asset_directory.push("assets");

  let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  bevy::app::App::new()
    .add_event::<ServerRequest>()
    .add_event::<ServerResponse>()
    .insert_resource(ServerConnection::new(&rt))
    .insert_resource(ScreenState::ServerSelection { insecure, server: String::new(), player: String::new(), error_message })
    .insert_resource(AssetManager(std::sync::Arc::new(puzzleverse_core::asset_store::FileSystemStore::new(
      asset_directory,
      [4, 4, 8].iter().cloned(),
    ))))
    .insert_resource::<Option<AuthKey>>(
      match (
        openssl::pkey::PKey::private_key_from_pem(&configuration.private_key.as_bytes()),
        openssl::pkey::PKey::public_key_from_pem(&configuration.public_key.as_bytes()),
      ) {
        (Ok(private_key), Ok(public_key)) => {
          use sha3::Digest;
          let mut name = sha3::Sha3_512::new();
          name.update(&configuration.private_key.as_bytes());
          Some(std::sync::Arc::new((hex::encode(name.finalize()), private_key, public_key.public_key_to_der().expect("Failed to encode public key"))))
        }
        _ => None,
      },
    )
    .init_resource::<Cache>()
    .init_resource::<CurrentAccess>()
    .init_resource::<InflightRequests>()
    .init_resource::<RealmState>()
    .init_resource::<StatusList>()
    .add_plugins(bevy::DefaultPlugins)
    .add_plugin(bevy_mod_picking::PickingPlugin)
    .add_plugin(bevy_mod_picking::InteractablePickingPlugin)
    .add_plugins(bevy_mod_picking::HighlightablePickingPlugins)
    .add_plugin(bevy_egui::EguiPlugin)
    .add_startup_system(setup)
    .add_system(bevy::animation::animation_player)
    .add_system(draw_ui)
    .add_system(mouse_event)
    .add_system(process_request)
    .add_system(realm_converter)
    .add_system_to_stage(bevy::app::CoreStage::PreUpdate, send_network_events)
    .add_system_to_stage(bevy::app::CoreStage::PreUpdate, receive_network_events)
    .run();
  rt.shutdown_background();
}

#[cfg(target_arch = "wasm32")]
fn main() {
  unimplemented!()
}
fn setup(mut commands: bevy::ecs::system::Commands, mut materials: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::pbr::StandardMaterial>>) {
  commands
    .spawn()
    .insert_bundle(bevy::core_pipeline::core_3d::Camera3dBundle::default())
    .insert_bundle(bevy_mod_picking::PickingCameraBundle::default());
}

fn send_network_events(connection: bevy::ecs::system::ResMut<ServerConnection>, mut network_events: bevy::ecs::event::EventWriter<ServerResponse>) {
  network_events.send_batch(connection.inbound_rx.lock().unwrap().try_iter());
}

fn draw_ui(
  egui: bevy::ecs::system::ResMut<bevy_egui::EguiContext>,
  auth_key: bevy::ecs::system::Res<Option<AuthKey>>,
  mut caches: bevy::ecs::system::ResMut<Cache>,
  mut clipboard: bevy::ecs::system::ResMut<bevy_egui::EguiClipboard>,
  mut exit: bevy::ecs::event::EventWriter<bevy::app::AppExit>,
  mut inflight_requests: bevy::ecs::system::ResMut<InflightRequests>,
  mut player_names: bevy::ecs::system::Query<(&mut bevy_mod_picking::Selection, &PlayerName)>,
  mut screen: bevy::ecs::system::ResMut<ScreenState>,
  mut server_requests: bevy::ecs::event::EventWriter<ServerRequest>,
  mut status_list: bevy::ecs::system::ResMut<StatusList>,
  mut windows: bevy::ecs::system::ResMut<bevy::window::Windows>,
) {
  let ui = egui.ctx_mut();
  let mut next_ui = None;
  let window = windows.get_primary_mut().unwrap();
  let mut fullscreen = window.mode() == bevy::window::WindowMode::BorderlessFullscreen;

  window.set_title(match &mut *screen {
    ScreenState::InTransit => {
      bevy_egui::egui::CentralPanel::default().show(&ui, |ui| {
        ui.label("Finding Realm...");
      });
      "Finding realm - Puzzleverse".to_string()
    }
    ScreenState::Loading { assets } => {
      bevy_egui::egui::CentralPanel::default().show(&ui, |ui| {
        ui.label("Loading Assets for Realm...");
        ui.label(format!("{} remaining", assets.len()));
      });
      "Loading realm - Puzzleverse".to_string()
    }
    ScreenState::Lost(realm_selector, error_message) => {
      bevy_egui::egui::Window::new("Navigate to Realm").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(
        &ui,
        |ui| {
          if let Some(error_message) = error_message {
            ui.horizontal(|ui| {
              ui.add(bevy_egui::egui::widgets::Label::new(
                bevy_egui::egui::RichText::new(error_message.to_string()).color(bevy_egui::egui::Color32::RED),
              ))
            });
          }
          realm_selector.draw_ui(ui, &mut caches.known_realms, &mut server_requests, |realm| puzzleverse_core::ClientRequest::RealmChange { realm })
        },
      );
      "Puzzleverse".into()
    }
    ScreenState::PasswordLogin { insecure, password, server, player, error_message } => {
      bevy_egui::egui::Window::new("Connect to Server").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(
        &ui,
        |ui| {
          ui.horizontal(|ui| {
            ui.label("Password: ");
            ui.add(bevy_egui::egui::TextEdit::singleline(password).password(true));
          });
          if let Some(error_message) = error_message {
            ui.horizontal(|ui| {
              ui.add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(error_message.to_string()).color(bevy_egui::egui::Color32::RED)))
            });
          }
          ui.horizontal(|ui| {
            if ui.button("Connect").clicked() {
              server_requests.send(ServerRequest::PasswordLogin {
                insecure: *insecure,
                server: server.clone(),
                player: player.clone(),
                password: password.clone(),
              });
              next_ui = Some(ScreenState::Busy("Connecting...".into()));
            }
            if ui.button("Back").clicked() {
              next_ui =
                Some(ScreenState::ServerSelection { insecure: *insecure, server: server.clone(), player: player.clone(), error_message: None });
            }
            if ui.button("Quit").clicked() {
              exit.send(bevy::app::AppExit);
            }
          });
        },
      );
      "Login - Puzzleverse".to_string()
    }
    ScreenState::Realm {
      clicked_realm_selector,
      confirm_delete,
      direct_message_user,
      is_mine,
      messages,
      new_chat,
      realm_asset,
      realm_id,
      realm_message,
      realm_name,
      realm_selector,
      realm_server,
      ..
    } => {
      bevy_egui::egui::TopBottomPanel::top("menu_bar").show(&ui, |ui| {
        if ui.button("🏠").clicked() {
          server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmChange { realm: puzzleverse_core::RealmTarget::Home }));
        }
        if ui.button("⎈").clicked() {
          if realm_selector.is_none() {
            *realm_selector = Some(RealmSelector::Player);
          }
        }
        ui.checkbox(&mut fullscreen, "Fullscreen");
      });
      bevy_egui::egui::SidePanel::left("toolbar").show(&ui, |ui| {
        bevy_egui::egui::CollapsingHeader::new("Realm").show(ui, |ui| {
          bevy_egui::egui::Grid::new("realm_grid").striped(true).spacing([40.0, 4.0]).show(ui, |ui| {
            ui.label("Name");
            ui.label(realm_name.as_str());
            ui.end_row();

            ui.label("URL");
            ui.label(realm_id.as_str());
            ui.end_row();

            if *is_mine {
              ui.add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new("Danger!!!").color(bevy_egui::egui::Color32::RED)));
              if ui.button("Delete").clicked() {
                *confirm_delete = true;
              }
              ui.end_row();
            } else {
              let realm_bookmarks = caches.bookmarks.get_mut(&puzzleverse_core::BookmarkType::Realm);
              let url = puzzleverse_core::RealmTarget::RemoteRealm { realm: realm_id.clone(), server: realm_server.clone() }.to_url();
              let was_bookmarked = realm_bookmarks.map(|b| b.contains(realm_id)).unwrap_or(false);
              let mut is_bookmarked = was_bookmarked;
              ui.checkbox(&mut is_bookmarked, "Bookmarked");
              if was_bookmarked != is_bookmarked {
                server_requests.send(ServerRequest::Deliver((if is_bookmarked {
                  puzzleverse_core::ClientRequest::BookmarkAdd
                } else {
                  puzzleverse_core::ClientRequest::BookmarkRemove
                })(puzzleverse_core::BookmarkType::Realm, url)));
                server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::Realm)));
              }
              if ui.button("Go to My Instance").clicked() {
                server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmChange {
                  realm: puzzleverse_core::RealmTarget::PersonalRealm(realm_asset.clone()),
                }));
              }
              ui.end_row();
            }
            if ui.button("Copy Link to Instance").clicked() {
              clipboard.set_contents(&puzzleverse_core::RealmTarget::RemoteRealm { realm: realm_id.clone(), server: realm_server.clone() }.to_url());
            }
            if ui.button("Copy Link to Personal Realm").clicked() {
              clipboard.set_contents(&puzzleverse_core::RealmTarget::PersonalRealm(realm_asset.clone()).to_url());
            }
          });
        });

        bevy_egui::egui::CollapsingHeader::new("Realm Chat").default_open(true).show(ui, |ui| {
          bevy_egui::egui::ScrollArea::vertical().id_source("realm_chat").show(ui, |ui| {
            bevy_egui::egui::Grid::new("realm_grid").striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
              for message in messages {
                ui.label(&message.sender).on_hover_text(&message.timestamp.with_timezone(&chrono::Local).format("%c").to_string());
                ui.add(bevy_egui::egui::Label::new(message.body.clone()).wrap(true));
                ui.end_row();
              }
            })
          });
          ui.horizontal(|ui| {
            let chatbox = ui.text_edit_singleline(realm_message);
            let send = ui.button("⮨");
            if chatbox.changed() && realm_message.ends_with('\n') || send.clicked() {
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::DirectMessageSend {
                recipient: direct_message_user.clone(),
                id: inflight_requests.push(InflightOperation::DirectMessage(direct_message_user.clone())),
                body: realm_message.clone(),
              }));
              realm_message.clear();
            }
          })
        });
        bevy_egui::egui::CollapsingHeader::new("Direct Chat").default_open(false).show(ui, |ui| {
          ui.horizontal(|ui| {
            if bevy_egui::egui::ComboBox::from_id_source("direct_chat")
              .selected_text(direct_message_user.as_str())
              .show_ui(ui, |ui| {
                for (user, info) in caches.direct_messages.iter() {
                  ui.selectable_value(
                    direct_message_user,
                    user.to_string(),
                    format!("{}{}", user, if info.last_viewed <= info.last_viewed { " *" } else { "" }),
                  );
                }
              })
              .response
              .changed()
            {
              for (mut selection, _) in player_names.iter_mut() {
                selection.set_selected(false);
              }
            }
            if ui.button("⊞").clicked() && new_chat.is_none() {
              *new_chat = Some(String::new());
            }
          });
          let mut info = caches.direct_messages.get_mut(direct_message_user);
          match player_names.iter_mut().filter(|(_, PlayerName(name))| name.as_str() == direct_message_user.as_str()).next() {
            Some((mut selection, _)) => {
              ui.horizontal(|ui| {
                ui.label("In this realm");
                if ui.button("Join Them").clicked() {
                  server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(
                    puzzleverse_core::RealmRequest::FollowRequest { player: direct_message_user.clone() },
                  )));
                }
                selection.set_selected(true);
              });
            }
            None => {
              ui.horizontal(|ui| {
                match info.as_ref().map(|i| &i.location).unwrap_or(&puzzleverse_core::PlayerLocationState::Unknown) {
                  puzzleverse_core::PlayerLocationState::Invalid => (),
                  puzzleverse_core::PlayerLocationState::Unknown => {
                    ui.label("Whereabouts unknown");
                  }
                  puzzleverse_core::PlayerLocationState::ServerDown => {
                    ui.label("Player's server is offline");
                  }
                  puzzleverse_core::PlayerLocationState::Offline => {
                    ui.label("Player is offline");
                  }
                  puzzleverse_core::PlayerLocationState::Online => {
                    ui.label("Player is online");
                  }
                  puzzleverse_core::PlayerLocationState::InTransit => {
                    ui.label("Player is in transit");
                  }
                  puzzleverse_core::PlayerLocationState::Realm(realm, server) => {
                    ui.label("Player is in online");
                    if ui.button("Join Them").clicked() {
                      server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmChange {
                        realm: puzzleverse_core::RealmTarget::RemoteRealm { realm: realm.clone(), server: server.clone() },
                      }));
                    }
                  }
                };
                if ui.button("Update").clicked() {
                  server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::PlayerCheck(direct_message_user.clone())));
                }
              });
            }
          }
          match info.as_deref_mut().filter(|l| !l.messages.is_empty()) {
            None => {
              ui.label("No messages");
            }
            Some(mut info) => {
              bevy_egui::egui::ScrollArea::vertical().id_source("direct_chat").show(ui, |ui| {
                bevy_egui::egui::Grid::new("direct_grid").striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                  info.last_message = chrono::Utc::now();
                  for message in info.messages.iter() {
                    ui.label(if message.inbound { direct_message_user.as_str() } else { "Me" })
                      .on_hover_text(&message.timestamp.with_timezone(&chrono::Local).format("%c").to_string());
                    ui.add(bevy_egui::egui::Label::new(message.body.to_string()).wrap(true));
                    ui.end_row();
                  }
                });
              });
            }
          }
          match info {
            None => (),
            Some(mut info) => {
              ui.horizontal(|ui| {
                let chatbox = ui.text_edit_singleline(&mut info.draft);
                let send = ui.button("⮨");
                if chatbox.changed() && info.draft.ends_with('\n') || send.clicked() {
                  server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(
                    puzzleverse_core::RealmRequest::SendMessage(info.draft.clone()),
                  )));
                  info.draft.clear();
                }
              });
            }
          }
        });
      });
      if let Some(realm_selector) = realm_selector {
        bevy_egui::egui::Window::new("Travel to Realm").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(
          &ui,
          |ui| {
            realm_selector.draw_ui(ui, &mut caches.known_realms, &mut server_requests, |realm| puzzleverse_core::ClientRequest::RealmChange { realm })
          },
        );
      }
      if let Some((path, key, point, realm_selector)) = clicked_realm_selector {
        bevy_egui::egui::Window::new("Set Realm in Puzzle").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(
          &ui,
          |ui| {
            realm_selector.draw_ui(ui, &mut caches.known_realms, &mut server_requests, |realm| {
              puzzleverse_core::ClientRequest::InRealm(puzzleverse_core::RealmRequest::Perform(
                path
                  .drain(..)
                  .chain(std::iter::once(puzzleverse_core::Action::Interaction {
                    at: point.clone(),
                    target: key.clone(),
                    interaction: puzzleverse_core::InteractionType::Realm(realm),
                    stop_on_failure: true,
                  }))
                  .collect(),
              ))
            });
          },
        );
      }
      let mut close_new_chat = false;
      if let Some(new_chat) = new_chat {
        bevy_egui::egui::Window::new("New Chat").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(&ui, |ui| {
          ui.horizontal(|ui| ui.text_edit_singleline(new_chat));
          ui.horizontal(|ui| {
            if ui.button("Start").clicked() && !new_chat.is_empty() {
              match caches.direct_messages.entry(new_chat.clone()) {
                std::collections::btree_map::Entry::Occupied(_) => (),
                std::collections::btree_map::Entry::Vacant(v) => {
                  v.insert(DirectMessageInfo {
                    messages: vec![],
                    last_viewed: chrono::DateTime::<chrono::Utc>::MIN_UTC,
                    last_message: chrono::DateTime::<chrono::Utc>::MIN_UTC,
                    location: puzzleverse_core::PlayerLocationState::Unknown,
                    draft: String::new(),
                  });
                }
              }
              close_new_chat = true;
            }
            if ui.button("Cancel").clicked() {
              close_new_chat = true;
            }
          });
        });
      }
      if close_new_chat {
        *new_chat = None;
      }
      if *confirm_delete {
        bevy_egui::egui::Window::new("Delete Realm").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(&ui, |ui| {
          ui.horizontal(|ui| ui.label("Are you sure you want to delete this realm?"));
          ui.horizontal(|ui| {
            if ui.button("Delete").clicked() {
              *confirm_delete = false;
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmDelete {
                id: inflight_requests.push(InflightOperation::RealmDeletion(realm_id.clone())),
                target: realm_id.clone(),
              }));
            }
            if ui.button("Cancel").clicked() {
              *confirm_delete = false;
            }
          });
        });
      }
      format!("{} - Puzzleverse", realm_name)
    }
    ScreenState::ServerSelection { insecure, server, player, error_message } => {
      bevy_egui::egui::Window::new("Connect to Server").anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0]).collapsible(false).show(
        &ui,
        |ui| {
          bevy_egui::egui::Grid::new("connect_grid").striped(true).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label("Server: ");
            ui.add(bevy_egui::egui::TextEdit::singleline(server).desired_width(300.0));
            ui.end_row();
            ui.label("Player: ");
            ui.add(bevy_egui::egui::TextEdit::singleline(player).desired_width(300.0));
            ui.end_row();
            if let Some(error_message) = error_message {
              ui.label("Error: ");
              ui.add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(error_message.to_string()).color(bevy_egui::egui::Color32::RED)));
              ui.end_row();
            }
            if *insecure {
              ui.label("Warning: ");
              ui.add(bevy_egui::egui::Label::new(
                bevy_egui::egui::RichText::new("Connection is unencrypted. I hope this is for debugging.").color(bevy_egui::egui::Color32::RED),
              ));
              ui.end_row();
            }
            if ui.button("Connect").clicked() {
              server_requests.send(ServerRequest::CheckAuthMethods {
                insecure: *insecure,
                server: server.clone(),
                player: player.clone(),
                key: auth_key.clone(),
              });
              next_ui = Some(ScreenState::Busy(format!("Contacting {}...", &server)))
            }
            if ui.button("Quit").clicked() {
              exit.send(bevy::app::AppExit);
            }
            ui.end_row();
            ui.add(bevy_egui::egui::Label::new(
              bevy_egui::egui::RichText::new(format!("v{}", self_update::cargo_crate_version!())).text_style(bevy_egui::egui::TextStyle::Small),
            ));
          });
        },
      );
      "Login - Puzzleverse".to_string()
    }
    ScreenState::Busy(message) => {
      bevy_egui::egui::CentralPanel::default().show(&ui, |ui| {
        ui.label(message.as_str());
      });
      "Puzzleverse".to_string()
    }
    ScreenState::Waiting => {
      bevy_egui::egui::CentralPanel::default().show(&ui, |ui| {
        ui.label("Connecting...");
      });
      "Connecting - Puzzleverse".to_string()
    }
    ScreenState::Error(error) => {
      bevy_egui::egui::CentralPanel::default().show(&ui, |ui| {
        ui.label(error.as_str());
        if ui.button("Reconnect").clicked() {
          next_ui = Some(ScreenState::ServerSelection { error_message: None, insecure: false, player: String::new(), server: String::new() })
        }
        if ui.button("Quit").clicked() {
          exit.send(bevy::app::AppExit);
        }
      });
      "Error - Puzzleverse".to_string()
    }
  });
  let now = chrono::Utc::now();
  status_list.list.retain(|s| match s {
    StatusInfo::TimeoutFailure(_, time) => time < &now,
    StatusInfo::TimeoutSuccess(_, time) => time < &now,
    _ => true,
  });
  if !inflight_requests.outstanding.is_empty() || !status_list.list.is_empty() {
    bevy_egui::egui::Window::new("Status").anchor(bevy_egui::egui::Align2::LEFT_BOTTOM, [5.0, -5.0]).title_bar(false).show(&ui, |ui| {
      for outstanding in inflight_requests.outstanding.iter() {
        ui.horizontal(|ui| {
          ui.label(match &outstanding.operation {
            InflightOperation::AccessChange(access) => format!("Changing {}...", access),
            InflightOperation::DirectMessage(name) => format!("Sending message to {}...", name),
            InflightOperation::AssetCreation(asset) => format!("Uploading {}...", asset),
            InflightOperation::RealmCreation(realm) => format!("Creating {}...", realm),
            InflightOperation::RealmDeletion(realm) => format!("Deleting {}...", realm),
          })
        });
      }
      let mut dead = None;
      for (index, status) in status_list.list.iter().enumerate() {
        ui.horizontal(|ui| match status {
          StatusInfo::AcknowledgeFailure(message) => {
            ui.add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(message).color(bevy_egui::egui::Color32::RED)));
            if ui.button("×").clicked() {
              dead = Some(index);
            }
          }
          StatusInfo::RealmLink(link, message) => {
            ui.label(message);
            if ui.button("Go There").clicked() {
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::RealmChange { realm: link.clone() }));
              dead = Some(index);
            }
            if ui.button("×").clicked() {
              dead = Some(index);
            }
          }
          StatusInfo::TimeoutFailure(message, _) => {
            ui.add(bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(message).color(bevy_egui::egui::Color32::RED)));
          }
          StatusInfo::TimeoutSuccess(message, _) => {
            ui.label(message);
          }
        });
      }
      if let Some(index) = dead {
        status_list.list.remove(index);
      }
    });
  }
  window.set_mode(if fullscreen { bevy::window::WindowMode::BorderlessFullscreen } else { bevy::window::WindowMode::Windowed });
  if let Some(value) = next_ui {
    *screen = value;
  }
}

fn mouse_event(
  interaction_targets: bevy::ecs::system::Query<&InteractionTarget>,
  targets: bevy::ecs::system::Query<&Target>,
  mut picking_events: bevy::ecs::event::EventReader<bevy_mod_picking::PickingEvent>,
  player_names: bevy::ecs::system::Query<&PlayerName>,
  mut screen: bevy::ecs::system::ResMut<ScreenState>,
  mut realm: bevy::ecs::system::ResMut<RealmState>,
  server_requests: bevy::ecs::event::EventWriter<ServerRequest>,
) {
  fn find_path<'a, I: IntoIterator<Item = &'a puzzleverse_core::Point> + Clone>(
    click: bool,
    target: &puzzleverse_core::Point,
    current_path: I,
    paths: &Paths,
    platform_distances: &PlatformDistances,
  ) -> Option<Vec<puzzleverse_core::Action>> {
    let (mut path, _) = pathfinding::directed::astar::astar(
      &target,
      |p| paths.get(p).into_iter().flatten().map(|t| (t, 1)),
      |p| {
        current_path
          .clone()
          .into_iter()
          .filter_map(|c| {
            if c.platform == p.platform {
              Some(abs_difference(c.x, p.x) + abs_difference(c.y, p.y))
            } else {
              platform_distances.get(&(c.platform.min(p.platform), c.platform.max(p.platform))).copied()
            }
          })
          .min()
          .unwrap_or(u32::MAX)
      },
      |p| current_path.clone().into_iter().any(|c| c.platform == p.platform && c.x == p.x && c.y == p.y),
    )?;
    if path.is_empty() {
      return Some(vec![]);
    }
    path.reverse();
    let mut result: Vec<_> = current_path
      .into_iter()
      .take_while(|c| c.platform != path[0].platform && c.x == path[0].x && c.y == path[0].y)
      .cloned()
      .map(puzzleverse_core::Action::Move)
      .collect();
    result.extend(path.into_iter().cloned().map(puzzleverse_core::Action::Move));
    Some(result)
  }
  if let ScreenState::Realm { clicked_realm_selector, direct_message_user, paths, platform_distances, .. } = &mut *screen {
    for picking_event in picking_events.iter() {
      if let bevy_mod_picking::PickingEvent::Selection(bevy_mod_picking::SelectionEvent::JustSelected(entity)) = picking_event {
        if let Ok(PlayerName(name)) = player_names.get(*entity) {
          direct_message_user.clear();
          direct_message_user.push_str(&name);
        } else if let Ok(InteractionTarget { key, point, click }) = interaction_targets.get(*entity) {
          if let Some(mut actions) = find_path(*click, point, &paths, &platform_distances) {
            if *click {
              actions.push(puzzleverse_core::Action::Interaction {
                at: point.clone(),
                target: key.clone(),
                interaction: puzzleverse_core::InteractionType::Click,
                stop_on_failure: false,
              });
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(puzzleverse_core::RealmRequest::Perform(actions))))
            } else {
              let mut old_selection = None;
              std::mem::swap(&mut old_selection, &mut *clicked_realm_selector);
              server_requests
                .send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(puzzleverse_core::RealmRequest::Perform(actions.clone()))));
              *clicked_realm_selector =
                Some((actions, key.clone(), point.clone(), old_selection.map(|(_, _, _, state)| state).unwrap_or(RealmSelector::Player)));
              server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(puzzleverse_core::RealmRequest::Perform(vec![]))));
            }
          }
        } else if let Ok(Target(point)) = targets.get(*entity) {
          if let Some(mut actions) = find_path(false, point, &paths, &platform_distances) {
            server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::InRealm(puzzleverse_core::RealmRequest::Perform(actions))))
          }
        }
      }
    }
  }
}
fn process_request(
  mut asset_manager: bevy::ecs::system::ResMut<AssetManager>,
  mut caches: bevy::ecs::system::ResMut<Cache>,
  mut commands: bevy::ecs::system::Commands,
  mut current_access: bevy::ecs::system::ResMut<CurrentAccess>,
  mut exit: bevy::ecs::event::EventWriter<bevy::app::AppExit>,
  mut inflight_requests: bevy::ecs::system::ResMut<InflightRequests>,
  mut realm_state: bevy::ecs::system::ResMut<RealmState>,
  mut screen: bevy::ecs::system::ResMut<ScreenState>,
  mut server_requests: bevy::ecs::event::EventWriter<ServerRequest>,
  mut server_responses: bevy::ecs::event::EventReader<ServerResponse>,
  mut status_list: bevy::ecs::system::ResMut<StatusList>,
  thread_pool: bevy::ecs::system::Res<bevy::tasks::IoTaskPool>,
) {
  for response in server_responses.iter() {
    match response {
      ServerResponse::AuthMethod { insecure, server, player, scheme: puzzleverse_core::AuthScheme::Password } => {
        *screen = ScreenState::PasswordLogin {
          insecure: *insecure,
          server: server.clone(),
          player: player.clone(),
          password: String::new(),
          error_message: None,
        };
      }
      ServerResponse::AuthMethod { insecure, server, player, scheme: puzzleverse_core::AuthScheme::OpenIdConnect } => {
        *screen = ScreenState::Busy("Waiting for OpenID Connect to complete...".to_string());
        server_requests.send(ServerRequest::OpenIdConnectLogin { insecure: *insecure, server: server.clone(), player: player.clone() });
      }
      ServerResponse::AuthMethod { insecure, server, player, scheme: puzzleverse_core::AuthScheme::Kerberos } => {
        *screen = ScreenState::Busy("Waiting for Kerberos to complete...".to_string());
        server_requests.send(ServerRequest::KerberosLogin { insecure: *insecure, server: server.clone(), player: player.clone() });
      }
      ServerResponse::AuthMethodFailed { insecure, server, player, error_message } => {
        *screen = ScreenState::ServerSelection {
          insecure: *insecure,
          server: server.clone(),
          player: player.clone(),
          error_message: Some(error_message.clone()),
        };
      }
      ServerResponse::AuthPasswordFailed { insecure, server, player, password, error_message } => {
        *screen = ScreenState::PasswordLogin {
          insecure: *insecure,
          server: server.clone(),
          player: player.clone(),
          password: password.clone(),
          error_message: Some(error_message.clone()),
        };
      }
      ServerResponse::Connected => {
        *screen = ScreenState::Waiting;
        server_requests.send_batch(
          vec![
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::Capabilities),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::DirectMessageStats),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::ConsensualEmote)),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::DirectedEmote)),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::Emote)),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::Realm)),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::RealmAsset)),
            ServerRequest::Deliver(puzzleverse_core::ClientRequest::BookmarksGet(puzzleverse_core::BookmarkType::Server)),
          ]
          .into_iter(),
        )
      }
      ServerResponse::Disconnected => {
        *screen = ScreenState::ServerSelection { insecure: false, server: String::new(), player: String::new(), error_message: None };
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::AccessChange { id, response }) => {
        if let Some(InflightOperation::AccessChange(kind)) = inflight_requests.finish(*id) {
          status_list.list.push(match response {
            puzzleverse_core::AccessChangeResponse::Denied => {
              StatusInfo::AcknowledgeFailure(format!("Not allowed to update access list for {}.", kind))
            }
            puzzleverse_core::AccessChangeResponse::Changed => {
              StatusInfo::TimeoutSuccess(format!("Access list for {} updated.", kind), chrono::Utc::now() + chrono::Duration::seconds(10))
            }
            puzzleverse_core::AccessChangeResponse::InternalError => {
              StatusInfo::TimeoutSuccess(format!("Server error updating {} access list.", kind), chrono::Utc::now() + chrono::Duration::seconds(10))
            }
          });
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::Asset(name, asset)) => {
        if !name.chars().all(|c| c.is_alphanumeric()) {
          eprintln!("Garbage asset {} from server. Dropping.", &name);
        } else {
          asset_manager.0.push(name, asset);
          let new_screen = match (&mut *screen, *realm_state) {
            (ScreenState::Loading { assets }, RealmState::Active { asset, .. }) => {
              if assets.remove(name) && assets.is_empty() {
                match asset_manager.pull(&asset) {
                  Err(e) => Some(ScreenState::Lost(
                    RealmSelector::Player,
                    Some(std::borrow::Cow::Owned(format!(
                      "An error occurred trying to load this realm. The realm asset is missing or corrupt even though it was okay previously: {:?}",
                      e
                    ))),
                  )),
                  Ok(asset) => {
                    let task = thread_pool.spawn(load_realm(asset_manager.clone(), asset));
                    commands.spawn().insert(LoadRealmTask { task });
                    Some(ScreenState::Waiting)
                  }
                }
              } else {
                None
              }
            }
            _ => None,
          };
          if let Some(new_screen) = new_screen {
            *screen = new_screen;
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::AssetCreationFailed { id, error }) => {
        if let Some(InflightOperation::AssetCreation(asset)) = inflight_requests.finish(*id) {
          status_list.list.push(StatusInfo::AcknowledgeFailure(match error {
            puzzleverse_core::AssetError::PermissionError => format!("Not allowed to create {}", asset),
            puzzleverse_core::AssetError::Invalid => format!("Asset {} is not valid.", asset),
            puzzleverse_core::AssetError::Missing(assets) => {
              format!("Asset {} references other assets that are not available: {}", asset, assets.join(", "))
            }
            puzzleverse_core::AssetError::UnknownKind => format!("Asset {} is not supported by the server.", asset),
            puzzleverse_core::AssetError::DecodeFailure => format!("Asset {} seems to be corrupt.", asset),
          }));
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::AssetCreationSucceeded { id, hash }) => {
        if let Some(InflightOperation::AssetCreation(asset)) = inflight_requests.finish(*id) {
          status_list
            .list
            .push(StatusInfo::TimeoutSuccess(format!("Uploaded {} as {}", asset, &hash), chrono::Utc::now() + chrono::Duration::seconds(3)));
          // TODO: the hash should get used somehow
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::AssetUnavailable(asset)) => {
        let missing = if let ScreenState::Loading { assets } = *screen { !assets.contains(asset) } else { false };
        if missing {
          *screen = ScreenState::Lost(
            RealmSelector::Player,
            Some(std::borrow::Cow::Owned(format!("The asset {} is not available on the server. This realm cannot be loaded.", asset))),
          );
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::Bookmarks(key, values)) => {
        caches.bookmarks.insert(*key, values.iter().cloned().collect());
        if key == &puzzleverse_core::BookmarkType::Player {
          for player in values.iter() {
            match caches.direct_messages.entry(player.clone()) {
              std::collections::btree_map::Entry::Occupied(_) => (),
              std::collections::btree_map::Entry::Vacant(v) => {
                v.insert(DirectMessageInfo {
                  last_viewed: chrono::DateTime::<chrono::Utc>::MIN_UTC,
                  messages: vec![],
                  last_message: chrono::DateTime::<chrono::Utc>::MIN_UTC,
                  location: puzzleverse_core::PlayerLocationState::Unknown,
                  draft: String::new(),
                });
              }
            }
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::Capabilities { server_capabilities }) => {
        caches.allowed_capabilities =
          puzzleverse_core::CAPABILITIES.into_iter().filter(|c| server_capabilities.iter().any(|s| *c == s)).cloned().collect();
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::CheckAssets { asset }) => {
        for id in asset.into_iter().filter(|a| !asset_manager.check(a)).cloned() {
          server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::AssetPull { id }))
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::CurrentAccess { target, acls, default }) => {
        current_access.0.insert(target.clone(), (acls.clone(), default.clone()));
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::DirectMessages { player, messages }) => {
        match caches.direct_messages.entry(player.clone()) {
          std::collections::btree_map::Entry::Occupied(mut o) => {
            let existing_messages = &mut o.get_mut().messages;
            existing_messages.extend(messages.clone());
            existing_messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            existing_messages.dedup_by(|a, b| a.inbound == b.inbound && a.body.eq(&b.body));
          }
          std::collections::btree_map::Entry::Vacant(v) => {
            v.insert(DirectMessageInfo {
              last_viewed: chrono::DateTime::<chrono::Utc>::MIN_UTC,
              messages: messages.clone(),
              last_message: messages.iter().map(|m| m.timestamp).max().unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC),
              location: puzzleverse_core::PlayerLocationState::Unknown,
              draft: String::new(),
            });
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::DirectMessageReceived { sender, body, timestamp }) => {
        let message = puzzleverse_core::DirectMessage { inbound: true, body: body.clone(), timestamp: *timestamp };
        match caches.direct_messages.entry(sender.clone()) {
          std::collections::btree_map::Entry::Occupied(mut o) => {
            o.get_mut().messages.push(message);
            o.get_mut().last_message = *timestamp
          }
          std::collections::btree_map::Entry::Vacant(v) => {
            v.insert(DirectMessageInfo {
              last_viewed: chrono::DateTime::<chrono::Utc>::MIN_UTC,
              messages: vec![message],
              last_message: *timestamp,
              location: puzzleverse_core::PlayerLocationState::Unknown,
              draft: String::new(),
            });
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::DirectMessageStats { stats, last_login }) => {
        for (sender, timestamp) in stats {
          match caches.direct_messages.entry(sender.clone()) {
            std::collections::btree_map::Entry::Occupied(mut o) => {
              o.get_mut().last_message = *timestamp;
            }
            std::collections::btree_map::Entry::Vacant(v) => {
              v.insert(DirectMessageInfo {
                last_viewed: *last_login,
                messages: vec![],
                last_message: *timestamp,
                location: puzzleverse_core::PlayerLocationState::Unknown,
                draft: String::new(),
              });
            }
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::Disconnect) => {
        exit.send(bevy::app::AppExit);
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::DirectMessageReceipt { id, status }) => {
        if let Some(InflightOperation::DirectMessage(recipient)) = inflight_requests.finish(*id) {
          match status {
            puzzleverse_core::DirectMessageStatus::Delivered => (),
            puzzleverse_core::DirectMessageStatus::Forbidden => {
              status_list.list.push(StatusInfo::AcknowledgeFailure(format!("Sending direct messages to {} is not permitted.", recipient)))
            }
            puzzleverse_core::DirectMessageStatus::Queued => status_list
              .list
              .push(StatusInfo::TimeoutSuccess(format!("Sending messages to {}...", recipient), chrono::Utc::now() + chrono::Duration::seconds(5))),
            puzzleverse_core::DirectMessageStatus::InternalError => {
              status_list.list.push(StatusInfo::AcknowledgeFailure(format!("Internal server error while sending direct messages to {}.", recipient)))
            }
            puzzleverse_core::DirectMessageStatus::UnknownRecipient => {
              status_list.list.push(StatusInfo::AcknowledgeFailure(format!("Recipient {} is unknown.", recipient)))
            }
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::InTransit) => {
        *screen = ScreenState::InTransit;
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::PublicKeys(keys)) => {
        caches.public_keys = keys.into_iter().cloned().collect();
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::RealmsAvailable { display, realms }) => {
        caches.known_realms.insert(display.clone(), RealmInfo { last_updated: chrono::Utc::now(), realms: realms.clone() });
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::RealmChanged(change)) => match change {
        puzzleverse_core::RealmChange::Denied => {
          *screen = ScreenState::Lost(RealmSelector::Local, Some(std::borrow::Cow::Borrowed("Cannot travel to the realm you requested.")));
        }
        puzzleverse_core::RealmChange::Success { capabilities, name, asset, seed, settings, realm, server } => {
          let missing_capabilities: Vec<_> =
            capabilities.iter().filter(|c| !puzzleverse_core::CAPABILITIES.contains(&c.as_str())).map(|c| c.as_str()).collect();
          if missing_capabilities.is_empty() {
            *screen = ScreenState::Loading { assets: Default::default() };
            *realm_state = RealmState::Active {
              name: name.clone(),
              asset: asset.clone(),
              seed: *seed,
              settings: settings.clone(),
              realm: realm.clone(),
              server: server.clone(),
            };
            match asset_manager.pull(asset) {
              Err(puzzleverse_core::asset_store::LoadError::Unknown | puzzleverse_core::asset_store::LoadError::Corrupt) => {
                server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::AssetPull { id: asset.clone() }));
              }
              Err(puzzleverse_core::asset_store::LoadError::InternalError) => {
                *screen = ScreenState::Lost(
                  RealmSelector::Player,
                  Some(std::borrow::Cow::Owned(format!("Failed to load realm asset {} for realm {} due to an internal error.", asset, realm))),
                );
              }
              Ok(asset) => {
                let missing: std::collections::BTreeSet<_> = asset.children.iter().filter(|c| !asset_manager.check(c)).cloned().collect();
                if missing.is_empty() {
                  *screen = ScreenState::Waiting;
                  commands.spawn().insert(LoadRealmTask { task: thread_pool.spawn(load_realm(asset_manager.clone(), asset)) });
                } else {
                  for missing_asset in missing.iter().cloned() {
                    server_requests.send(ServerRequest::Deliver(puzzleverse_core::ClientRequest::AssetPull { id: missing_asset }));
                  }
                  *screen = ScreenState::Loading { assets: missing };
                }
              }
            }
          } else {
            *screen = ScreenState::Lost(
              RealmSelector::Player,
              Some(std::borrow::Cow::Owned(format!("Your client does not support {} required by this realm.", missing_capabilities.join(" nor ")))),
            );
          }
        }
      },
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::RealmCreation { id, status }) => {
        if let Some(InflightOperation::RealmCreation(realm)) = inflight_requests.finish(*id) {
          status_list.list.push(match status {
            puzzleverse_core::RealmCreationStatus::Created(principal) => {
              StatusInfo::RealmLink(puzzleverse_core::RealmTarget::LocalRealm(principal.clone()), format!("Realm has been created."))
            }
            puzzleverse_core::RealmCreationStatus::InternalError => {
              StatusInfo::AcknowledgeFailure(format!("Unknown error trying to create realm {}.", realm))
            }
            puzzleverse_core::RealmCreationStatus::TooManyRealms => {
              StatusInfo::AcknowledgeFailure(format!("Cannot create realm {}. You already have too many realms.", realm))
            }
            puzzleverse_core::RealmCreationStatus::Duplicate => {
              StatusInfo::AcknowledgeFailure(format!("Realm {} is a duplicate of an existing realm.", realm))
            }
          });
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::RealmDeletion { id, ok }) => {
        if let Some(InflightOperation::RealmDeletion(realm)) = inflight_requests.finish(*id) {
          status_list.list.push(if *ok {
            StatusInfo::TimeoutSuccess(format!("Realm {} has been deleted.", realm), chrono::Utc::now() + chrono::Duration::seconds(10))
          } else {
            StatusInfo::AcknowledgeFailure(format!("Cannot delete realm {}.", realm))
          });
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::Servers(servers)) => {
        caches.known_servers = servers.into_iter().cloned().collect();
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::PlayerState { player, state }) => {
        match caches.direct_messages.entry(player.clone()) {
          std::collections::btree_map::Entry::Vacant(v) => {
            v.insert(DirectMessageInfo {
              messages: vec![],
              last_viewed: chrono::Utc::now(),
              last_message: chrono::Utc::now(),
              location: state.clone(),
              draft: String::new(),
            });
          }
          std::collections::btree_map::Entry::Occupied(mut o) => {
            o.get_mut().location = state.clone();
          }
        }
      }
      ServerResponse::Deliver(puzzleverse_core::ClientResponse::InRealm(response)) => eprintln!("Got unhandled realm request: {:?}", response),
      ServerResponse::Deliver(r) => eprintln!("Got unhandled request: {:?}", r),
    }
  }
}

fn receive_network_events(connection: bevy::ecs::system::ResMut<ServerConnection>, mut network_events: bevy::ecs::event::EventReader<ServerRequest>) {
  for message in network_events.iter() {
    if let Err(e) = (*connection).outbound_tx.lock().unwrap().send(message.clone()) {
      panic!("Failed to send to server monitoring process: {}", e);
    }
  }
}

#[derive(Component)]
struct LoadRealmTask {
  task: bevy::tasks::Task<Result<puzzleverse_core::asset::AssetAnyRealm, puzzleverse_core::AssetError>>,
}
fn realm_converter(
  mut commands: bevy::ecs::system::Commands,
  mut load_tasks: bevy::ecs::system::Query<(bevy::ecs::entity::Entity, &mut LoadRealmTask)>,
  realm_state: bevy::ecs::system::Res<RealmState>,
  mut screen: bevy::ecs::system::ResMut<ScreenState>,
  mut ambient_light: bevy::ecs::system::ResMut<bevy::pbr::AmbientLight>,
  mut materials_assets: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::pbr::StandardMaterial>>,
  mut meshes: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::render::mesh::Mesh>>,
  mut scenes: bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::scene::Scene>>,
) {
  for (entity, mut task) in load_tasks.iter_mut() {
    if let Some(result) = futures_lite::future::block_on(futures_lite::future::poll_once(&mut task.task)) {
      match &*realm_state {
        RealmState::Active { asset, seed, .. } => match result {
          Err(puzzleverse_core::AssetError::DecodeFailure) => {
            *screen =
              ScreenState::Lost(RealmSelector::Player, Some(std::borrow::Cow::Borrowed("Failed to decode realm asset. Maybe the file is corrupt?")));
          }
          Err(puzzleverse_core::AssetError::Invalid) => {
            *screen = ScreenState::Lost(RealmSelector::Player, Some(std::borrow::Cow::Borrowed("Realm is not a valid.")));
          }
          Err(puzzleverse_core::AssetError::PermissionError) => {
            *screen =
              ScreenState::Lost(RealmSelector::Player, Some(std::borrow::Cow::Borrowed("Permission error reading realm. This shouldn't happen.")));
          }
          Err(puzzleverse_core::AssetError::UnknownKind) => {
            *screen =
              ScreenState::Lost(RealmSelector::Player, Some(std::borrow::Cow::Borrowed("Realm is not a valid type supported by this client.")));
          }
          Err(puzzleverse_core::AssetError::Missing(assets)) => {
            *screen = ScreenState::Lost(
              RealmSelector::Player,
              Some(std::borrow::Cow::Owned(format!(
                "Realm is missing assets, but they weren't included in the realm's manifest. This realm must be broken. Missing assets are {}",
                assets.join(" and ")
              ))),
            );
          }
          Ok(realm) => {
            fn convert_gradiators<T: IntoBevy>(
              gradiators: std::collections::BTreeMap<String, puzzleverse_core::asset::gradiator::Gradiator<T>>,
              bool_updates: &mut std::collections::BTreeMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>,
              num_updates: &mut std::collections::BTreeMap<String, std::sync::Arc<std::sync::atomic::AtomicU32>>,
            ) -> Result<std::collections::BTreeMap<String, crate::gradiator::Gradiator<T>>, E> {
              struct GradiatorVariables<'a> {
                bool_updates: &'a mut std::collections::BTreeMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>,
                num_updates: &'a mut std::collections::BTreeMap<String, std::sync::Arc<std::sync::atomic::AtomicU32>>,
              }
              impl<'a> puzzleverse_core::asset::gradiator::Resolver<String, String> for GradiatorVariables<'a> {
                type Bool = crate::gradiator::BoolUpdateState;
                type Num = crate::gradiator::NumUpdateState;
                fn resolve_bool(&mut self, value: String) -> Self::Bool {
                  crate::gradiator::BoolUpdateState(
                    match self.bool_updates.entry(value) {
                      std::collections::btree_map::Entry::Vacant(v) => {
                        let value = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        v.insert(value.clone());
                        value
                      }
                      std::collections::btree_map::Entry::Occupied(o) => o.get().clone(),
                    },
                    false,
                  )
                }
                fn resolve_num(&mut self, value: String, len: usize) -> Self::Num {
                  crate::gradiator::NumUpdateState(
                    match self.num_updates.entry(value) {
                      std::collections::btree_map::Entry::Vacant(v) => {
                        let value = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
                        v.insert(value.clone());
                        value
                      }
                      std::collections::btree_map::Entry::Occupied(o) => o.get().clone(),
                    },
                    0,
                  )
                }
              }

              gradiators
                .into_iter()
                .map(|(name, gradiator)| {
                  (
                    name,
                    crate::gradiator::Gradiator {
                      sources: gradiator.sources.into_iter().map(|s| s.resolve(&mut GradiatorVariables { bool_updates, num_updates })).collect(),
                      points: Default::default(),
                    },
                  )
                })
                .collect()
            }
            fn simple_realm(
              realm: puzzleverse_core::asset::SimpleRealmDescription<
                puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyAudio>,
                puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>,
                puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyCustom>,
              >,
              commands: &mut bevy::ecs::system::Commands,
              load_tasks: &mut bevy::ecs::system::Query<(bevy::ecs::entity::Entity, &mut LoadRealmTask)>,
              realm_state: &bevy::ecs::system::Res<RealmState>,
              ambient_light: &mut bevy::ecs::system::ResMut<bevy::pbr::AmbientLight>,
              materials_assets: &mut bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::pbr::StandardMaterial>>,
              meshes: &mut bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::render::mesh::Mesh>>,
              scenes: &mut bevy::ecs::system::ResMut<bevy::asset::Assets<bevy::scene::Scene>>,
              seed: i32,
            ) -> Result<ScreenState, String> {
              use bevy::prelude::BuildChildren;
              use bevy::render::mesh::*;
              use bevy::render::render_resource::*;
              let ground_square = meshes.add(shape::Box::new(1.0, 1.0, 0.05).into());
              let mut bool_updates = std::collections::BTreeMap::new();
              let mut num_updates = std::collections::BTreeMap::new();
              let gradiators_audio = convert_gradiators(realm.gradiators_audio, &mut bool_updates, &mut num_updates);
              let gradiators_color = convert_gradiators(realm.gradiators_color, &mut bool_updates, &mut num_updates);
              let gradiators_intensity = convert_gradiators(realm.gradiators_intensity, &mut bool_updates, &mut num_updates);
              let mut materials = Vec::new();
              let default_material = materials_assets.add(bevy::render::color::Color::rgb(0.5, 0.5, 0.5).into());
              //pub aesthetic: Aesthetic,
              for material in realm.materials {
                match material {
                  puzzleverse_core::asset::Material::BrushedMetal { color } => todo!(),
                }
              }

              //pub ambient_audio: Vec<AmbientAudio<A>>,
              //pub event_audio: Vec<EventAudio<A>>,

              let mut updates = std::collections::HashMap::new();

              updates.extend(
                bool_updates
                  .into_iter()
                  .map(|(name, target)| (puzzleverse_core::PropertyKey::BoolSink(name), vec![crate::update_handler::Update::BoolShared(target)])),
              );

              updates.extend(
                num_updates
                  .into_iter()
                  .map(|(name, target)| (puzzleverse_core::PropertyKey::NumSink(name), vec![crate::update_handler::Update::NumShared(target)])),
              );
              let mut settings = Default::default();
              let mut world_building_state = convert::WorldBuildingState {
                locals_color: Vec::new(),
                locals_intensity: Vec::new(),
                gradiators_color: &mut gradiators_color,
                gradiators_intensity: &mut gradiators_intensity,
                meshes: &mut meshes,
                occupied: Default::default(),
                seed,
                masks: &realm.masks,
                default_material: &mut default_material,
                settings: &mut settings,
                materials: &materials,
                updates: &mut updates,
              };
              ambient_light.color = convert::convert_global(realm.ambient_color, convert::AmbientLight, &mut world_building_state);
              ambient_light.brightness =
                convert::convert_global(realm.ambient_intensity, convert::AmbientLight, &mut world_building_state) as f32 * convert::MAX_ILLUMINATION;

              let mut paths: Paths = Default::default();
              let sprays = realm
                .sprays
                .into_iter()
                .enumerate()
                .map(|(index, spray)| {
                  spray::ConvertedSpray::new(spray, &mut meshes, seed, &mut world_building_state).ok_or(format!("Cannot convert spray {}", index))
                })
                .collect::<Result<Vec<_>, _>>()?;

              let walls: Vec<_> =
                realm.walls.into_iter().map(|wall| spray::ConvertedWall::new(wall, &mut meshes, seed, &mut world_building_state)).collect();

              for (platform_id, platform) in realm.platforms.into_iter().enumerate() {
                world_building_state.occupied.clear();
                let platform_normal = bevy::math::Quat::IDENTITY;
                let base = meshes.add(Mesh::from(match platform.base {
                  puzzleverse_core::asset::PlatformBase::Thin => shape::Box::new(platform.width as f32, platform.length as f32, 1.0),
                  puzzleverse_core::asset::PlatformBase::Box { thickness } => {
                    shape::Box::new(platform.width as f32, platform.length as f32, thickness as f32)
                  }
                }));
                let platform_commands = commands.spawn();
                platform_commands.insert_bundle(bevy::pbr::PbrBundle {
                  mesh: base,
                  material: match materials.get(platform.material as usize) {
                    Some(material) => material.at(platform.x, platform.y, platform.z, platform_commands.id(), &mut world_building_state),
                    None => default_material.clone(),
                  },
                  transform: Transform::from_translation(bevy::math::Vec3::new(platform.x as f32, platform.y as f32, platform.z as f32)),
                  ..Default::default()
                });
                for puzzleverse_core::asset::PlatformItem { x, y, item } in platform.contents {
                  match item {
                    puzzleverse_core::asset::PuzzleItem::Button { arguments, enabled, model, name, transformation, .. } => match *model {
                      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                        convert::add_mesh(
                          &mut commands,
                          &mut world_building_state,
                          Some(puzzleverse_core::InteractionKey::Button(name)),
                          None,
                          model,
                          arguments,
                          platform_id as u32,
                          x + platform.x,
                          y + platform.y,
                          platform.z,
                          transformation,
                        );
                      }
                    },
                    puzzleverse_core::asset::PuzzleItem::Switch { arguments, enabled, initial, model, name, transformation, .. } => match *model {
                      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                        convert::add_mesh(
                          &mut commands,
                          &mut world_building_state,
                          Some(puzzleverse_core::InteractionKey::Switch(name.clone())),
                          Some(puzzleverse_core::PropertyKey::BoolSink(name)),
                          model,
                          arguments,
                          platform_id as u32,
                          x + platform.x,
                          y + platform.y,
                          platform.z,
                          transformation,
                        );
                      }
                    },
                    puzzleverse_core::asset::PuzzleItem::CycleButton { arguments, enabled, model, name, states, transformation, .. } => {
                      match *model {
                        puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                          convert::add_mesh(
                            &mut commands,
                            &mut world_building_state,
                            Some(puzzleverse_core::InteractionKey::Button(name.clone())),
                            Some(puzzleverse_core::PropertyKey::NumSink(name)),
                            model,
                            arguments,
                            platform_id as u32,
                            x + platform.x,
                            y + platform.y,
                            platform.z,
                            transformation,
                          );
                        }
                      }
                    }
                    puzzleverse_core::asset::PuzzleItem::CycleDisplay { arguments, model, name, states, transformation } => match *model {
                      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                        convert::add_mesh(
                          &mut commands,
                          &mut world_building_state,
                          None,
                          Some(puzzleverse_core::PropertyKey::NumSink(name)),
                          model,
                          arguments,
                          platform_id as u32,
                          x + platform.x,
                          y + platform.y,
                          platform.z,
                          transformation,
                        );
                      }
                    },
                    puzzleverse_core::asset::PuzzleItem::Display { arguments, model, transformation } => match *model {
                      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                        convert::add_mesh(
                          &mut commands,
                          &mut world_building_state,
                          None,
                          None,
                          model,
                          arguments,
                          platform_id as u32,
                          x + platform.x,
                          y + platform.y,
                          platform.z,
                          transformation,
                        );
                      }
                    },
                    puzzleverse_core::asset::PuzzleItem::RealmSelector { arguments, model, name, transformation, .. } => match *model {
                      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
                        convert::add_mesh(
                          &mut commands,
                          &mut world_building_state,
                          Some(puzzleverse_core::InteractionKey::RealmSelector(name)),
                          None,
                          model,
                          arguments,
                          platform_id as u32,
                          x + platform.x,
                          y + platform.y,
                          platform.z,
                          transformation,
                        );
                      }
                    },
                    puzzleverse_core::asset::PuzzleItem::Proximity { .. } => (),
                    puzzleverse_core::asset::PuzzleItem::Custom {
                      item,
                      transformation,
                      gradiators_color,
                      gradiators_intensity,
                      materials,
                      settings,
                    } => match item {},
                  }
                }
                for (wall_id, wall_path) in platform.walls {
                  if let Some(Some(wall)) = walls.get(wall_id as usize) {
                    for segment in wall_path {
                      segment.plot_points(|x, y| {
                        let x = platform.x + x;
                        let y = platform.y + y;
                        let random = ((seed as i64).abs() as u64).wrapping_mul(x as u64).wrapping_mul(y as u64);

                        let is_solid = match wall {
                          spray::ConvertedWall::Solid { width, width_perturbation, material } => {
                            let width = *width + width_perturbation.compute(seed, x, y).1;

                            let mut spawn = commands.spawn();
                            let source = spawn.id();
                            spawn.insert_bundle(bevy::pbr::PbrBundle {
                              mesh: meshes.add(shape::Box::new(width, width, 1.0).into()),
                              material: material.at(x, y, platform.z, source, &mut world_building_state),
                              global_transform: Transform::from_xyz(x as f32 + 0.5, y as f32 + 0.5, platform.z as f32 + 0.5).into(),
                              ..Default::default()
                            });
                            true
                          }
                          spray::ConvertedWall::Fence { angle, posts, vertical, vertical_perturbation } => {
                            let index = random % posts.iter().map(|(weight, _)| (*weight).max(1) as u64).sum();
                            let mut accumulator = 0u32;
                            let (_, model) = posts
                              .iter()
                              .skip_while(|(weight, _)| {
                                accumulator += (*weight).max(1) as u32;
                                index < accumulator
                              })
                              .next()
                              .unwrap();
                            model.instantiate(
                              &mut commands,
                              x,
                              y,
                              platform.z,
                              seed,
                              angle,
                              if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                              vertical_perturbation,
                              &mut world_building_state,
                            );
                            true
                          }
                          spray::ConvertedWall::Gate { angle, model, vertical, vertical_perturbation } => {
                            model.instantiate(
                              &mut commands,
                              x,
                              y,
                              platform.z,
                              seed,
                              angle,
                              if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                              vertical_perturbation,
                              &mut world_building_state,
                            );
                            false
                          }
                          spray::ConvertedWall::Block { angle, identifier, model, vertical, vertical_perturbation } => {
                            let update = crate::update_handler::Update::BoolVisibility(
                              model
                                .instantiate(
                                  &mut commands,
                                  x,
                                  y,
                                  platform.z,
                                  seed,
                                  angle,
                                  if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                                  vertical_perturbation,
                                  &mut world_building_state,
                                )
                                .insert(bevy::render::view::visibility::Visibility { is_visible: true })
                                .id(),
                            );
                            world_building_state.updates.entry(puzzleverse_core::PropertyKey::BoolSink(identifier.clone())).or_default().push(update);
                            false
                          }
                        };
                        if is_solid {
                          world_building_state.occupied.insert((x, y));
                        }
                      })
                    }
                  }
                }

                for x in 0..=platform.width {
                  for y in 0..=platform.length {
                    if !world_building_state.occupied.contains(&(x, y)) {
                      if x > 0 {
                        if y > 0 && !world_building_state.occupied.contains(&(x - 1, y - 1)) {
                          paths
                            .entry(puzzleverse_core::Point { platform: platform_id as u32, x: x - 1, y: y - 1 })
                            .or_default()
                            .push(puzzleverse_core::Point { platform: platform_id as u32, x, y });
                        }
                        if !world_building_state.occupied.contains(&(x - 1, y)) {
                          paths
                            .entry(puzzleverse_core::Point { platform: platform_id as u32, x: x - 1, y })
                            .or_default()
                            .push(puzzleverse_core::Point { platform: platform_id as u32, x, y });
                        }
                        if y < platform.length && !world_building_state.occupied.contains(&(x - 1, y + 1)) {
                          paths
                            .entry(puzzleverse_core::Point { platform: platform_id as u32, x: x - 1, y: y + 1 })
                            .or_default()
                            .push(puzzleverse_core::Point { platform: platform_id as u32, x, y });
                        }
                      }
                      if y > 0 {
                        if !world_building_state.occupied.contains(&(x, y - 1)) {
                          paths
                            .entry(puzzleverse_core::Point { platform: platform_id as u32, x, y: y - 1 })
                            .or_default()
                            .push(puzzleverse_core::Point { platform: platform_id as u32, x, y });
                        }
                        if x < platform.width && !world_building_state.occupied.contains(&(x + 1, y - 1)) {
                          paths.entry(puzzleverse_core::Point { platform: platform_id as u32, x, y }).or_default().push(puzzleverse_core::Point {
                            platform: platform_id as u32,
                            x: x + 1,
                            y: y - 1,
                          });
                        }
                      }
                      let position = puzzleverse_core::Point { platform: platform_id as u32, x, y };
                      let x = platform.x + x;
                      let y = platform.y + y;
                      let random = ((seed as i64).abs() as u64).wrapping_mul(x as u64).wrapping_mul(y as u64);
                      let index = random
                        % platform
                          .sprays
                          .iter()
                          .copied()
                          .flat_map(|id| sprays.get(id as usize).into_iter())
                          .flat_map(|spray| spray.elements.iter())
                          .map(|(weight, _)| (*weight).max(1) as u64)
                          .sum();
                      let mut accumulator = 0u64;
                      match platform
                        .sprays
                        .iter()
                        .copied()
                        .flat_map(|id| sprays.get(id as usize).into_iter())
                        .flat_map(|spray| spray.elements.iter().map(|(weight, model)| (*weight, model, spray)))
                        .skip_while(|(weight, _, _)| {
                          accumulator += (*weight).max(1) as u64;
                          index < accumulator
                        })
                        .next()
                      {
                        Some((_, model, spray)) => {
                          let child = model
                            .instantiate(
                              &mut commands,
                              x,
                              y,
                              platform.z,
                              seed,
                              &spray.angle,
                              if spray.vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                              &spray.vertical_perturbation,
                              &mut world_building_state,
                            )
                            .id();
                          let mut commands = commands.spawn();
                          commands.add_child(child);
                          commands
                        }
                        None => commands.spawn(),
                      }
                      .with_children(|builder| {
                        let mut commands = builder.spawn();
                        commands.insert_bundle(bevy::pbr::PbrBundle {
                          mesh: ground_square,
                          material: match materials.get(platform.material as usize) {
                            Some(material) => material.at(x, y, platform.z, commands.id(), &mut world_building_state),
                            None => default_material.clone(),
                          },
                          transform: Transform::from_translation(bevy::math::Vec3::new(x as f32, y as f32, platform.z as f32)),
                          ..Default::default()
                        });
                      })
                      .insert(Target(position));
                    }
                  }
                }
              }
              Ok(ScreenState::Realm {
                clicked_realm_selector: None,
                confirm_delete: false,
                direct_message_user: Default::default(),
                is_mine: (),
                messages: Vec::new(),
                new_chat: None,
                paths,
                realm_asset: (),
                realm_id: (),
                realm_message: (),
                realm_name: (),
                realm_selector: (),
                realm_server: (),
              })
            }

            match realm {
              puzzleverse_core::asset::AssetAnyRealm::Simple(realm) => {
                let result = simple_realm(
                  realm,
                  &mut commands,
                  &mut load_tasks,
                  &realm_state,
                  &mut ambient_light,
                  &mut materials_assets,
                  &mut meshes,
                  &mut scenes,
                  *seed,
                );
                *screen = match result {
                  Err(e) => ScreenState::Error(e),
                  Ok(s) => s,
                }
              }
            }
          }
        },
        _ => {
          eprintln!("Finished loading realm when state was not active. Ignoring.");
        }
      }
      commands.entity(entity).remove::<LoadRealmTask>();
    }
  }
}
