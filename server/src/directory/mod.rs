use crate::access::AccessManagement;
use crate::asset_store;
use crate::asset_store::manager::{AssetManager, AssetRequest, RealmTemplate};
use crate::asset_store::ServerAssetStore;
use crate::database::database_location_directory::DatabaseLocationRequest;
use crate::database::{database_location_directory, Database, StaleRemoteCalendar};
use crate::directory::location_endpoint::LocationEndpoint;
use crate::directory::peer_directory::{PeerDirectoryRequest, PeerRequest};
use crate::directory::player_directory::PlayerDirectoryRequest;
use crate::join_request::JoinRequest;
use crate::peer::message::PeerLocationSearch;
use crate::player_location_update::PlayerLocationUpdate;
use chrono::Duration;
use spadina_core::asset::Asset;
use spadina_core::communication::{DirectMessageStatus, MessageBody};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::{Activity, DirectoryEntry};
use spadina_core::location::target::LocalTarget;
use spadina_core::location::DescriptorKind;
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::net::server::AssetError;
use spadina_core::player::{OnlineState, PlayerIdentifier};
use spadina_core::shared_ref::SharedRef;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::{oneshot, watch};
use tokio_tungstenite::WebSocketStream;

pub mod location_endpoint;
pub mod peer_directory;
pub mod player_directory;

#[derive(Clone)]
pub struct Directory {
  pub access_management: Arc<AccessManagement>,
  assets: AssetManager,
  peers: peer_directory::PeerDirectory,
  locations: database_location_directory::DatabaseLocationDirectory,
  players: player_directory::PlayerDirectory,
}

impl Directory {
  pub fn new(auth: Arc<AccessManagement>, asset_store: ServerAssetStore, database: Database) -> Directory {
    let (assets, rx_asset) = mpsc::channel(500);
    let (peers, rx_peer) = mpsc::channel(500);
    let (players, rx_player) = mpsc::channel(500);
    let (locations, rx_locations) = mpsc::channel(500);
    let directory = Directory { access_management: auth.clone(), assets, peers, locations, players };
    peer_directory::start(database.clone(), directory.clone(), rx_peer);
    player_directory::start(database.clone(), directory.clone(), rx_player);
    database_location_directory::start(&directory.access_management, database.clone(), directory.clone(), rx_locations);
    asset_store::manager::start(asset_store, directory.clone(), rx_asset);
    directory
  }
  pub async fn check_activity(&self, target: LocalTarget<SharedRef<str>>) -> Result<Activity, oneshot::Receiver<Activity>> {
    let (tx, rx) = oneshot::channel();
    if self.locations.send(DatabaseLocationRequest::Activity(target, tx)).await.is_err() {
      Ok(Activity::Unknown)
    } else {
      Err(rx)
    }
  }

  pub async fn check_host_activity(&self, host: SharedRef<str>) -> Result<Activity, oneshot::Receiver<Activity>> {
    let (tx, rx) = oneshot::channel();
    if self.players.send(PlayerDirectoryRequest::Activity(host, tx)).await.is_err() {
      Ok(Activity::Unknown)
    } else {
      Err(rx)
    }
  }
  pub async fn check_host_activity_on_peer(&self, server: SharedRef<str>, host: SharedRef<str>) -> Result<Activity, oneshot::Receiver<Activity>> {
    let (tx, rx) = oneshot::channel();
    if self.peers.send(PeerDirectoryRequest::Request { server, request: PeerRequest::Activity(host, tx) }).await.is_err() {
      Ok(Activity::Unknown)
    } else {
      Err(rx)
    }
  }

