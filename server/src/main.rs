mod asset_store;
mod auth;
mod client;
mod database;
mod html;
mod peer;
mod player_state;
mod prometheus_locks;
mod puzzle;
mod realm;
mod schema;
mod views;

use futures::SinkExt;
use futures::StreamExt;
use puzzleverse_core::Announcement;
use std::future::Future;
use std::io::prelude::*;

pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

slotmap::new_key_type! { pub struct PlayerKey; }
slotmap::new_key_type! { pub struct RealmKey; }
slotmap::new_key_type! { pub struct PeerKey; }

lazy_static::lazy_static! {
    static ref BUILD_ID: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_build_id", "Current server build ID.", &["build_id"]).unwrap();
}
lazy_static::lazy_static! {
    static ref BAD_CLIENT_REQUESTS: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_bad_client_requests", "Number of client requests that couldn't be decoded.", &["player"]).unwrap();
}

lazy_static::lazy_static! {
    static ref BAD_JWT: prometheus::IntCounter=
        prometheus::register_int_counter!("puzzleverse_bad_jwt", "Number of times a bad JWT was received from a client or server.").unwrap();
}
lazy_static::lazy_static! {
    static ref BAD_LINK: prometheus::IntCounter=
        prometheus::register_int_counter!("puzzleverse_bad_link", "Number of times a player was moved to an invalid link.").unwrap();
}
lazy_static::lazy_static! {
    static ref BAD_SERVER_REQUESTS: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_bad_server_requests", "Number of server requests that couldn't be decoded.", &["player"]).unwrap();
}
lazy_static::lazy_static! {
    static ref BAD_WEB_REQUEST: prometheus::IntCounter =
        prometheus::register_int_counter!("puzzleverse_bad_web_request", "Number of invalid HTTP requests.").unwrap();
}
lazy_static::lazy_static! {
    static ref FAILED_CLIENT_EVICT: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_failed_client_evict", "Number of client connections that produced an error while being evicted.", &["player"]).unwrap();
}
lazy_static::lazy_static! {
    static ref FAILED_SERVER_CALLBACK: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_failed_server_callback", "Number of times a server asked for a connection and then failed to be accessible.", &["server"]).unwrap();
}
lazy_static::lazy_static! {
    static ref FAILED_SERVER_EVICT: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_failed_server_evict", "Number of server connections that produced an error while being evicted.", &["server"]).unwrap();
}
lazy_static::lazy_static! {
static ref PEER_LOCK: prometheus_locks::labelled_mutex::PrometheusLabelled =
   prometheus_locks::labelled_mutex::PrometheusLabelled ::new( "puzzleverse_peer_server_connection_lock", "The number of seconds to acquire the peer server connection lock", "server").unwrap();
}
lazy_static::lazy_static! {
static ref PUZZLE_STATE_LOCK: prometheus_locks::labelled_mutex::PrometheusLabelled = prometheus_locks::labelled_mutex::PrometheusLabelled::new( "puzzleverse_realm_lock", "The number of seconds to acquire the lock for a realm", "realm").unwrap();
}
lazy_static::lazy_static! {
static ref PLAYER_AVATAR_LOCK: prometheus_locks::labelled_rwlock::PrometheusLabelled = prometheus_locks::labelled_rwlock::PrometheusLabelled::new( "puzzleverse_player_avatar_lock", "The number of seconds to acquire the lock for a player's avatar", "player").unwrap();
}
lazy_static::lazy_static! {
    static ref OUTSTANDING_ASSET_COUNT: prometheus::IntGauge =
        prometheus::register_int_gauge!("puzzleverse_outstanding_asset_count", "Number of assets the server is waiting on.").unwrap();
}
lazy_static::lazy_static! {
    static ref ASSET_LOAD_FAILURE: prometheus::IntCounterVec =
        prometheus::register_int_counter_vec!("puzzleverse_asset_load_failure", "Number of times an asset has failed loading.", &["reason"]).unwrap();
}
lazy_static::lazy_static! {
static ref DEFAULT_DENY_ACCESS: Vec<u8> = {
    let value: AccessControlSetting = ( puzzleverse_core::AccessDefault::Deny, vec![],);
    rmp_serde::encode::to_vec(&value).unwrap()
};
}
lazy_static::lazy_static! {
static ref DEFAULT_LOCAL_ONLY_ACCESS: Vec<u8> = {
    let value: AccessControlSetting = (
        puzzleverse_core::AccessDefault::Deny,
        vec![puzzleverse_core::AccessControl::AllowLocal(None)],
    );
    rmp_serde::encode::to_vec(&value).unwrap()
};
}
pub type AccessControlSetting = (puzzleverse_core::AccessDefault, Vec<puzzleverse_core::AccessControl>);

pub enum AssetPullAction {
  PushToPlayer(PlayerKey),
  LoadRealm(String, std::sync::Arc<std::sync::atomic::AtomicUsize>),
  AddToTrain(bool),
}

type OutgoingConnection<T, C> = futures::sink::With<
  futures::sink::SinkMapErr<
    futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<C>, tokio_tungstenite::tungstenite::protocol::Message>,
    fn(tokio_tungstenite::tungstenite::Error) -> PacketEncodingError,
  >,
  tokio_tungstenite::tungstenite::protocol::Message,
  T,
  futures::future::Ready<Result<tokio_tungstenite::tungstenite::protocol::Message, PacketEncodingError>>,
  fn(T) -> futures::future::Ready<Result<tokio_tungstenite::tungstenite::protocol::Message, PacketEncodingError>>,
>;