  pub async fn check_online(
    &self,
    requester: PlayerIdentifier<SharedRef<str>>,
    target: SharedRef<str>,
  ) -> Result<OnlineState<SharedRef<str>>, oneshot::Receiver<OnlineState<SharedRef<str>>>> {
    let (tx, rx) = oneshot::channel();
    if self.players.send(PlayerDirectoryRequest::Check(requester, target, tx)).await.is_err() {
      Ok(OnlineState::Unknown)
    } else {
      Err(rx)
    }
  }
  pub async fn check_online_on_peer(
    &self,
    requester: Arc<str>,
    server: String,
    target: String,
  ) -> Result<OnlineState<SharedRef<str>>, oneshot::Receiver<OnlineState<SharedRef<str>>>> {
    let (output, input) = oneshot::channel();
    if self
      .peers
      .send(PeerDirectoryRequest::Request {
        server: SharedRef::Single(server),
        request: PeerRequest::CheckOnline { requester, target: SharedRef::Single(target), output },
      })
      .await
      .is_err()
    {
      Ok(OnlineState::Unknown)
    } else {
      Err(input)
    }
  }
  pub async fn create_location(&self, descriptor_kind: DescriptorKind<SharedRef<str>>, join_request: JoinRequest) {
    if let Err(mpsc::error::SendError(DatabaseLocationRequest::Create(_, join_request))) =
      self.locations.send(DatabaseLocationRequest::Create(descriptor_kind, join_request)).await
    {
      let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
    }
  }
  pub async fn join_host(&self, owner: SharedRef<str>, join_request: JoinRequest) {
    if let Err(mpsc::error::SendError(PlayerDirectoryRequest::Join(_, join_request))) =
      self.players.send(PlayerDirectoryRequest::Join(owner, join_request)).await
    {
      let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
    }
  }
  pub async fn join_host_on_peer(&self, owner: String, server: String, join_request: JoinRequest) {
    if let Err(mpsc::error::SendError(PeerDirectoryRequest::Request { request: PeerRequest::Host(_, join_request), .. })) =
      self.peers.send(PeerDirectoryRequest::Request { server: SharedRef::Single(server), request: PeerRequest::Host(owner, join_request) }).await
    {
      let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
    }
  }
  pub async fn join_location(&self, target: LocalTarget<SharedRef<str>>, join_request: JoinRequest) {
    if let Err(mpsc::error::SendError(DatabaseLocationRequest::Join(_, join_request))) =
      self.locations.send(DatabaseLocationRequest::Join(target, join_request)).await
    {
      let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
    }
  }
  pub async fn join_location_on_peer(&self, target: LocalTarget<SharedRef<str>>, server: SharedRef<str>, request: JoinRequest) {
    if let Err(mpsc::error::SendError(PeerDirectoryRequest::Request { request: PeerRequest::Location { request, .. }, .. })) = self
      .peers
      .send(PeerDirectoryRequest::Request { server, request: PeerRequest::Location { descriptor: target.descriptor, player: target.owner, request } })
      .await
    {
      let _ = request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
    }
  }
  pub async fn peers(&self) -> Result<oneshot::Receiver<Vec<Arc<str>>>, ()> {
    let (output, input) = oneshot::channel();
    self.peers.send(PeerDirectoryRequest::Peers(output)).await.map_err(|_| ())?;
    Ok(input)
  }
  pub async fn pull_asset(&self, asset: Arc<str>, search_peers: bool) -> Result<oneshot::Receiver<Arc<Asset<Arc<str>, Arc<[u8]>>>>, ()> {
    let (output, input) = oneshot::channel();
    self.assets.send(AssetRequest::Pull(asset, output, search_peers)).await.map_err(|_| ())?;
    Ok(input)
  }
  pub async fn pull_realm(&self, asset: Arc<str>) -> Result<oneshot::Receiver<RealmTemplate>, ()> {
    let (output, input) = oneshot::channel();
    self.assets.send(AssetRequest::Realm(asset, output)).await.map_err(|_| ())?;
    Ok(input)
  }
  pub async fn pull_asset_remote(&self, server: SharedRef<str>, asset: SharedRef<str>) -> Result<oneshot::Receiver<Asset<String, Vec<u8>>>, ()> {
    let (output, input) = oneshot::channel();
    self.peers.send(PeerDirectoryRequest::Request { server, request: PeerRequest::Asset(asset, output) }).await.map_err(|_| ())?;
    Ok(input)
  }
  pub async fn push_asset(&self, asset: Asset<String, Vec<u8>>) -> Result<(), AssetError> {
    let (output, input) = oneshot::channel();
    self.assets.send(AssetRequest::Upload(asset, output)).await.map_err(|_| AssetError::InternalError)?;
    input.await.map_err(|_| AssetError::InternalError)?
  }
  pub async fn refresh_calendars(&self, updates: Vec<StaleRemoteCalendar>) {
    for StaleRemoteCalendar { server, player } in updates {
      let _ =
        self.peers.send(PeerDirectoryRequest::Request { server: SharedRef::Single(server), request: PeerRequest::RefreshCalendar { player } }).await;
    }
  }
  pub async fn register_host(&self, owner: Arc<str>, endpoint: LocationEndpoint) -> Result<(), ()> {
    self.players.send(PlayerDirectoryRequest::Host(owner, endpoint)).await.map_err(|_| ())
  }
  pub async fn register_peer(&self, server: Arc<str>, connection: WebSocketStream<MixedConnection>) -> Result<(), ()> {
    self
      .peers
      .try_send(PeerDirectoryRequest::Request { server: SharedRef::Shared(server), request: PeerRequest::Connect(connection) })
      .map_err(|_| ())
  }
  pub async fn register_player(&self, name: Arc<str>, connection: WebSocketStream<MixedConnection>) -> Result<(), ()> {
    self.players.send(PlayerDirectoryRequest::Connect(name, connection)).await.map_err(|_| ())
  }
  pub async fn search_on_peer(
    &self,
    server: String,
    timeout: Duration,
    query: PeerLocationSearch<String>,
  ) -> Result<watch::Receiver<Vec<DirectoryEntry<String>>>, ()> {
    let (output, input) = watch::channel(Vec::new());
    self
      .peers
      .send(PeerDirectoryRequest::Request { server: SharedRef::Single(server), request: PeerRequest::Available { query, timeout, output } })
      .await
      .map_err(|_| ())?;
    Ok(input)
  }
  pub async fn send_dm(
    &self,
    recipient: PlayerIdentifier<SharedRef<str>>,
    sender: PlayerIdentifier<SharedRef<str>>,
    body: MessageBody<String>,
  ) -> Result<watch::Receiver<DirectMessageStatus>, DirectMessageStatus> {
    if self.access_management.check_access("send_dm", &sender).await {
      let (status, rx) = watch::channel(DirectMessageStatus::Queued);
      match recipient {
        PlayerIdentifier::Local(recipient) => {
          let _ = self.players.send(PlayerDirectoryRequest::DirectMessage { recipient, sender, body, status }).await;
        }
        PlayerIdentifier::Remote { server, player } => match sender {
          PlayerIdentifier::Local(sender) => {
            let _ = self
              .peers
              .send(PeerDirectoryRequest::Request { server, request: PeerRequest::DirectMessage { recipient: player, sender, body, status } })
              .await;
          }
          PlayerIdentifier::Remote { .. } => return Err(DirectMessageStatus::InternalError),
        },
      }
      Ok(rx)
    } else {
      Err(DirectMessageStatus::Forbidden)
    }
  }
}