enum OutstandingId {
  Bookmarks { player: PlayerKey, results: std::collections::HashMap<PeerKey, Vec<puzzleverse_core::Realm>> },
  Manual { player: PlayerKey, realm: String, server: String },
}
#[derive(Debug)]
enum PacketEncodingError {
  Tungstenite(tokio_tungstenite::tungstenite::Error),
  Encoding(rmp_serde::encode::Error),
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct PlayerClaim {
  exp: usize,
  name: String,
}

enum RealmKind {
  Loaded(RealmKey),
  WaitingAsset { waiters: Vec<(PlayerKey, u64)> },
}

pub(crate) enum RealmMove {
  ToHome(PlayerKey),
  ToExistingRealm { player: PlayerKey, realm: String, server: Option<String> },
  ToRealm { player: PlayerKey, owner: String, asset: String },
  ToTrain { player: PlayerKey, owner: String, train: u16 },
}

/// The state management for the server
pub struct Server {
  access_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  admin_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  announcements: prometheus_locks::rwlock::PrometheusRwLock<Vec<Announcement>>,
  asset_store: Box<dyn puzzleverse_core::asset_store::AsyncAssetStore>,
  attempting_peer_contacts: prometheus_locks::mutex::PrometheusMutex<std::collections::HashSet<String>>,
  /// The key to check JWT from users during authorization
  /// The authentication provider that can determine what users can get a JWT to log in
  authentication: std::sync::Arc<dyn auth::AuthProvider>,
  /// A pool of database connections to be used as required
  database: crate::database::Database,
  id_sequence: std::sync::atomic::AtomicI32,
  jwt_decoding_key: jsonwebtoken::DecodingKey,
  /// The key to create JWT for users during authentication
  jwt_encoding_key: jsonwebtoken::EncodingKey,
  jwt_nonce_decoding_key: jsonwebtoken::DecodingKey,
  jwt_nonce_encoding_key: jsonwebtoken::EncodingKey,
  message_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  move_epoch: std::sync::atomic::AtomicU64,
  move_queue: prometheus_locks::mutex::PrometheusMutex<tokio::sync::mpsc::Sender<RealmMove>>,
  /// The self-hostname for this server
  name: String,
  outstanding_assets: prometheus_locks::mutex::PrometheusMutex<std::collections::HashMap<String, Vec<AssetPullAction>>>,
  /// While these need to match against a database record, they do not have the same information. Some information about a player is transient and lost if the server is restarted.
  players: prometheus_locks::rwlock::PrometheusRwLock<std::collections::HashMap<String, PlayerKey>>,
  /// The state information of active players
  player_states: prometheus_locks::rwlock::PrometheusRwLock<slotmap::DenseSlotMap<PlayerKey, player_state::PlayerState>>,
  push_assets: prometheus_locks::mutex::PrometheusMutex<tokio::sync::mpsc::Sender<(PlayerKey, String)>>,
  /// The state of information about active realms
  realms: prometheus_locks::mutex::PrometheusMutex<std::collections::HashMap<String, RealmKind>>,
  realm_states: prometheus_locks::rwlock::PrometheusRwLock<slotmap::DenseSlotMap<RealmKey, realm::RealmState>>,
  peer_states: prometheus_locks::rwlock::PrometheusRwLock<slotmap::DenseSlotMap<PeerKey, peer::PeerState>>,
  peers: prometheus_locks::rwlock::PrometheusRwLock<std::collections::HashMap<String, PeerKey>>,
  banned_peers: prometheus_locks::rwlock::PrometheusRwLock<std::collections::BTreeSet<String>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ServerConfiguration {
  asset_store: asset_store::AssetStoreConfiguration,
  authentication: crate::auth::AuthConfiguration,
  bind_address: Option<String>,
  certificate: Option<std::path::PathBuf>,
  database_url: String,
  default_realm: String,
  name: String,
  unix_sockets: std::collections::BTreeMap<String, bool>,
}

fn connection_has(value: &http::header::HeaderValue, needle: &str) -> bool {
  if let Ok(v) = value.to_str() {
    v.split(',').any(|s| s.trim().eq_ignore_ascii_case(needle))
  } else {
    false
  }
}

fn convert_key(input: &[u8]) -> String {
  use sha3::Digest;
  const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
  let mut digest = sha1::Sha1::new();
  digest.update(input);
  digest.update(WS_GUID);
  base64::encode(digest.finalize().as_slice())
}

pub(crate) fn encode_message<T: serde::Serialize + Sized>(
  input: T,
) -> futures::future::Ready<Result<tokio_tungstenite::tungstenite::protocol::Message, PacketEncodingError>> {
  futures::future::ready(match rmp_serde::to_vec(&input) {
    Ok(data) => Ok(tokio_tungstenite::tungstenite::Message::Binary(data)),
    Err(e) => Err(PacketEncodingError::Encoding(e)),
  })
}

#[cfg(feature = "wasm-client")]
fn etag_request(
  content_type: &'static str,
  contents: &'static [u8],
  req: http::Request<hyper::Body>,
) -> Result<http::Response<hyper::Body>, http::Error> {
  if req.headers().get("If-None-Match").map(|tag| tag.to_str().ok()).flatten().map(|v| v == build_id::get().to_string()).unwrap_or(false) {
    http::Response::builder().status(304).body(hyper::Body::empty())
  } else {
    http::Response::builder()
      .status(http::StatusCode::OK)
      .header("Content-Type", content_type)
      .header("ETag", build_id::get().to_string())
      .body(hyper::body::Bytes::from_static(contents).into())
  }
}
fn id_for_type(bookmark_type: &puzzleverse_core::BookmarkType) -> &'static str {
  match bookmark_type {
    puzzleverse_core::BookmarkType::Asset => "a",
    puzzleverse_core::BookmarkType::ConsensualEmote => "c",
    puzzleverse_core::BookmarkType::DirectedEmote => "E",
    puzzleverse_core::BookmarkType::Emote => "e",
    puzzleverse_core::BookmarkType::Player => "p",
    puzzleverse_core::BookmarkType::Realm => "r",
    puzzleverse_core::BookmarkType::RealmAsset => "R",
    puzzleverse_core::BookmarkType::Server => "s",
  }
}
fn jwt_expiry_time(duration_secs: u64) -> usize {
  (std::time::SystemTime::now() + std::time::Duration::from_secs(duration_secs)).duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as usize
}

fn load_cert<P: AsRef<std::path::Path>>(certificate_path: P) -> Result<tokio_native_tls::TlsAcceptor, String> {
  let mut f = std::fs::File::open(&certificate_path).map_err(|e| e.to_string())?;
  let mut buffer = Vec::new();
  f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
  let cert = native_tls::Identity::from_pkcs12(&buffer, "").map_err(|e| e.to_string())?;
  Ok(tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build().map_err(|e| e.to_string())?))
}
impl std::fmt::Display for PacketEncodingError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PacketEncodingError::Tungstenite(e) => e.fmt(f),
      PacketEncodingError::Encoding(e) => e.fmt(f),
    }
  }
}
impl Server {
  pub async fn add_train(&self, asset: &str, allowed_first: bool) {
    if self
      .load_realm_description(asset, None, false, |_, _, propagation_rules, _, _, _| {
        propagation_rules.iter().any(|rule| match rule.propagation_match {
          puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext => true,
          _ => false,
        })
      })
      .await
    {
      if let Err(e) = self.database.train_add(asset, allowed_first) {
        eprintln!("Failed to insert realm into train: {}", e);
      }
    }
  }
  async fn attempt_peer_server_connection(self: &std::sync::Arc<Server>, peer_name: &str) -> () {
    if self.banned_peers.read("attempt_connection").await.contains(peer_name) {
      return;
    }
    let mut peer_contacts = self.attempting_peer_contacts.lock("attempt_connection").await;
    if !peer_contacts.contains(peer_name)
      || match self.peers.read("attempt_peer_connection").await.get(peer_name) {
        None => true,
        Some(key) => match self.peer_states.read("attempt_peer_connection").await.get(key.clone()) {
          None => true,
          Some(state) => match &*state.connection.lock("attempt_connection").await {
            peer::PeerConnection::Online(_) => false,
            peer::PeerConnection::Dead(_, _) => true,
            peer::PeerConnection::Offline => true,
          },
        },
      }
    {
      peer_contacts.insert(peer_name.to_string());
      match peer_name.parse::<http::uri::Authority>() {
        Err(e) => {
          println!("Bad peer server name {}: {}", peer_name, e);
        }
        Ok(authority) => match hyper::Uri::builder().scheme(http::uri::Scheme::HTTPS).path_and_query("/api/server/v1").authority(authority).build() {
          Err(e) => {
            println!("Bad URL construction for server name {}: {}", peer_name, e);
          }
          Ok(uri) => match jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &peer::PeerClaim { exp: jwt_expiry_time(3600), name: self.name.clone() },
            &self.jwt_encoding_key,
          ) {
            Ok(token) => {
              let request = serde_json::to_vec(&peer::PeerConnectRequest { token, server: self.name.to_string() }).unwrap();
              let server_name = peer_name.to_string();
              tokio::spawn(async move {
                let mut delay = 30;
                loop {
                  let connector = hyper_tls::HttpsConnector::new();
                  let client = hyper::client::Client::builder().build::<_, hyper::Body>(connector);

                  match client
                    .request(hyper::Request::post(&uri).version(http::Version::HTTP_11).body(hyper::Body::from(request.clone())).unwrap())
                    .await
                  {
                    Err(e) => {
                      eprintln!("Failed contact to {}: {}", &server_name, e)
                    }
                    Ok(response) => {
                      if response.status() == http::StatusCode::OK {
                        return;
                      } else {
                        eprintln!("Failed to connect to peer server {}: {}", &server_name, response.status());
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                        delay = 500.min(delay + 15);
                      }
                    }
                  }
                }
              });
            }
            Err(e) => {
              BAD_JWT.inc();
              eprintln!("Error generation JWT: {}", e);
            }
          },
        },
      }
    }
  }
  async fn check_player_state(
    self: &std::sync::Arc<Server>,
    target_player: &str,
    requesting_player: &str,
    requesting_server: Option<&str>,
  ) -> puzzleverse_core::PlayerLocationState {
    match self.load_player(target_player, false, |_| None).await {
      Some((target_player_id, _)) => match self.player_states.read("check_player_state").await.get(target_player_id) {
        Some(target_state) => {
          let online_acl = target_state.online_acl.lock().await;
          if online_acl.0.check(online_acl.1.iter(), requesting_player, &requesting_server, &self.name) {
            let target_mutable_state = target_state.mutable.lock().await;
            match &target_mutable_state.connection {
              player_state::PlayerConnection::Local(_, _, _) => {
                let location_acl = target_state.location_acl.lock().await;
                if location_acl.0.check(location_acl.1.iter(), requesting_player, &requesting_server, &self.name) {
                  match &target_mutable_state.goal {
                    player_state::Goal::InRealm(realm_key, _) => match self.realm_states.read("check_player_state").await.get(realm_key.clone()) {
                      Some(realm_state) => puzzleverse_core::PlayerLocationState::Realm(realm_state.id.clone(), self.name.clone()),
                      None => puzzleverse_core::PlayerLocationState::Online,
                    },
                    player_state::Goal::OnPeer(peer_id, realm_id) => match realm_id {
                      Some(realm) => match self.peer_states.read("check_player_state").await.get(peer_id.clone()) {
                        None => puzzleverse_core::PlayerLocationState::Online,
                        Some(peer_state) => puzzleverse_core::PlayerLocationState::Realm(realm.clone(), peer_state.name.clone()),
                      },
                      None => puzzleverse_core::PlayerLocationState::Online,
                    },
                    _ => puzzleverse_core::PlayerLocationState::InTransit,
                  }
                } else {
                  puzzleverse_core::PlayerLocationState::Online
                }
              }
              _ => puzzleverse_core::PlayerLocationState::Offline,
            }
          } else {
            puzzleverse_core::PlayerLocationState::Unknown
          }
        }
        None => puzzleverse_core::PlayerLocationState::Unknown,
      },
      None => puzzleverse_core::PlayerLocationState::Invalid,
    }
  }

  async fn debut(self: &Server, player: &str) {
    let write_to_db = match self.players.read("debut").await.get(player) {
      Some(player_key) => match self.player_states.read("debut").await.get(player_key.clone()) {
        Some(player_state) => {
          if player_state.debuted.load(std::sync::atomic::Ordering::Relaxed) {
            false
          } else {
            player_state.debuted.store(true, std::sync::atomic::Ordering::Relaxed);
            true
          }
        }
        None => true,
      },
      None => true,
    };
    if write_to_db {
      if let Err(e) = self.database.player_debut(&player) {
        println!("Failed to debut player {}: {}", player, e);
      }
    }
  }
  async fn find_asset(self: &std::sync::Arc<Server>, id: &str, post_action: AssetPullAction) -> bool {
    let mut outstanding_assets = self.outstanding_assets.lock("find_asset").await;
    OUTSTANDING_ASSET_COUNT.set(outstanding_assets.len() as i64);
    if self.asset_store.check(id).await {
      true
    } else {
      match outstanding_assets.entry(id.to_string()) {
        std::collections::hash_map::Entry::Vacant(e) => {
          e.insert(vec![post_action]);
          let s = std::sync::Arc::downgrade(self);
          let id = id.to_string();
          tokio::spawn(async move {
            for attempt in 1..5 {
              use peer::PeerMessage;
              use rand::seq::SliceRandom;
              let mut peers: Vec<_> = match s.upgrade() {
                Some(server) => server.peer_states.read("find_asset").await.keys().into_iter().collect(),
                None => return,
              };
              peers.shuffle(&mut rand::thread_rng());
              for peer_id in peers {
                let server = match s.upgrade() {
                  Some(server) => server,
                  None => return,
                };
                let request_sent = match server.peer_states.read("find_asset").await.get(peer_id) {
                  Some(peer_state) => {
                    let mut connection = peer_state.connection.lock("find_asset").await;
                    if let peer::PeerConnection::Online(c) = &mut *connection {
                      if let Err(e) = c.send(PeerMessage::AssetsPull { assets: vec![id.clone()] }).await {
                        eprintln!("Failed to communicate with peer: {}", e);
                        false
                      } else {
                        true
                      }
                    } else {
                      false
                    }
                  }
                  None => false,
                };
                if request_sent {
                  // TODO: we should pick a threshold based on data...
                  tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
              }
              eprintln!("Failed to Find asset {} on attempt {}. Sleeping for a bit and will try again", &id, attempt);
              tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
          });
        }
        std::collections::hash_map::Entry::Occupied(mut e) => {
          e.get_mut().push(post_action);
        }
      }
      false
    }
  }

  async fn find_realm(
    self: std::sync::Arc<Server>,
    player_id: PlayerKey,
    player_name: &str,
    player_server: Option<String>,
    predicate: database::RealmScope<'_>,
  ) -> (player_state::Goal, Option<puzzleverse_core::RealmChange>) {
    let mut realms = self.realms.lock("find_realm").await;
    match self.database.realm_find(predicate) {
      Ok(None) => (player_state::Goal::Undecided, Some(puzzleverse_core::RealmChange::Denied)),
      Ok(Some((principal, asset))) => match realms.entry(principal.clone()) {
        std::collections::hash_map::Entry::Occupied(mut e) => match e.get_mut() {
          RealmKind::Loaded(key) => match self.realm_states.read("find_realm").await.get(key.clone()) {
            None => (player_state::Goal::Undecided, Some(puzzleverse_core::RealmChange::Denied)),
            Some(realm_state) => {
              let allowed = {
                let access_acl = realm_state.access_acl.lock().await;
                access_acl.0.check(access_acl.1.iter(), player_name, &player_server, &self.name)
              };
              if allowed {
                let asset = realm_state.asset.clone();
                if let Err(e) = self.push_assets.lock("find_realm").await.send((player_id.clone(), asset)).await {
                  eprintln!("Failed to start asset push process: {}", e);
                }
                (
                  crate::player_state::Goal::WaitingAssetTransfer(key.clone()),
                  Some(puzzleverse_core::RealmChange::Success {
                    realm: principal,
                    server: self.name.clone(),
                    name: realm_state.name.read().await.clone(),
                    asset: realm_state.asset.clone(),
                    capabilities: realm_state.capabilities.clone(),
                    seed: realm_state.seed,
                    settings: realm_state.puzzle_state.lock("find_realm").await.settings.clone(),
                  }),
                )
              } else {
                (crate::player_state::Goal::Undecided, Some(puzzleverse_core::RealmChange::Denied))
              }
            }
          },
          RealmKind::WaitingAsset { waiters } => {
            let current = self.move_epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            waiters.push((player_id, current));
            (player_state::Goal::ResolvingLink(current), None)
          }
        },
        std::collections::hash_map::Entry::Vacant(v) => {
          let current = self.move_epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
          let load =
            self.find_asset(&asset, AssetPullAction::LoadRealm(principal.clone(), std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)))).await;
          v.insert(RealmKind::WaitingAsset { waiters: vec![(player_id, current)] });
          if load {
            self.clone().load_realm(principal).await;
          }
          (player_state::Goal::ResolvingLink(current), None)
        }
      },
      Err(e) => {
        eprintln!("Failed to get realm: {}", e);
        (player_state::Goal::Undecided, Some(puzzleverse_core::RealmChange::Denied))
      }
    }
  }
  /// Deal with an incoming WebSocket from a player
  ///
  /// Each player's client will have on active WebSocket used for messages. Messages are asynchronously communicated between client and server, so this splits the responsibility of processing incoming and outgoing messages.
  async fn handle_client_websocket(
    self: std::sync::Arc<Server>,
    player_claim: PlayerClaim,
    ws: tokio_tungstenite::WebSocketStream<puzzleverse_core::net::IncomingConnection>,
    superuser: bool,
  ) {
    eprintln!("Connection for player: {}", &player_claim.name);
    let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (read, player, db_id) = {
      let (write, read) = ws.split();
      let (player, db_id) = self
        .load_player(&player_claim.name, true, |id| {
          Some(crate::player_state::PlayerConnection::Local(
            id,
            write
              .sink_map_err(PacketEncodingError::Tungstenite as fn(tokio_tungstenite::tungstenite::Error) -> PacketEncodingError)
              .with(crate::encode_message),
            active.clone(),
          ))
        })
        .await
        .unwrap();
      (read, player, db_id)
    };

    let server = self.clone();
    tokio::spawn(async move {
      read
        .for_each(|m| async {
          if active.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(tokio_tungstenite::tungstenite::protocol::Message::Binary(buf)) = m {
              {
                match rmp_serde::from_slice::<puzzleverse_core::ClientRequest>(&buf) {
                  Ok(req) => {
                    if client::process_client_request(&server, &player_claim.name, &player, db_id, superuser, req).await {
                      active.store(false, std::sync::atomic::Ordering::Relaxed)
                    }
                  }
                  Err(_) => BAD_CLIENT_REQUESTS.with_label_values(&[&player_claim.name]).inc(),
                }
              }
            }
          }
        })
        .await;
    });
  }
  /// Process an incoming HTTP request
  async fn handle_http_request(self: std::sync::Arc<Server>, req: http::Request<hyper::Body>) -> Result<http::Response<hyper::Body>, http::Error> {
    match (req.method(), req.uri().path()) {
      // For the root, provide an HTML PAGE with the web client
      (&http::Method::GET, "/") => http::Response::builder().header("Content-Type", "text/html; charset=utf-8").body(html::create_main().into()),
      (&http::Method::GET, "/metrics") => {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let mut buffer = vec![];
        let mf = prometheus::gather();
        encoder.encode(&mf, &mut buffer).unwrap();
        http::Response::builder().header(hyper::header::CONTENT_TYPE, encoder.format_type()).body(buffer.into())
      }
      // Describe what authentication scheme is used
      (&http::Method::GET, puzzleverse_core::net::AUTH_METHOD_PATH) => {
        let scheme = self.authentication.scheme();
        match serde_json::to_string(&scheme) {
          Err(e) => {
            BAD_WEB_REQUEST.inc();
            eprintln!("Failed to serialise authentication scheme: {}", e);
            http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(hyper::Body::empty())
          }
          Ok(auth_json) => http::Response::builder().status(http::StatusCode::OK).header("Content-Type", "application/json").body(auth_json.into()),
        }
      }
      // Deliver the webclient
      #[cfg(feature = "wasm-client")]
      (&http::Method::GET, "/puzzleverse-client_bg.wasm") => etag_request("application/wasm", include_bytes!("../puzzleverse-client_bg.wasm"), req),
      #[cfg(feature = "wasm-client")]
      (&http::Method::GET, "/puzzleverse-client.js") => etag_request("text/javascript", include_bytes!("../puzzleverse-client.js"), req),
      // Handle a new player connection by upgrading to a web socket
      (&http::Method::GET, puzzleverse_core::net::CLIENT_V1_PATH) => {
        self.open_websocket(req, |s, claim, ws| s.handle_client_websocket(claim, ws, false))
      }
      // Handle a new server connection by upgrading to a web socket
      (&http::Method::GET, "/api/server/v1") => self.open_websocket(req, Server::handle_server_websocket),
      (&http::Method::POST, puzzleverse_core::net::CLIENT_NONCE_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          BAD_WEB_REQUEST.inc();
          eprintln!("Failed to aggregate body: {}", e);
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
        }
        Ok(whole_body) => {
          use bytes::buf::Buf;
          match serde_json::from_reader::<_, String>(whole_body.reader()) {
            Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
            Ok(name) => match jsonwebtoken::encode(
              &jsonwebtoken::Header::default(),
              &PlayerClaim { exp: jwt_expiry_time(30), name },
              &self.jwt_nonce_encoding_key,
            ) {
              Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
              Err(e) => {
                eprintln!("Failed to encode JWT as nonce: {}", e);
                http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(hyper::Body::empty())
              }
            },
          }
        }
      },
      (&http::Method::POST, puzzleverse_core::net::CLIENT_KEY_PATH) => match hyper::body::aggregate(req).await {
        Err(e) => {
          BAD_WEB_REQUEST.inc();
          eprintln!("Failed to aggregate body: {}", e);
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
        }
        Ok(whole_body) => {
          use bytes::buf::Buf;
          match serde_json::from_reader::<_, puzzleverse_core::AuthPublicKey<String>>(whole_body.reader()) {
            Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
            Ok(data) => {
              match jsonwebtoken::decode::<PlayerClaim>(
                &data.nonce,
                &self.jwt_nonce_decoding_key,
                &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
              ) {
                Ok(player_claim) => {
                  match self.database.public_key_get(&player_claim.claims.name) {
                    Ok(public_keys) => {
                      for der in public_keys {
                        if let Ok(pkey) = openssl::pkey::PKey::public_key_from_der(der.as_slice()) {
                          if let Ok(mut verifier) = openssl::sign::Verifier::new(openssl::hash::MessageDigest::sha256(), &pkey) {
                            if let Err(e) = verifier.update(&data.nonce.as_bytes()) {
                              eprintln!("Signature verification error: {}", e);
                              continue;
                            }
                            if verifier.verify(&data.signature).unwrap_or(false) {
                              return match jsonwebtoken::encode(
                                &jsonwebtoken::Header::default(),
                                &PlayerClaim { exp: jwt_expiry_time(3600), name: player_claim.claims.name },
                                &self.jwt_encoding_key,
                              ) {
                                Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
                                Err(e) => {
                                  BAD_WEB_REQUEST.inc();
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
      (&http::Method::POST, "/api/server/v1") => match hyper::body::aggregate(req).await {
        Err(e) => {
          BAD_WEB_REQUEST.inc();
          eprintln!("Failed to aggregate body: {}", e);
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body(format!("Aggregation failed: {}", e).into())
        }
        Ok(whole_body) => {
          use bytes::buf::Buf;
          match serde_json::from_reader::<_, peer::PeerConnectRequest>(whole_body.reader()) {
            Err(e) => http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
            Ok(data) => match puzzleverse_core::parse_server_name(&data.server) {
              Some(peer_name) => {
                if self.banned_peers.read("handle_post").await.contains(&peer_name) {
                  http::Response::builder().status(http::StatusCode::FORBIDDEN).body("Access denied".into())
                } else {
                  match data.server.parse::<http::uri::Authority>() {
                    Ok(authority) => {
                      match hyper::Uri::builder().scheme(http::uri::Scheme::HTTPS).path_and_query("/api/server/v1").authority(authority).build() {
                        Ok(uri) => {
                          let server = self.clone();
                          tokio::spawn(async move {
                            use rand::RngCore;
                            let connector = hyper_tls::HttpsConnector::new();
                            let client = hyper::client::Client::builder().build::<_, hyper::Body>(connector);

                            match client
                              .request(
                                hyper::Request::get(uri)
                                  .version(http::Version::HTTP_11)
                                  .header(http::header::CONNECTION, "upgrade")
                                  .header(http::header::SEC_WEBSOCKET_VERSION, "13")
                                  .header(http::header::SEC_WEBSOCKET_PROTOCOL, "puzzleverse")
                                  .header(http::header::UPGRADE, "websocket")
                                  .header(http::header::SEC_WEBSOCKET_KEY, format!("pv{}", &mut rand::thread_rng().next_u64()))
                                  .header(http::header::AUTHORIZATION, format!("Bearer {}", data.token))
                                  .body(hyper::Body::empty())
                                  .unwrap(),
                              )
                              .await
                            {
                              Err(e) => {
                                FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                                eprintln!("Failed callback to {}: {}", data.server, e)
                              }
                              Ok(response) => {
                                if response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
                                  match hyper::upgrade::on(response).await {
                                    Ok(upgraded) => {
                                      server
                                        .handle_server_websocket(
                                          peer::PeerClaim { exp: jwt_expiry_time(3600), name: peer_name },
                                          tokio_tungstenite::WebSocketStream::from_raw_socket(
                                            upgraded,
                                            tokio_tungstenite::tungstenite::protocol::Role::Client,
                                            None,
                                          )
                                          .await,
                                        )
                                        .await
                                    }
                                    Err(e) => {
                                      FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                                      eprintln!("Failed to connect to {}: {}", data.server, e);
                                    }
                                  }
                                } else {
                                  FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                                  let status = response.status();
                                  match hyper::body::aggregate(response).await {
                                    Err(e) => eprintln!("Failed to connect to {} {}: {}", data.server, status, e),
                                    Ok(buf) => eprintln!(
                                      "Failed to connect to {} {}: {}",
                                      data.server,
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
                          FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                          http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(format!("{}", e).into())
                        }
                      }
                    }
                    Err(e) => {
                      FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                      http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(format!("{}", e).into())
                    }
                  }
                }
              }
              None => {
                FAILED_SERVER_CALLBACK.with_label_values(&[&data.server]).inc();
                http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Bad server name".into())
              }
            },
          }
        }
      },
      // For other URLs, see if the authentication mechanism is prepared to deal with them
      _ => match self.authentication.handle(req).await {
        auth::AuthResult::Failure => {
          http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body("Internal server error during authentication".into())
        }
        auth::AuthResult::NotHandled => http::Response::builder().status(http::StatusCode::NOT_FOUND).body("Not Found".into()),
        auth::AuthResult::Page(page) => page,
        auth::AuthResult::SendToken(user_name) => {
          match jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &PlayerClaim { exp: jwt_expiry_time(3600), name: user_name },
            &self.jwt_encoding_key,
          ) {
            Ok(token) => http::Response::builder().status(http::StatusCode::OK).body(token.into()),
            Err(e) => {
              BAD_WEB_REQUEST.inc();
              eprintln!("Error generation JWT: {}", e);
              http::Response::builder().status(http::StatusCode::INTERNAL_SERVER_ERROR).body("Failed to generate token".into())
            }
          }
        }
      },
    }
  }
  /// Deal with an incoming WebSocket from a federated server
  ///
  /// Each player's client will have on active WebSocket used for messages. Messages are asynchronously communicated between client and server, so this splits the responsibility of processing incoming and outgoing messages.
  async fn handle_server_websocket(
    self: std::sync::Arc<Server>,
    server_claim: peer::PeerClaim,
    ws: tokio_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
  ) {
    if self.banned_peers.read("handle_websocket").await.contains(&server_claim.name) {
      return;
    }
    let server_for_reader = self.clone();

    let (read, (peer_id, dead)) = {
      let mut peers = (*self).peers.write("open_socket").await;
      let (write, read) = ws.split();

      let mut new_state = peer::PeerConnection::Online(
        write
          .sink_map_err(PacketEncodingError::Tungstenite as fn(tokio_tungstenite::tungstenite::Error) -> PacketEncodingError)
          .with(crate::encode_message),
      );
      let queued_messages = self.database.remote_direct_messages_queued(&server_claim.name);
      match queued_messages {
        Ok(messages) => {
          new_state
            .send(peer::PeerMessage::DirectMessage(
              messages
                .into_iter()
                .map(|(body, recipient, timestamp, sender)| peer::PeerDirectMessage { sender, recipient, timestamp, body })
                .collect(),
            ))
            .await;
          if let Err(e) = self.database.remote_direct_messages_sent(&server_claim.name) {
            eprintln!("Failed to marked queued messages as sent for {}: {}", &server_claim.name, e)
          }
        }
        Err(e) => eprintln!("Failed to pull queued messages for {}: {}", &server_claim.name, e),
      }

      let mut active_states = (*self).peer_states.write("open_socket").await;
      (
        read,
        match peers.entry(server_claim.name.clone()) {
          std::collections::hash_map::Entry::Vacant(entry) => {
            let peer_id = active_states.insert(peer::PeerState {
              connection: PEER_LOCK.create(server_claim.name.clone(), new_state),
              interested_in_list: tokio::sync::Mutex::new(std::collections::HashSet::new()),
              interested_in_ids: tokio::sync::Mutex::new(std::collections::HashMap::new()),
              name: server_claim.name.clone(),
            });
            entry.insert(peer_id.clone());
            (peer_id, None)
          }
          std::collections::hash_map::Entry::Occupied(mut entry) => match active_states.get_mut(*entry.get()) {
            None => {
              let peer_id = active_states.insert(peer::PeerState {
                connection: PEER_LOCK.create(server_claim.name.clone(), new_state),
                interested_in_list: tokio::sync::Mutex::new(std::collections::HashSet::new()),
                interested_in_ids: tokio::sync::Mutex::new(std::collections::HashMap::new()),
                name: server_claim.name.clone(),
              });
              entry.insert(peer_id.clone());
              (peer_id, None)
            }
            Some(value) => {
              let mut current = value.connection.lock("handle_websocket").await;
              std::mem::swap(&mut new_state, &mut *current);
              (
                entry.get().clone(),
                match new_state {
                  peer::PeerConnection::Online(socket) => Some(socket),
                  peer::PeerConnection::Dead(_, queued) => {
                    for message in queued {
                      current.send(message).await;
                    }
                    None
                  }
                  peer::PeerConnection::Offline => None,
                },
              )
            }
          },
        },
      )
    };
    self.attempting_peer_contacts.lock("post_connect_success").await.remove(&server_claim.name);
    if let Some(mut dead_socket) = dead {
      if let Err(e) = dead_socket.close().await {
        FAILED_SERVER_EVICT.with_label_values(&[&server_claim.name]).inc();
        eprintln!("Error evicting old connection for server {}: {}", server_claim.name, e)
      }
    }

    tokio::spawn(async move {
      let s = server_for_reader.clone();
      read
        .for_each(|m| async {
          match m {
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Binary(buf)) => match rmp_serde::from_slice::<peer::PeerMessage>(&buf) {
              Ok(req) => peer::process_server_message(&s, &server_claim.name, &peer_id, req).await,
              Err(err) => {
                BAD_SERVER_REQUESTS.with_label_values(&[&server_claim.name]).inc();
                eprintln!("Error from {}: {}", server_claim.name, err)
              }
            },
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Ping(_)) => {
              eprintln!("Ping from {}", server_claim.name)
            }
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Pong(_)) => {
              eprintln!("Ping from {}", server_claim.name)
            }
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Text(message)) => {
              eprintln!("Text from {}: {}", server_claim.name, message)
            }
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Close(_)) => {
              eprintln!("Connection from {} is closing", server_claim.name)
            }
            Ok(tokio_tungstenite::tungstenite::protocol::Message::Frame(_)) => {
              eprintln!("Frame from {}; this shouldn't happen when reading", server_claim.name)
            }
            Err(err) => {
              BAD_SERVER_REQUESTS.with_label_values(&[&server_claim.name]).inc();
              eprintln!("Error from {}: {}", server_claim.name, err)
            }
          }
        })
        .await;
    });
  }

  async fn load_player<F: FnOnce(i32) -> Option<player_state::PlayerConnection>>(
    self: &std::sync::Arc<Server>,
    player_name: &str,
    create: bool,
    create_connection: F,
  ) -> Option<(PlayerKey, i32)> {
    fn new_state(
      player_name: &str,
      server: &Server,
      debuted: bool,
      avatar: crate::prometheus_locks::labelled_rwlock::PrometheusLabelledRwLock<'static, puzzleverse_core::avatar::Avatar>,
      message_acl: std::sync::Arc<tokio::sync::Mutex<AccessControlSetting>>,
      online_acl: std::sync::Arc<tokio::sync::Mutex<AccessControlSetting>>,
      location_acl: std::sync::Arc<tokio::sync::Mutex<AccessControlSetting>>,
      new_realm_access_acl: std::sync::Arc<tokio::sync::Mutex<AccessControlSetting>>,
      new_realm_admin_acl: std::sync::Arc<tokio::sync::Mutex<AccessControlSetting>>,
      new_connection: crate::player_state::PlayerConnection,
    ) -> crate::player_state::PlayerState {
      crate::player_state::PlayerState {
        debuted: debuted.into(),
        name: player_name.to_string(),
        principal: format!("{}@{}", player_name, server.name),
        server: Some(server.name.clone()),
        avatar,
        mutable: tokio::sync::Mutex::new(crate::player_state::MutablePlayerState { connection: new_connection, goal: player_state::Goal::Undecided }),
        message_acl,
        online_acl,
        location_acl,
        new_realm_access_acl,
        new_realm_admin_acl,
      }
    }
    let announcements = puzzleverse_core::ClientResponse::Announcements(self.announcements.read("load_player").await.clone());
    match self.database.player_load(&player_name, create) {
      Ok(Some(database::PlayerInfo { db_id, debuted, avatar, message_acl, online_acl, location_acl, access_acl, admin_acl })) => {
        let mut active_players = self.player_states.write("load_player").await;
        let (player, old_socket) = match self.players.write("load_player").await.entry(player_name.to_string()) {
          std::collections::hash_map::Entry::Vacant(entry) => {
            let mut connection = create_connection(db_id).unwrap_or(player_state::PlayerConnection::Offline);
            connection.send_local(announcements).await;
            let id = active_players.insert(new_state(
              &player_name,
              &self,
              debuted,
              avatar,
              message_acl,
              online_acl,
              location_acl,
              access_acl,
              admin_acl,
              connection,
            ));

            entry.insert(id.clone());
            (id, None)
          }
          std::collections::hash_map::Entry::Occupied(mut entry) => (
            entry.get().clone(),
            match active_players.get_mut(*entry.get()) {
              None => {
                let mut connection = create_connection(db_id).unwrap_or(player_state::PlayerConnection::Offline);
                connection.send_local(announcements).await;
                entry.insert(active_players.insert(new_state(
                  &player_name,
                  &self,
                  debuted,
                  avatar,
                  message_acl,
                  online_acl,
                  location_acl,
                  access_acl,
                  admin_acl,
                  connection,
                )));
                None
              }
              Some(value) => match create_connection(db_id) {
                None => None,
                Some(mut new_connection) => {
                  new_connection.send_local(announcements).await;
                  let mut old = value.mutable.lock().await;
                  std::mem::swap(&mut new_connection, &mut old.connection);
                  match new_connection {
                    crate::player_state::PlayerConnection::Local(_, socket, active) => {
                      active.store(false, std::sync::atomic::Ordering::Relaxed);
                      Some(socket)
                    }
                    crate::player_state::PlayerConnection::LocalDead(_, _, mut queued) => {
                      if let crate::player_state::Goal::InRealm(realm, _) = &old.goal {
                        if let Some(realm_state) = self.realm_states.read("load_player").await.get(realm.clone()) {
                          old
                            .connection
                            .send_change(
                              self,
                              puzzleverse_core::RealmChange::Success {
                                asset: realm_state.asset.clone(),
                                capabilities: realm_state.capabilities.clone(),
                                name: realm_state.name.read().await.clone(),
                                realm: realm_state.id.clone(),
                                seed: realm_state.seed,
                                server: self.name.clone(),
                                settings: realm_state.puzzle_state.lock("new_state").await.settings.clone(),
                              },
                            )
                            .await;
                        }
                      }
                      for message in queued.drain(..) {
                        old.connection.send_local(message).await;
                      }
                      None
                    }
                    _ => None,
                  }
                }
              },
            },
          ),
        };
        if let Some(mut old_socket) = old_socket {
          let pn = player_name.to_string();
          tokio::spawn(async move {
            match old_socket.send(puzzleverse_core::ClientResponse::Disconnect).await {
              Ok(_) => (),
              Err(e) => {
                eprintln!("Could not close replaced websocket: {}", e);
                FAILED_CLIENT_EVICT.with_label_values(&[&pn]).inc();
              }
            };
            if let Err(e) = old_socket.close().await {
              eprintln!("Error evicting old connection for player {}: {}", pn, e);
              FAILED_CLIENT_EVICT.with_label_values(&[&pn]).inc();
            }
          });
        }
        Some((player, db_id))
      }
      Ok(None) => None,
      Err(e) => {
        eprintln!("Failed to load player {}: {}", &player_name, e);
        None
      }
    }
  }
  pub async fn load_realm(self: std::sync::Arc<Server>, principal: String) {
    tokio::spawn(async move {
      match realm::RealmState::load(&self, &principal).await {
        Ok(state) => {
          let (access_default, access_acls) = (*state.access_acl.lock().await).clone();
          let asset = state.asset.clone();
          let success = puzzleverse_core::RealmChange::Success {
            realm: principal.clone(),
            server: self.name.clone(),
            name: state.name.read().await.clone(),
            asset: asset.clone(),
            capabilities: state.capabilities.clone(),
            seed: state.seed,
            settings: state.puzzle_state.lock("load_realm").await.settings.clone(),
          };
          let realm_key = self.realm_states.write("load_realm").await.insert(state);
          match self.realms.lock("load_realm").await.insert(principal, RealmKind::Loaded(realm_key.clone())) {
            None => eprintln!("Loaded a realm no one was interested in"),
            Some(RealmKind::Loaded(_)) => {
              panic!("Replaced an active realm. This should not happen.")
            }
            Some(RealmKind::WaitingAsset { mut waiters, .. }) => {
              let player_states = self.player_states.read("load_realm").await;
              for (player, epoch) in waiters.drain(..) {
                if let Some(player_state) = player_states.get(player.clone()) {
                  let mut mutable_player_state = player_state.mutable.lock().await;
                  if mutable_player_state.goal == player_state::Goal::ResolvingLink(epoch) {
                    let (new_goal, change) = if access_default.check(access_acls.iter(), &player_state.name, &player_state.server, &self.name) {
                      if let Err(e) = self.push_assets.lock("load_realm").await.send((player.clone(), asset.clone())).await {
                        eprintln!("Failed to start asset push process: {}", e);
                      }
                      (crate::player_state::Goal::WaitingAssetTransfer(realm_key.clone()), success.clone())
                    } else {
                      mutable_player_state.connection.release_player(&self).await;
                      (crate::player_state::Goal::Undecided, puzzleverse_core::RealmChange::Denied)
                    };
                    mutable_player_state.goal = new_goal;
                    mutable_player_state.connection.send_change(&self, change).await;
                  }
                }
              }
            }
          }
        }
        Err(e) => {
          eprintln!("Failed to load realm: {:?}", e);
        }
      }
    });
  }
  async fn load_realm_description<
    T,
    F: FnOnce(
      Vec<String>,
      Vec<Box<dyn crate::puzzle::PuzzleAsset>>,
      Vec<puzzleverse_core::asset::rules::PropagationRule<usize>>,
      crate::realm::navigation::RealmManifold,
      std::collections::BTreeMap<u8, puzzleverse_core::avatar::Effect>,
      std::collections::BTreeMap<String, puzzleverse_core::RealmSetting>,
    ) -> T,
  >(
    &self,
    asset: &str,
    seed: Option<i32>,
    error: T,
    process: F,
  ) -> T {
    let err = match self.asset_store.pull(asset).await {
      Ok(a) => match puzzleverse_core::asset::AssetAnyRealm::load(a, &self.asset_store).await {
        Ok((realm, capabilities)) => match realm {
          puzzleverse_core::asset::AssetAnyRealm::Simple(realm) => match realm::convert::convert_realm(realm, seed) {
            Ok((puzzle_assets, rules, manifold, effects, settings)) => {
              return process(capabilities, puzzle_assets, rules, manifold, effects, settings)
            }
            Err(e) => e,
          },
        },
        Err(e) => e,
      },
      Err(e) => match e {
        puzzleverse_core::asset_store::LoadError::Corrupt => puzzleverse_core::AssetError::Invalid,
        puzzleverse_core::asset_store::LoadError::InternalError => puzzleverse_core::AssetError::Missing(vec![asset.to_owned()]),
        puzzleverse_core::asset_store::LoadError::Unknown => puzzleverse_core::AssetError::Invalid,
      },
    };

    ASSET_LOAD_FAILURE
      .with_label_values(&[match &err {
        puzzleverse_core::AssetError::DecodeFailure => "decode-failure",
        puzzleverse_core::AssetError::Invalid => "invalid",
        puzzleverse_core::AssetError::Missing(_) => "missing",
        puzzleverse_core::AssetError::PermissionError => "permission-error",
        puzzleverse_core::AssetError::UnknownKind => "unknown-kind",
      }])
      .inc();
    eprintln!("Failed to load realm asset {}: {:?}", asset, err);
    error
  }
  async fn move_player(self: &std::sync::Arc<Server>, request: RealmMove) -> () {
    let (player_id, release_target) = match request {
      RealmMove::ToHome(id) => (id, peer::ReleaseTarget::Home),
      RealmMove::ToExistingRealm { player, server: peer_name, realm } => (
        player,
        peer::ReleaseTarget::Realm(
          realm,
          match peer_name {
            Some(name) => name,
            None => self.name.clone(),
          },
        ),
      ),
      RealmMove::ToRealm { player, owner, asset } => (
        player,
        match self.database.realm_upsert_by_asset(&owner, &asset) {
          Err(e) => {
            eprintln!("Failed to figure out realm {} for {}: {}", &asset, &owner, e);
            peer::ReleaseTarget::Transit
          }
          Ok(id) => peer::ReleaseTarget::Realm(id, self.name.clone()),
        },
      ),
      RealmMove::ToTrain { player, owner, train } => (
        player,
        match self.database.realm_upsert_by_train(&owner, train) {
          Err(e) => {
            eprintln!("Failed to figure out next realm in train after {} for {}: {}", train, &owner, e);
            peer::ReleaseTarget::Transit
          }
          Ok(None) => peer::ReleaseTarget::Home,
          Ok(Some(id)) => peer::ReleaseTarget::Realm(id, self.name.clone()),
        },
      ),
    };
    match self.player_states.read("move_player").await.get(player_id.clone()) {
      None => (),
      Some(state) => {
        let mut mutable_state = state.mutable.lock().await;
        let mut set_dead = None;
        let mut new_goal = None;
        match &mut mutable_state.connection {
          player_state::PlayerConnection::Offline => (),
          player_state::PlayerConnection::Local(db_id, connection, _) => {
            new_goal =
              Some(self.resolve_realm(&player_id, &state.name, &*state.avatar.read("move_player").await, None, *db_id, release_target).await);
            if let Err(e) = connection.send(puzzleverse_core::ClientResponse::InTransit).await {
              eprintln!("Failed to handoff player: {}", e);
              set_dead = Some(*db_id);
            }
          }
          player_state::PlayerConnection::LocalDead(db_id, _, queue) => {
            new_goal =
              Some(self.resolve_realm(&player_id, &state.name, &*state.avatar.read("move_player").await, None, *db_id, release_target).await);
            queue.push(puzzleverse_core::ClientResponse::InTransit);
          }
          player_state::PlayerConnection::FromPeer(player, peer_id) => match self.peer_states.read("move_player").await.get(peer_id.clone()) {
            None => {
              eprintln!("Trying to hand off player {} to missing sever.", &player)
            }
            Some(peer_state) => {
              peer_state.connection.lock("realm_move").await.send(peer::PeerMessage::VisitorRelease(player.clone(), release_target)).await;
              mutable_state.goal = player_state::Goal::OnPeer(peer_id.clone(), None);
            }
          },
        };
        if let Some((goal, change)) = new_goal {
          mutable_state.goal = goal;
          if let Some(change) = change {
            mutable_state.connection.send_change(self, change).await;
          }
        }
        if let Some(db_id) = set_dead {
          mutable_state.connection =
            player_state::PlayerConnection::LocalDead(db_id, chrono::Utc::now(), vec![puzzleverse_core::ClientResponse::InTransit]);
        }
      }
    }
  }
  async fn move_players_from_realm<T: IntoIterator<Item = (crate::PlayerKey, puzzleverse_core::asset::rules::RealmLink)>>(
    &self,
    realm_owner: &str,
    train: Option<u16>,
    links: T,
  ) {
    let queue = self.move_queue.lock("move_from_realm").await;
    for (player, link) in links {
      match link {
        puzzleverse_core::asset::rules::RealmLink::Global(realm, server_name) => {
          if let Err(e) =
            queue.send(RealmMove::ToExistingRealm { player, realm, server: if &server_name == &self.name { None } else { Some(server_name) } }).await
          {
            eprintln!("Failed to put player into move queue: {}", e);
          }
        }
        puzzleverse_core::asset::rules::RealmLink::Owner(asset) => {
          if let Err(e) = queue.send(RealmMove::ToRealm { player, owner: realm_owner.to_string(), asset }).await {
            eprintln!("Failed to put player into move queue: {}", e);
          }
        }
        puzzleverse_core::asset::rules::RealmLink::Spawn(_) => {
          eprintln!("Trying to move player to spawn point. This should have been dealt with already")
        }
        puzzleverse_core::asset::rules::RealmLink::Home => {
          if let Err(e) = queue.send(RealmMove::ToHome(player)).await {
            eprintln!("Failed to put player into move queue: {}", e);
          }
        }
        puzzleverse_core::asset::rules::RealmLink::TrainNext => {
          self.debut(realm_owner).await;
          if let Err(e) = queue
            .send(match train {
              Some(train) => RealmMove::ToTrain { player, owner: realm_owner.into(), train },
              None => RealmMove::ToHome(player),
            })
            .await
          {
            eprintln!("Failed to put player into move queue: {}", e);
          }
        }
      }
    }
  }
  fn open_websocket<
    R: 'static + Future<Output = ()> + Send,
    F: 'static + Send + FnOnce(std::sync::Arc<Server>, T, tokio_tungstenite::WebSocketStream<S>) -> R,
    T: 'static + serde::de::DeserializeOwned + Send,
    S: From<hyper::upgrade::Upgraded> + tokio::io::AsyncWrite + tokio::io::AsyncRead + Unpin + Send,
  >(
    self: std::sync::Arc<Server>,
    req: hyper::Request<hyper::Body>,
    handler: F,
  ) -> Result<hyper::Response<hyper::Body>, http::Error> {
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
          match jsonwebtoken::decode::<T>(&value[7..], &(*self).jwt_decoding_key, &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256)) {
            Err(e) => {
              BAD_JWT.inc();
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
        let is_upgrade = req.headers().get(http::header::CONNECTION).map_or(false, |v| connection_has(v, "upgrade"));
        let is_websocket_upgrade =
          req.headers().get(http::header::UPGRADE).and_then(|v| v.to_str().ok()).map_or(false, |v| v.eq_ignore_ascii_case("websocket"));
        let is_websocket_version_13 =
          req.headers().get(http::header::SEC_WEBSOCKET_VERSION).and_then(|v| v.to_str().ok()).map_or(false, |v| v == "13");
        if !is_http_11 || !is_upgrade || !is_websocket_upgrade || !is_websocket_version_13 {
          BAD_WEB_REQUEST.inc();
          return http::Response::builder()
            .status(http::StatusCode::UPGRADE_REQUIRED)
            .header(http::header::SEC_WEBSOCKET_VERSION, "13")
            .body("Expected Upgrade to WebSocket version 13".into());
        }
        match req.headers().get(http::header::SEC_WEBSOCKET_KEY) {
          Some(value) => {
            let accept = convert_key(value.as_bytes());
            tokio::spawn(async move {
              match hyper::upgrade::on(req).await {
                Err(e) => {
                  BAD_WEB_REQUEST.inc();
                  eprintln!("Upgrade error: {}", e);
                }
                Ok(upgraded) => {
                  handler(
                    self,
                    token_contents,
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
          None => {
            BAD_WEB_REQUEST.inc();
            http::Response::builder().status(http::StatusCode::BAD_REQUEST).body("Websocket key is not in header".into())
          }
        }
      }
      None => {
        BAD_WEB_REQUEST.inc();
        http::Response::builder().status(http::StatusCode::UNAUTHORIZED).body("Invalid or missing token. Please authenticate first".into())
      }
    }
  }

  async fn perform_on_peer_server<R: 'static + Send, F>(&self, peer_name: String, action: &str, func: F) -> R
  where
    for<'b> F: 'static + Send + FnOnce(&'b PeerKey, &'b peer::PeerState) -> std::pin::Pin<Box<dyn 'b + Future<Output = R> + Send>>,
  {
    let mut states = self.peer_states.write(action).await;
    match self.peers.write(action).await.entry(peer_name.clone()) {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let peer_id = o.get();
        match states.get(peer_id.clone()) {
          None => {
            let state = peer::PeerState {
              connection: PEER_LOCK.create(peer_name.clone(), peer::PeerConnection::Dead(chrono::Utc::now(), Vec::new())),
              interested_in_list: tokio::sync::Mutex::new(std::collections::HashSet::new()),
              interested_in_ids: tokio::sync::Mutex::new(std::collections::HashMap::new()),
              name: peer_name,
            };
            let result = func(peer_id, &state).await;
            let id = states.insert(state);
            o.insert(id);
            result
          }
          Some(state) => func(peer_id, state).await,
        }
      }
      std::collections::hash_map::Entry::Vacant(e) => {
        let state = peer::PeerState {
          connection: PEER_LOCK.create(peer_name.clone(), peer::PeerConnection::Dead(chrono::Utc::now(), Vec::new())),
          interested_in_list: tokio::sync::Mutex::new(std::collections::HashSet::new()),
          interested_in_ids: tokio::sync::Mutex::new(std::collections::HashMap::new()),
          name: peer_name,
        };
        let id = states.insert(state);
        let result = func(&id, &states[id.clone()]).await;
        e.insert(id);
        result
      }
    }
  }

  fn perform_player_actions(
    player: &PlayerKey,
    player_state: &mut crate::player_state::MutablePlayerState,
    realm_key: &RealmKey,
    puzzle_state: &mut crate::realm::RealmPuzzleState,
    mut actions: Vec<puzzleverse_core::Action>,
  ) {
    player_state.goal = player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Idle);
    let time = chrono::Utc::now();
    puzzle_state.committed_movements.retain(|m| *m.movement.time() > time - chrono::Duration::seconds(5));
    let mut remaining_actions = if let Some((_, remaining_actions, _)) = puzzle_state.active_players.get_mut(&player) {
      let mut new_list = Vec::new();
      std::mem::swap(&mut new_list, remaining_actions);
      new_list
    } else {
      Vec::new()
    };

    if let Some(last_position) = puzzle_state
      .committed_movements
      .iter()
      .filter(|m| &m.player == player)
      .max_by_key(|m| m.movement.time())
      .map(|m| m.movement.end_position())
      .flatten()
      .cloned()
    {
      // If the player has no last position, they must be in liminal space and can't move
      let mut stop = false;
      let mut current_time = time;
      let mut current_position = last_position.clone();
      let mut active_proximity: std::collections::HashSet<_> = puzzle_state.manifold.active_proximity(&last_position).collect();
      let mut cleaned_actions: Vec<_> = actions
        .drain(..)
        .skip_while(|action| &last_position != action.position())
        .scan(last_position.clone(), |previous, action| {
          let current = action.position().clone();
          let result = if previous.platform == current.platform {
            if puzzle_state.manifold.verify(&current)
              && puzzleverse_core::abs_difference(previous.x, current.x) < 2
              && puzzleverse_core::abs_difference(previous.y, current.y) < 2
            {
              Some(action)
            } else {
              None
            }
          } else {
            if puzzle_state.manifold.verify_join(previous, &current, true) {
              Some(action)
            } else {
              None
            }
          };
          *previous = current;
          result
        })
        .collect();
      for (index, action) in cleaned_actions.drain(..).enumerate() {
        if stop {
          remaining_actions.push(action);
        } else {
          let (movement, enter_pieces, leave_pieces) = match action {
            puzzleverse_core::Action::DirectedEmote { animation, at, direction, duration } => {
              current_position = at;
              let end_time = current_time + chrono::Duration::milliseconds(duration as i64);
              let result = puzzleverse_core::CharacterMotion::DirectedEmote {
                animation: puzzleverse_core::CharacterAnimation::Custom(animation),
                at: current_position.clone(),
                direction,
                start: current_time,
              };
              current_time = end_time;
              (result, vec![], vec![])
            }
            puzzleverse_core::Action::Emote { animation, at, duration } => {
              current_position = at;
              let end_time = current_time + chrono::Duration::milliseconds(duration as i64);
              let result = puzzleverse_core::CharacterMotion::Internal {
                from: current_position.clone(),
                to: current_position.clone(),
                start: current_time,
                end: end_time.clone(),
                animation: puzzleverse_core::CharacterAnimation::Custom(animation),
              };
              current_time = end_time;
              (result, vec![], vec![])
            }
            puzzleverse_core::Action::Interaction { at, target, interaction, stop_on_failure } => {
              stop = stop_on_failure;
              let (animation, duration) = puzzle_state
                .manifold
                .interaction_animation(&at, &target)
                .unwrap_or((&puzzleverse_core::CharacterAnimation::Confused, chrono::Duration::milliseconds(500)));
              let result = puzzleverse_core::CharacterMotion::Interaction {
                start: current_time.clone(),
                end: current_time + duration,
                animation: animation.clone(),
                interaction,
                at,
                target,
              };
              current_time = current_time + duration;
              (result, vec![], vec![])
            }
            puzzleverse_core::Action::Move(point) => {
              let (animation, duration, gated) = puzzle_state.manifold.animation(&current_position, &point, index == 0);
              let new_proximity: std::collections::HashSet<_> = puzzle_state.manifold.active_proximity(&point).collect();
              let enter_pieces = new_proximity.difference(&active_proximity).copied().collect();
              let leave_pieces = active_proximity.difference(&new_proximity).copied().collect();
              active_proximity = new_proximity;

              stop = gated;
              let result = puzzleverse_core::CharacterMotion::Internal {
                from: current_position,
                to: point.clone(),
                start: current_time,
                end: current_time + duration,
                animation: animation.clone(),
              };
              current_position = point;
              current_time = current_time + duration;
              (result, enter_pieces, leave_pieces)
            }
          };
          puzzle_state.committed_movements.push(crate::realm::PlayerMovement { player: player.clone(), movement, enter_pieces, leave_pieces });
        }
      }
    }
    if let Some((_, player_remaining_actions, _)) = puzzle_state.active_players.get_mut(&player) {
      std::mem::swap(player_remaining_actions, &mut remaining_actions);
    }
    player_state.goal = crate::player_state::Goal::InRealm(realm_key.clone(), crate::player_state::RealmGoal::Idle);
  }
  async fn populate_activity(&self, realms: &mut [puzzleverse_core::Realm]) {
    for realm in realms {
      if realm.activity == puzzleverse_core::RealmActivity::Unknown && realm.server.as_ref().map(|n| n == &self.name).unwrap_or(true) {
        let key = self
          .realms
          .lock("populate_activity")
          .await
          .get(&realm.name)
          .map(|key| match key {
            RealmKind::Loaded(key) => Some(key.clone()),
            _ => None,
          })
          .flatten();
        realm.activity = match key {
          Some(key) => match self.realm_states.read("populate_activity").await.get(key) {
            None => puzzleverse_core::RealmActivity::Deserted,
            Some(state) => match state.activity.load(std::sync::atomic::Ordering::Relaxed) {
              0 => puzzleverse_core::RealmActivity::Deserted,
              1..=19 => puzzleverse_core::RealmActivity::Quiet,
              20..=99 => puzzleverse_core::RealmActivity::Popular,
              100..=499 => puzzleverse_core::RealmActivity::Busy,
              _ => puzzleverse_core::RealmActivity::Crowded,
            },
          },
          None => puzzleverse_core::RealmActivity::Deserted,
        };
      }
    }
  }

  async fn process_realm_request(
    self: &std::sync::Arc<Server>,
    player_name: &str,
    player: &PlayerKey,
    server_name: Option<&str>,
    superuser: bool,
    request: puzzleverse_core::RealmRequest,
  ) -> bool {
    let player_states = self.player_states.read("realm_request").await;
    match player_states.get(player.clone()) {
      Some(player_state) => {
        let mut mutable_player_state = player_state.mutable.lock().await;
        let realm = match &(*mutable_player_state).goal {
          crate::player_state::Goal::InRealm(realm, _) => Some((false, realm.clone())),
          crate::player_state::Goal::WaitingAssetTransfer(realm) => Some((true, realm.clone())),
          _ => None,
        };

        let force_to_undecided = match realm {
          Some((change, realm_key)) => match (*self).realm_states.read("realm_request").await.get(realm_key.clone()) {
            Some(realm_state) => {
              if change {
                realm_state
                  .puzzle_state
                  .lock("process_realm_request")
                  .await
                  .spawn_player(
                    self,
                    &realm_state.name.read().await,
                    *realm_state.in_directory.read().await,
                    &realm_key,
                    player,
                    &mut mutable_player_state,
                  )
                  .await;
              }
              match request {
                puzzleverse_core::RealmRequest::ChangeName(name, in_directory) => {
                  if &realm_state.owner == &player_state.principal || superuser || {
                    let acl = realm_state.admin_acl.lock().await;
                    (*acl).0.check(acl.1.iter(), player_name, &server_name, &self.name)
                  } {
                    let new_name = match name {
                      Some(name_value) => {
                        let mut realm_name = realm_state.name.write().await;
                        realm_name.truncate(0);
                        realm_name.push_str(&name_value);
                        name_value
                      }
                      None => realm_state.name.read().await.clone(),
                    };
                    let new_in_directory = match in_directory {
                      Some(in_directory_value) => {
                        let mut realm_in_directory = realm_state.in_directory.write().await;
                        *realm_in_directory = in_directory_value;
                        in_directory_value
                      }
                      None => *realm_state.in_directory.read().await,
                    };
                    realm_state
                      .puzzle_state
                      .lock("process_realm_request")
                      .await
                      .process_realm_event(
                        &self,
                        realm_state.db_id,
                        Some((&player, &mut mutable_player_state)),
                        realm::Multi::Single(puzzleverse_core::RealmResponse::NameChanged(new_name.clone(), new_in_directory)),
                      )
                      .await;
                  }
                }
                puzzleverse_core::RealmRequest::ChangeSetting(name, setting) => {
                  if let Some(setting) = setting.clean() {
                    if &realm_state.owner == &player_state.principal || superuser || {
                      let acl = realm_state.admin_acl.lock().await;
                      (*acl).0.check(acl.1.iter(), player_name, &server_name, &self.name)
                    } {
                      let mut puzzle_state = realm_state.puzzle_state.lock("change_setting").await;
                      if let Some(value) = puzzle_state.settings.get_mut(&name) {
                        if value.type_matched_update(&setting) {
                          puzzle_state
                            .process_realm_event(
                              &self,
                              realm_state.db_id,
                              Some((&player, &mut mutable_player_state)),
                              realm::Multi::Single(puzzleverse_core::RealmResponse::SettingChanged(name, setting)),
                            )
                            .await;
                        }
                      }
                    }
                  }
                }
                puzzleverse_core::RealmRequest::ConsensualEmoteRequest { emote, player: target_player_name } => {
                  if &target_player_name != &player_state.principal {
                    if let Some(target_player_id) = self.players.read("consensual_emote").await.get(&target_player_name) {
                      if let Some(target_player_state) = self.player_states.read("consensual_emote").await.get(target_player_id.clone()) {
                        let mut target_mutable_player_state = target_player_state.mutable.lock().await;
                        if target_mutable_player_state.goal == player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Idle) {
                          let puzzle_state = realm_state.puzzle_state.lock("consensual_emote_request").await;
                          match (
                            puzzle_state.active_players.get(player).filter(|(_, actions, _)| actions.is_empty()).map(|(point, _, _)| point),
                            puzzle_state.active_players.get(target_player_id).filter(|(_, actions, _)| actions.is_empty()).map(|(point, _, _)| point),
                          ) {
                            (Some(initiator_position), Some(recipient_position)) => {
                              if initiator_position.platform == recipient_position.platform
                                && (puzzleverse_core::abs_difference(initiator_position.x, recipient_position.x) < 2
                                  || puzzleverse_core::abs_difference(initiator_position.y, recipient_position.y) < 2)
                              {
                                let epoch = realm_state.consent_epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                mutable_player_state.goal = player_state::Goal::InRealm(
                                  realm_key.clone(),
                                  player_state::RealmGoal::ConsensualEmote {
                                    emote: emote.clone(),
                                    initiator: player.clone(),
                                    initiator_position: initiator_position.clone(),
                                    recipient_position: recipient_position.clone(),
                                    epoch,
                                  },
                                );
                                target_mutable_player_state
                                  .connection
                                  .send(
                                    self,
                                    target_player_id,
                                    puzzleverse_core::RealmResponse::ConsensualEmoteRequest { id: epoch, emote, player: player_name.to_string() },
                                  )
                                  .await;
                              }
                            }
                            _ => (),
                          }
                        }
                      }
                    }
                  }
                }
                puzzleverse_core::RealmRequest::ConsensualEmoteResponse { id, ok } => {
                  let mut puzzle_state = realm_state.puzzle_state.lock("consensual_emote_repsonse").await;
                  let (reset, update) = if let player_state::Goal::InRealm(
                    found_realm_key,
                    player_state::RealmGoal::ConsensualEmote { emote, epoch, initiator, initiator_position, recipient_position },
                  ) = &mutable_player_state.goal
                  {
                    (
                      true,
                      if ok
                        && id == *epoch
                        && found_realm_key == &realm_key
                        && puzzle_state
                          .active_players
                          .get(initiator)
                          .map(|(position, actions, _)| position == initiator_position && actions.is_empty())
                          .unwrap_or(false)
                        && puzzle_state
                          .active_players
                          .get(player)
                          .map(|(position, actions, _)| position == recipient_position && actions.is_empty())
                          .unwrap_or(false)
                      {
                        let start = chrono::Utc::now() + chrono::Duration::milliseconds(300);
                        puzzle_state.committed_movements.extend_from_slice(&[
                          realm::PlayerMovement {
                            player: initiator.clone(),
                            movement: puzzleverse_core::CharacterMotion::ConsensualEmoteInitiator {
                              start: start.clone(),
                              animation: emote.clone(),
                              at: initiator_position.clone(),
                            },
                            leave_pieces: vec![],
                            enter_pieces: vec![],
                          },
                          realm::PlayerMovement {
                            player: initiator.clone(),
                            movement: puzzleverse_core::CharacterMotion::ConsensualEmoteRecipient {
                              start: start.clone(),
                              animation: emote.clone(),
                              at: recipient_position.clone(),
                            },
                            leave_pieces: vec![],
                            enter_pieces: vec![],
                          },
                        ]);
                        true
                      } else {
                        false
                      },
                    )
                  } else {
                    (false, false)
                  };
                  if reset {
                    mutable_player_state.goal = player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Idle);
                  }
                  if update {
                    let update = puzzle_state.make_update_state(&chrono::Utc::now(), &player_states).await;
                    puzzle_state.process_realm_event(self, realm_state.db_id, Some((player, &mut mutable_player_state)), update).await;
                  }
                }
                puzzleverse_core::RealmRequest::FollowRequest { player: target_player_name } => {
                  if &target_player_name != &player_state.principal {
                    if let Some(target_player_id) = self.players.read("follow_request").await.get(&target_player_name) {
                      if let Some(target_player_state) = self.player_states.read("follow_request").await.get(target_player_id.clone()) {
                        let location_acl = target_player_state.location_acl.lock().await;
                        let mut target_mutable_player_state = target_player_state.mutable.lock().await;
                        if target_mutable_player_state.goal == player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Idle) {
                          // This is messy, if the player is local, we can determine if the request is okay, but if they are a peer, we need the peer endpoint to do that, so we treat is as "ask" and let the peer server intercept the request.
                          let policy = match &player_state.server {
                            Some(_) => None,
                            None => puzzleverse_core::check_acls(location_acl.1.iter(), player_name, &player_state.server, &self.name),
                          };
                          match policy {
                            Some(true) => {
                              let mut puzzle_state = realm_state.puzzle_state.lock("follow_request").await;
                              let start = chrono::Utc::now() + chrono::Duration::milliseconds(300);
                              puzzle_state.committed_movements.retain(|m| m.movement.time() < &start || &m.player != player);
                              let initiator_position = match puzzle_state.active_players.get_mut(player) {
                                Some((initiator_position, movements, _)) => {
                                  movements.clear();
                                  Some(initiator_position.clone())
                                }
                                None => None,
                              };
                              let target_position =
                                puzzle_state.active_players.get(target_player_id).map(|(target_position, _, _)| target_position.clone());

                              if let Some(initiator_position) = initiator_position {
                                if let Some(target_position) = target_position {
                                  let leave_pieces = puzzle_state.manifold.active_proximity(&initiator_position).collect();
                                  let enter_pieces = puzzle_state.manifold.active_proximity(&target_position).collect();
                                  let to = puzzle_state.manifold.find_adjacent_or_same(&target_position);
                                  puzzle_state.committed_movements.extend_from_slice(&[
                                    realm::PlayerMovement {
                                      player: player.clone(),
                                      leave_pieces,
                                      movement: puzzleverse_core::CharacterMotion::Leave { from: initiator_position, start: start.clone() },
                                      enter_pieces: vec![],
                                    },
                                    realm::PlayerMovement {
                                      player: player.clone(),
                                      movement: puzzleverse_core::CharacterMotion::Enter {
                                        to,
                                        end: start.clone() + chrono::Duration::milliseconds(200),
                                      },
                                      leave_pieces: vec![],
                                      enter_pieces,
                                    },
                                  ]);
                                }
                              }
                            }
                            Some(false) => (),
                            None => {
                              let epoch = realm_state.consent_epoch.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                              mutable_player_state.goal =
                                player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Follow { initiator: player.clone(), epoch });
                              target_mutable_player_state
                                .connection
                                .send(
                                  self,
                                  target_player_id,
                                  puzzleverse_core::RealmResponse::FollowRequest { id: epoch, player: player_name.to_string() },
                                )
                                .await;
                            }
                          }
                        }
                      }
                    }
                  }
                }
                puzzleverse_core::RealmRequest::FollowResponse { id, ok } => {
                  let mut puzzle_state = realm_state.puzzle_state.lock("follow_response").await;
                  let (reset, update) = if let player_state::Goal::InRealm(found_realm_key, player_state::RealmGoal::Follow { epoch, initiator }) =
                    &mutable_player_state.goal
                  {
                    (
                      true,
                      if ok && id == *epoch && found_realm_key == &realm_key {
                        let start = chrono::Utc::now() + chrono::Duration::milliseconds(300);
                        puzzle_state.committed_movements.retain(|m| m.movement.time() < &start || &m.player != initiator);
                        let initiator_position = match puzzle_state.active_players.get_mut(initiator) {
                          Some((initiator_position, movements, _)) => {
                            movements.clear();
                            Some(initiator_position.clone())
                          }
                          None => None,
                        };
                        let target_position = puzzle_state.active_players.get(player).map(|(target_position, _, _)| target_position.clone());
                        if let Some(initiator_position) = initiator_position {
                          if let Some(target_position) = target_position {
                            let leave_pieces = puzzle_state.manifold.active_proximity(&initiator_position).collect();
                            let enter_pieces = puzzle_state.manifold.active_proximity(&target_position).collect();
                            let to = puzzle_state.manifold.find_adjacent_or_same(&target_position);
                            puzzle_state.committed_movements.extend_from_slice(&[
                              realm::PlayerMovement {
                                player: initiator.clone(),
                                leave_pieces,
                                movement: puzzleverse_core::CharacterMotion::Leave { from: initiator_position, start: start.clone() },
                                enter_pieces: vec![],
                              },
                              realm::PlayerMovement {
                                player: initiator.clone(),
                                movement: puzzleverse_core::CharacterMotion::Enter { to, end: start.clone() + chrono::Duration::milliseconds(200) },
                                leave_pieces: vec![],
                                enter_pieces,
                              },
                            ]);
                          }
                        }
                        true
                      } else {
                        false
                      },
                    )
                  } else {
                    (false, false)
                  };
                  if reset {
                    mutable_player_state.goal = player_state::Goal::InRealm(realm_key.clone(), player_state::RealmGoal::Idle);
                  }
                  if update {
                    let update = puzzle_state.make_update_state(&chrono::Utc::now(), &player_states).await;
                    puzzle_state.process_realm_event(self, realm_state.db_id, Some((player, &mut mutable_player_state)), update).await;
                  }
                }
                puzzleverse_core::RealmRequest::GetAccess { target } => {
                  let acls = match target {
                    puzzleverse_core::RealmAccessTarget::Access => &realm_state.access_acl,
                    puzzleverse_core::RealmAccessTarget::Admin => &realm_state.admin_acl,
                  }
                  .lock()
                  .await;
                  mutable_player_state
                    .connection
                    .send(self, player, puzzleverse_core::RealmResponse::AccessCurrent { target, acls: acls.1.clone(), default: acls.0.clone() })
                    .await;
                }
                puzzleverse_core::RealmRequest::GetMessages { from, to } => match self.database.realm_messages(realm_state.db_id, from, to) {
                  Err(e) => eprintln!("Failed to get messages: {}", e),
                  Ok(mut data) => {
                    mutable_player_state
                      .connection
                      .send(
                        self,
                        player,
                        puzzleverse_core::RealmResponse::Messages(
                          data.drain(..).map(|(sender, timestamp, body)| puzzleverse_core::RealmMessage { body, sender, timestamp }).collect(),
                        ),
                      )
                      .await
                  }
                },
                puzzleverse_core::RealmRequest::Kick(kick_player) => {
                  if &realm_state.owner == &player_state.principal || superuser || {
                    let acl = realm_state.admin_acl.lock().await;
                    (*acl).0.check(acl.1.iter(), player_name, &server_name, &self.name)
                  } {
                    if let Some(kicked_player_id) = self.players.read("kick").await.get(&kick_player) {
                      let mut links = std::collections::hash_map::HashMap::new();
                      links.insert(kicked_player_id.clone(), puzzleverse_core::asset::rules::RealmLink::Home);
                      realm_state.puzzle_state.lock("kick").await.yank(kicked_player_id, &mut links);
                      self.move_players_from_realm(&realm_state.owner, None, links).await;
                    }
                  }
                }
                puzzleverse_core::RealmRequest::NoOperation => (),
                puzzleverse_core::RealmRequest::Perform(actions) => {
                  let mut puzzle_state = realm_state.puzzle_state.lock("perform").await;
                  Server::perform_player_actions(&player, &mut *mutable_player_state, &realm_key, &mut *puzzle_state, actions)
                }
                puzzleverse_core::RealmRequest::SendMessage(body) => {
                  realm_state.activity.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                  realm_state
                    .puzzle_state
                    .lock("send_message")
                    .await
                    .process_realm_event(
                      &self,
                      realm_state.db_id,
                      Some((&player, &mut mutable_player_state)),
                      realm::Multi::Single(puzzleverse_core::RealmResponse::MessagePosted {
                        sender: match server_name {
                          Some(server) => {
                            format!("{}@{}", player_name, server)
                          }
                          None => player_name.to_string(),
                        },
                        body,
                        timestamp: chrono::Utc::now(),
                      }),
                    )
                    .await
                }
                puzzleverse_core::RealmRequest::SetAccess { id, target, acls, default } => {
                  mutable_player_state
                    .connection
                    .send(
                      self,
                      player,
                      puzzleverse_core::RealmResponse::AccessChange {
                        id,
                        response: if &realm_state.owner == &player_state.principal || superuser || {
                          let acl = realm_state.admin_acl.lock().await;
                          (*acl).0.check(acl.1.iter(), player_name, &server_name, &self.name)
                        } {
                          match self.database.realm_acl(realm_state.db_id, target, default, acls) {
                            Err(e) => {
                              println!("Failed to update realm name: {}", e);
                              puzzleverse_core::AccessChangeResponse::InternalError
                            }
                            Ok(_) => puzzleverse_core::AccessChangeResponse::Changed,
                          }
                        } else {
                          puzzleverse_core::AccessChangeResponse::Denied
                        },
                      },
                    )
                    .await
                }
              }
              false
            }
            None => true,
          },
          None => false,
        };
        if force_to_undecided {
          // Somehow, the player is in a dead realm; kick them into transit mode
          mutable_player_state.goal = crate::player_state::Goal::Undecided;
          mutable_player_state.connection.release_player(&self).await;
        }
        false
      }
      None => true,
    }
  }
  pub async fn receive_asset(
    self: &std::sync::Arc<Server>,
    source: &str,
    assets: impl IntoIterator<Item = (String, puzzleverse_core::asset::Asset)>,
  ) {
    let mut post_insert_actions = Vec::new();
    {
      let mut outstanding_assets = self.outstanding_assets.lock("receive_asset").await;
      for (asset, value) in assets {
        if &asset != &value.principal_hash() {
          eprintln!("Garbage asset {} from {}. Dropping.", &asset, source);
          continue;
        }
        if !self.asset_store.check(&asset).await {
          self.asset_store.push(&asset, &value).await;
        }
        if let Some(mut actions) = outstanding_assets.remove(&asset) {
          post_insert_actions.extend(actions.drain(..).map(|action| (asset.clone(), value.clone(), action)));
        }
      }
    }
    for (asset_name, asset_value, action) in post_insert_actions.drain(..) {
      match action {
        AssetPullAction::PushToPlayer(player) => {
          if let Some(player_state) = self.player_states.read("push_received_asset_to_player").await.get(player) {
            player_state.mutable.lock().await.connection.send_local(puzzleverse_core::ClientResponse::Asset(asset_name, asset_value)).await;
          }
        }
        AssetPullAction::AddToTrain(allowed_first) => {
          self.add_train(&asset_name, allowed_first).await;
        }
        AssetPullAction::LoadRealm(realm, counter) => {
          let mut missing_children = Vec::new();
          for ca in asset_value.children {
            if self.asset_store.missing(&ca).await {
              missing_children.push(ca);
            }
          }
          counter.fetch_add(missing_children.len(), std::sync::atomic::Ordering::Relaxed);
          for missing_child in missing_children {
            if self.find_asset(&missing_child, crate::AssetPullAction::LoadRealm(realm.clone(), counter.clone())).await {
              if counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) <= 1 {
                panic!("Invalid reference count for realm asset tracker");
              }
            }
          }
          if counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) <= 1 {
            // We were the last asset, trigger this to run
            self.clone().load_realm(realm).await;
          }
        }
      }
    }
  }
  async fn resolve_realm(
    self: &std::sync::Arc<Server>,
    player_id: &PlayerKey,
    player_name: &str,
    avatar: &puzzleverse_core::avatar::Avatar,
    server_name: Option<String>,
    db_id: i32,
    target: peer::ReleaseTarget,
  ) -> (player_state::Goal, Option<puzzleverse_core::RealmChange>) {
    match target {
      peer::ReleaseTarget::Home => {
        self.clone().find_realm(player_id.clone(), player_name, server_name, database::RealmScope::Train { owner: db_id, train: 0 }).await
      }
      peer::ReleaseTarget::Transit => (player_state::Goal::Undecided, Some(puzzleverse_core::RealmChange::Denied)),
      peer::ReleaseTarget::Realm(realm, server_name) => {
        if &server_name == &self.name {
          self.clone().find_realm(player_id.clone(), player_name, Some(server_name), database::RealmScope::ByPrincipal(&realm)).await
        } else {
          // Request hand-off to server
          let player = player_name.to_string();
          let avatar = avatar.clone();
          self
            .perform_on_peer_server(server_name, "visitor_send", move |peer_id, peer| {
              Box::pin(async move {
                peer.connection.lock("visitor_send").await.send(peer::PeerMessage::VisitorSend { player, realm: realm.clone(), avatar }).await;
                (player_state::Goal::OnPeer(peer_id.clone(), Some(realm)), None)
              })
            })
            .await
        }
      }
    }
  }

  async fn send_direct_message(&self, sender: &str, recipient: &str, body: String) -> puzzleverse_core::DirectMessageStatus {
    if sender == recipient {
      return puzzleverse_core::DirectMessageStatus::UnknownRecipient;
    }
    match self.database.direct_message_write(sender, recipient, &body) {
      Err(diesel::NotFound) => puzzleverse_core::DirectMessageStatus::UnknownRecipient,
      Err(e) => {
        eprintln!("Failed to send DM from {} to {}: {}", sender, recipient, e);
        puzzleverse_core::DirectMessageStatus::InternalError
      }
      Ok(timestamp) => {
        match self.players.read("direct_message").await.get(recipient) {
          None => {}
          Some(recipient_key) => match self.player_states.read("direct_message").await.get(recipient_key.clone()) {
            None => {}
            Some(state) => {
              state
                .mutable
                .lock()
                .await
                .connection
                .send_local(puzzleverse_core::ClientResponse::DirectMessageReceived { sender: sender.to_string(), body, timestamp })
                .await
            }
          },
        }
        puzzleverse_core::DirectMessageStatus::Delivered
      }
    }
  }

  async fn send_response_to_player_visiting_peer(
    &self,
    peer_id: &PeerKey,
    peer_name: &str,
    player: &str,
    goal: Option<player_state::Goal>,
    response: puzzleverse_core::ClientResponse,
  ) {
    if match self.players.read("response_to_visiting").await.get(player) {
      Some(player_id) => match self.player_states.read("response_to_visiting").await.get(player_id.clone()) {
        None => true,
        Some(playerstate) => {
          let mut state = playerstate.mutable.lock().await;
          match state.goal {
            crate::player_state::Goal::OnPeer(selected_peer, _) => {
              if &selected_peer == peer_id {
                let forward =
                  if let puzzleverse_core::ClientResponse::InRealm(puzzleverse_core::RealmResponse::FollowRequest { id, player }) = &response {
                    if let Some((name, server_name)) = match puzzleverse_core::PlayerIdentifier::new(&player, Some(peer_name)) {
                      puzzleverse_core::PlayerIdentifier::Local(name) => Some((name, peer_name.to_string())),
                      puzzleverse_core::PlayerIdentifier::Remote { server, player } => Some((player, server)),
                      puzzleverse_core::PlayerIdentifier::Bad => None,
                    } {
                      let location_acl = playerstate.location_acl.lock().await;
                      match puzzleverse_core::check_acls((*location_acl).1.iter(), &name, &Some(server_name), &self.name) {
                        Some(ok) => {
                          if let Some(state) = self.peer_states.read("response_to_visiting").await.get(peer_id.clone()) {
                            state
                              .connection
                              .lock("send_response_to_visitor")
                              .await
                              .send(peer::PeerMessage::RealmRequest {
                                player: player.clone(),
                                request: puzzleverse_core::RealmRequest::FollowResponse { id: *id, ok },
                              })
                              .await;
                          }
                          false
                        }
                        None => true,
                      }
                    } else {
                      false
                    }
                  } else {
                    true
                  };
                if forward {
                  state.connection.send_local(response).await;
                  if let Some(goal) = goal {
                    state.goal = goal;
                  }
                }
                false
              } else {
                true
              }
            }
            _ => true,
          }
        }
      },
      None => true,
    } {
      // If the peer server thinks a player is active and we don't, then tell that server we want the player back
      if let Some(state) = self.peer_states.read("visitor_mismatch").await.get(peer_id.clone()) {
        state.connection.lock("visitor_mismatch").await.send(peer::PeerMessage::VisitorYank(player.to_string())).await;
      }
    }
  }
  async fn update_announcements(self: std::sync::Arc<Server>) {
    let announcements = self.announcements.read("send_updates").await.clone();
    for (_, state) in &*self.player_states.read("update_announcements").await {
      state.mutable.lock().await.connection.send_local(puzzleverse_core::ClientResponse::Announcements(announcements.clone())).await;
    }
  }
}
/// Start the server. This is in a separate function from main because the tokio annotation mangles compile error information
async fn start() -> Result<(), Box<dyn std::error::Error>> {
  BUILD_ID.with_label_values(&[&build_id::get().to_string()]).inc();
  let mut configuration: ServerConfiguration = {
    let mut configuration_file: String = "puzzleverse.config".into();
    {
      let mut ap = argparse::ArgumentParser::new();
      ap.set_description("Puzzleverse Server");
      ap.refer(&mut configuration_file).add_option(&["-c", "--config"], argparse::Store, "Set the configuration JSON file");
      ap.parse_args_or_exit();
    }
    serde_json::from_reader(std::fs::File::open(&configuration_file).expect("Cannot open configuration file"))
      .expect("Cannot parse configuration file.")
  };

  let db_pool = {
    let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new(configuration.database_url);
    diesel::r2d2::Pool::builder().build(manager).expect("Failed to create pool.")
  };

  configuration.name.make_ascii_lowercase();

  let (spawn_sender, spawn_receiver) = tokio::sync::mpsc::channel(100);
  let (asset_sender, asset_receiver) = tokio::sync::mpsc::channel(100);
  let mut jwt_secret = [0; 32];
  openssl::rand::rand_bytes(&mut jwt_secret).expect("Failed to generate JWT");
  let mut jwt_nonce_secret = [0; 32];
  openssl::rand::rand_bytes(&mut jwt_nonce_secret).expect("Failed to generate JWT");
  let server = std::sync::Arc::new(Server {
    asset_store: configuration.asset_store.load(),
    outstanding_assets: prometheus_locks::mutex::PrometheusMutex::new(
      "outstanding_assets",
      "outstanding asset catalogue",
      std::collections::HashMap::new(),
    )
    .expect("Failed to create Prometheus-monitored mutex"),
    push_assets: prometheus_locks::mutex::PrometheusMutex::new("push_assets", "asset pusher", asset_sender)
      .expect("Failed to create Prometheus-monitored mutex"),
    authentication: configuration.authentication.load(&configuration.name).await.unwrap(),
    jwt_decoding_key: jsonwebtoken::DecodingKey::from_secret(&jwt_secret),
    jwt_encoding_key: jsonwebtoken::EncodingKey::from_secret(&jwt_secret),
    jwt_nonce_decoding_key: jsonwebtoken::DecodingKey::from_secret(&jwt_nonce_secret),
    jwt_nonce_encoding_key: jsonwebtoken::EncodingKey::from_secret(&jwt_nonce_secret),
    name: puzzleverse_core::parse_server_name(&configuration.name).expect("Configured server name is not a valid DNS name"),
    players: prometheus_locks::rwlock::PrometheusRwLock::new("players", "convert player names to identifiers", std::collections::HashMap::new())
      .expect("Failed to create Prometheus-monitored read-write lock"),
    player_states: prometheus_locks::rwlock::PrometheusRwLock::new(
      "player_states",
      "hold player state information",
      slotmap::DenseSlotMap::with_key(),
    )
    .expect("Failed to create Prometheus-monitored read-write lock"),
    move_queue: prometheus_locks::mutex::PrometheusMutex::new("move_queue", "access the player moving queue", spawn_sender)
      .expect("Failed to create Prometheus-monitored mutex"),
    realms: prometheus_locks::mutex::PrometheusMutex::new("realm", "accessing realms", std::collections::HashMap::new())
      .expect("Failed to create Prometheus-monitored mutex"),
    realm_states: prometheus_locks::rwlock::PrometheusRwLock::new("realm_states", "hold realm state information", slotmap::DenseSlotMap::with_key())
      .expect("Failed to create Prometheus-monitored read-write lock"),
    peers: prometheus_locks::rwlock::PrometheusRwLock::new("peers", "hold peer names to identifiers", std::collections::HashMap::new())
      .expect("Failed to create Prometheus-monitored read-write lock"),
    peer_states: prometheus_locks::rwlock::PrometheusRwLock::new("peer_states", "peer server information", slotmap::DenseSlotMap::with_key())
      .expect("Failed to create Prometheus-monitored read-write lock"),
    banned_peers: prometheus_locks::rwlock::PrometheusRwLock::new("peers_banned", "banned peers", {
      use diesel::prelude::*;
      use schema::bannedpeers::dsl as bannedpeers;
      bannedpeers::bannedpeers.select(bannedpeers::server).load(&mut db_pool.get()?)?.into_iter().collect()
    })
    .expect("Failed to create Prometheus-monitored read-write lock"),
    attempting_peer_contacts: prometheus_locks::mutex::PrometheusMutex::new(
      "peer_contact",
      "attemping to hold the peer contact lock",
      std::collections::HashSet::new(),
    )
    .expect("Failed to create Prometheus-monitored mutex"),
    announcements: prometheus_locks::rwlock::PrometheusRwLock::new("announcements", "server announcements", {
      use diesel::prelude::*;
      use schema::announcement::dsl as announcement;
      let mut db_connection = db_pool.get()?;
      diesel::delete(announcement::announcement.filter(announcement::expires.lt(chrono::Utc::now())))
        .execute(&mut db_connection)
        .expect("Failed to remove old announcements");
      announcement::announcement
        .select((announcement::contents, announcement::expires, announcement::event, announcement::realm))
        .load::<(String, chrono::DateTime<chrono::Utc>, Vec<u8>, Vec<u8>)>(&mut db_connection)?
        .into_iter()
        .filter_map(|(text, expires, event, realm)| match (rmp_serde::from_slice(&event), rmp_serde::from_slice(&realm)) {
          (Ok(event), Ok(realm)) => Some(puzzleverse_core::Announcement { text, expires, event, realm }),
          _ => None,
        })
        .collect()
    })
    .expect("Failed to create Prometheus-monitored read-write lock"),
    admin_acl: database::read_acl(&db_pool, "A", (puzzleverse_core::AccessDefault::Deny, vec![puzzleverse_core::AccessControl::AllowLocal(None)])),
    access_acl: database::read_acl(&db_pool, "a", (puzzleverse_core::AccessDefault::Allow, vec![])),
    message_acl: database::read_acl(&db_pool, "m", (puzzleverse_core::AccessDefault::Allow, vec![])),
    database: database::Database::new(db_pool, &configuration.default_realm),
    move_epoch: std::sync::atomic::AtomicU64::new(0),
    id_sequence: std::sync::atomic::AtomicI32::new(0),
  });

  start_player_mover_task(&server, spawn_receiver);
  start_asset_puller_task(&server, asset_receiver);
  start_message_cleaner_task(&server);
  let addr =
    configuration.bind_address.unwrap_or(if configuration.certificate.is_none() { "0.0.0.0:80".to_string() } else { "0.0.0.0:443".to_string() });

  // Create two different forms of the server depending on whether we are using HTTP or HTTPS
  let webserver: std::pin::Pin<Box<dyn Future<Output = hyper::Result<()>>>> = match configuration.certificate {
    Some(certificate_path) => {
      let acceptor = std::sync::Arc::new(std::sync::RwLock::new(load_cert(&certificate_path)?));
      let server = server.clone();
      let (tx, rx) = std::sync::mpsc::channel();
      match notify::watcher(tx, std::time::Duration::from_secs(30)) {
        Ok(mut watcher) => {
          use notify::Watcher;
          watcher.watch(&certificate_path, notify::RecursiveMode::NonRecursive).unwrap();
          let a = acceptor.clone();
          std::thread::spawn(move || loop {
            match rx.recv() {
              Ok(event) => {
                if let Some(file) = match event {
                  notify::DebouncedEvent::Create(p) => Some(std::borrow::Cow::from(p)),
                  notify::DebouncedEvent::Write(p) => Some(std::borrow::Cow::from(p)),
                  notify::DebouncedEvent::Rescan => Some(std::borrow::Cow::from(&certificate_path)),
                  _ => None,
                } {
                  match load_cert(file) {
                    Ok(acceptor) => {
                      *a.write().unwrap() = acceptor;
                    }
                    Err(e) => eprintln!("Failed to load new SSL cert: {}", e),
                  }
                }
              }
              Err(e) => {
                eprintln!("SSL certificate loader died: {}", e);
                break;
              }
            }
          });
        }
        Err(e) => eprintln!("Failed to set up watcher on SSL cert: {}", e),
      }

      struct TokioTcpListener(tokio::net::TcpListener);
      impl futures::Stream for TokioTcpListener {
        type Item = tokio::net::TcpStream;

        fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut futures::task::Context<'_>) -> futures::task::Poll<Option<Self::Item>> {
          self.0.poll_accept(cx).map(|result| match result {
            Ok((socket, _)) => Some(socket),
            Err(e) => {
              eprintln!("Failed to accept TCP request: {}", e);
              None
            }
          })
        }
      }

      Box::pin(
        hyper::server::Server::builder(hyper::server::accept::from_stream(TokioTcpListener(tokio::net::TcpListener::bind(&addr).await?).then(
          move |socket| {
            let acceptor = acceptor.clone();
            async move { acceptor.read().unwrap().accept(socket).await }
          },
        )))
        .serve(hyper::service::make_service_fn(move |_| {
          let server = server.clone();
          async move {
            Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req: http::Request<hyper::Body>| {
              let server = server.clone();
              server.handle_http_request(req)
            }))
          }
        })),
      )
    }
    None => {
      let server = server.clone();
      let addr = addr.parse().expect("Invalid bind address");
      Box::pin(hyper::Server::bind(&addr).serve(hyper::service::make_service_fn(move |_| {
        let server = server.clone();
        async move {
          Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req: http::Request<hyper::Body>| {
            let server = server.clone();
            server.handle_http_request(req)
          }))
        }
      })))
    }
  };
  for (player, superuser) in configuration.unix_sockets {
    let server = server.clone();
    let listener = tokio::net::UnixListener::bind(format!("./puzzleverse-{}.socket", &player)).expect("Failed to create UNIX socket");
    tokio::spawn(async move {
      loop {
        match listener.accept().await {
          Ok((stream, _)) => {
            let server = server.clone();
            tokio::spawn(server.handle_client_websocket(
              PlayerClaim { exp: 0, name: player.clone() },
              tokio_tungstenite::WebSocketStream::from_raw_socket(stream.into(), tokio_tungstenite::tungstenite::protocol::Role::Server, None).await,
              superuser,
            ));
          }
          Err(e) => {
            eprintln!("Failed to connect on UNIX socket: {}", e);
          }
        }
      }
    });
  }

  match server.database.remote_direct_messages_peers() {
    Ok(peers) => {
      for peer in peers {
        let server = server.clone();
        tokio::spawn(async move { server.attempt_peer_server_connection(&peer).await });
      }
    }
    Err(diesel::result::Error::NotFound) => (),
    Err(e) => {
      eprintln!("Failed to scrape known peers: {}", e);
    }
  }
  webserver.await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}
fn start_asset_puller_task(server: &std::sync::Arc<Server>, mut asset_receiver: tokio::sync::mpsc::Receiver<(PlayerKey, String)>) {
  let s = std::sync::Arc::downgrade(server);
  tokio::spawn(async move {
    loop {
      match asset_receiver.recv().await {
        None => break,
        Some((player, asset)) => match s.upgrade() {
          None => break,
          Some(server) => {
            match server.asset_store.pull(&asset).await {
              Ok(loaded) => match server.player_states.read("asset_loaded").await.get(player) {
                Some(player_state) => {
                  player_state
                    .mutable
                    .lock()
                    .await
                    .connection
                    .send_assets(&server, loaded.children.iter().cloned().chain(std::iter::once(asset)))
                    .await
                }
                None => eprintln!("Trying to send asset {} to non-existent player", asset),
              },
              Err(puzzleverse_core::asset_store::LoadError::Unknown) => {
                if server.find_asset(&asset, AssetPullAction::PushToPlayer(player.clone())).await {
                  // Race condition; this was added while were busy. Cycle it through the queue again
                  if let Err(e) = server.push_assets.lock("asset_puller").await.send((player, asset)).await {
                    eprintln!("Failed to cycle asset request: {}", e);
                  }
                }
              }
              Err(_) => {
                eprintln!("Age is using bad asset: {}", asset);
              }
            }
          }
        },
      }
    }
  });
}
fn start_message_cleaner_task(server: &std::sync::Arc<Server>) {
  let s = std::sync::Arc::downgrade(server);
  tokio::spawn(async move {
    let mut counter: u32 = 0;
    loop {
      tokio::time::sleep(std::time::Duration::from_secs(1)).await;
      match s.upgrade() {
        Some(server) => {
          counter += 1;
          if counter > 600 {
            counter = 0;
            if let Err(e) = server.database.direct_message_clean() {
              eprintln!("Failed to delete old chats: {}", e);
            }
          }
        }
        None => break,
      }
    }
  });
}

fn start_player_mover_task(server: &std::sync::Arc<Server>, mut spawn_receiver: tokio::sync::mpsc::Receiver<RealmMove>) {
  let s = std::sync::Arc::downgrade(server);
  tokio::spawn(async move {
    loop {
      match spawn_receiver.recv().await {
        None => break,
        Some(request) => match s.upgrade() {
          None => break,
          Some(server) => server.move_player(request).await,
        },
      }
    }
  });
}

// Actual main method. The tokio::main annotation causes all compile errors in the body to be on the line with the annotation, so keep this short
#[tokio::main(worker_threads = 8)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  start().await
}
