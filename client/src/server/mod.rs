use crate::server::active_connection::ActiveConnection;
use crate::server::direct_message::MessageFailure;
use crate::server::exports::Export;
use crate::server::peer::Peer;
use crate::server::updates::Update;
use chrono::{DateTime, Duration, Utc};
use futures::stream::{BoxStream, SelectAll};
use futures::{FutureExt, Stream, StreamExt};
use serde::Serialize;
use spadina_core::access::{AccessControl, AccessSetting, BannedPeer, BulkLocationSelector, OnlineAccess, Privilege, SimpleAccess};
use spadina_core::asset::Asset;
use spadina_core::asset_store::{AssetStore, LoadError};
use spadina_core::avatar::Avatar;
use spadina_core::communication::{Announcement, DirectMessage, DirectMessageStatus, MessageBody};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::{Activity, DirectoryEntry, Search};
use spadina_core::location::protocol::LocationResponse;
use spadina_core::location::target::LocalTarget;
use spadina_core::net::server::auth::PublicKey;
use spadina_core::net::server::hosting::HostEvent;
use spadina_core::net::server::{AssetError, ClientRequest, ClientResponse, DirectMessageStats};
use spadina_core::player::{OnlineState, PlayerIdentifier};
use spadina_core::reference_converter::{AsReference, ForPacket};
use spadina_core::resource::Resource;
use spadina_core::tracking_map::TrackingMap;
use spadina_core::UpdateResult;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::task::Poll;
use tokio_tungstenite::tungstenite::Message;

pub mod active_connection;
pub mod cache;
pub mod direct_message;
pub mod exports;
pub mod peer;
pub mod updates;

trait EventKind: 'static + Sized + Send {
  type AccessChange: 'static
    + Update<AccessSetting<String, Privilege>>
    + Update<AccessSetting<String, OnlineAccess>>
    + Update<AccessSetting<String, SimpleAccess>>
    + Send;
  type ActivityCheck: 'static + Send;
  type Announcement: 'static + Update<Vec<Announcement<String>>> + Send;
  type AssetDownload: 'static + Send;
  type AssetUpload: 'static + Send;
  type Avatar: 'static + Update<Avatar> + Send;
  type BannedPeers: 'static + Update<HashSet<BannedPeer<String>>> + Send;
  type Bookmark: 'static + Update<HashSet<Resource<String>>> + Send;
  type CalendarLocation: 'static + Update<BTreeSet<LocalTarget<String>>> + Send;
  type LocationSearch: Clone + 'static + Send;
  type LocationVisibility: 'static + Send;
  type PlayerReset: 'static + Send;
  type PublicKey: 'static + Update<BTreeMap<String, PublicKey>> + Send;

  fn access_change(context: Self::AccessChange, result: UpdateResult) -> Option<Self>;
  fn access_location_default_updated() -> Option<Self>;
  fn access_message_updated() -> Option<Self>;
  fn access_online_updated() -> Option<Self>;
  fn activity_event(context: Self::ActivityCheck, activity: Activity) -> Option<Self>;
  fn announcement_change(context: Self::Announcement, result: UpdateResult) -> Option<Self>;
  fn announcements_updated() -> Option<Self>;
  fn asset_available(context: Self::AssetDownload, asset: Asset<String, Vec<u8>>) -> Option<Self>;
  fn asset_unavailable(context: Self::AssetDownload) -> Option<Self>;
  fn asset_uploaded(context: Self::AssetUpload, result: Result<(), AssetError>) -> Option<Self>;
  fn avatar_update(context: Self::Avatar, result: UpdateResult) -> Option<Self>;
  fn avatar_updated() -> Option<Self>;
  fn banned_peers_changed(context: Self::BannedPeers, result: UpdateResult) -> Option<Self>;
  fn banned_peers_updated() -> Option<Self>;
  fn bookmark_update(context: Self::Bookmark, success: bool) -> Option<Self>;
  fn bookmarks_updated() -> Option<Self>;
  fn calendar_updated() -> Option<Self>;
  fn calendar_location_changed(context: Self::CalendarLocation, success: bool) -> Option<Self>;
  fn calendar_location_updated() -> Option<Self>;
  fn direct_message(sender: PlayerIdentifier<String>) -> Option<Self>;
  fn direct_message_failed(player: PlayerIdentifier<String>, failure: MessageFailure, body: MessageBody<String>) -> Option<Self>;
  fn direct_message_stats_updated() -> Option<Self>;
  fn hosting(event: HostEvent<String, Vec<u8>>) -> Option<Self>;
  fn in_location(response: LocationResponse<String, Vec<u8>>) -> Option<Self>;
  fn location_change(response: LocationChangeResponse<String>) -> Option<Self>;
  fn location_search(context: Self::LocationSearch, result: Result<Vec<DirectoryEntry<String>>, Option<String>>) -> Option<Self>;
  fn location_visibility_changed(context: Self::LocationVisibility, result: UpdateResult) -> Option<Self>;
  fn peers_updated() -> Option<Self>;
  fn player_online_state_updated() -> Option<Self>;
  fn player_reset_changed(context: Self::PlayerReset, result: UpdateResult) -> Option<Self>;
  fn public_keys_changed(context: Self::PublicKey, result: UpdateResult) -> Option<Self>;
  fn public_keys_updated() -> Option<Self>;
}

enum Task<Event: EventKind> {
  Asset(Option<Asset<String, Vec<u8>>>, Event::AssetDownload),
  Download(String, Event::AssetDownload),
  Send(Message),
}

pub struct Server<Event: EventKind, Store: AssetStore> {
  access_default: cache::Cache<AccessSetting<String, Privilege>>,
  access_message: cache::Cache<AccessSetting<String, SimpleAccess>>,
  access_online: cache::Cache<AccessSetting<String, OnlineAccess>>,
  access_updates: TrackingMap<Event::AccessChange>,
  activity_updates: TrackingMap<Event::ActivityCheck>,
  announcement_updates: TrackingMap<Event::Announcement>,
  announcements: cache::Cache<Vec<Announcement<String>>>,
  asset_download: TrackingMap<Event::AssetDownload>,
  asset_store: Arc<Store>,
  asset_upload: TrackingMap<Event::AssetUpload>,
  avatar: cache::Cache<Avatar>,
  avatar_updates: TrackingMap<Event::Avatar>,
  banned_peers: cache::Cache<HashSet<BannedPeer<String>>>,
  banned_peers_updates: TrackingMap<Event::BannedPeers>,
  bookmark_updates: TrackingMap<Event::Bookmark>,
  bookmarks: cache::Cache<HashSet<Resource<String>>>,
  calendar_id: cache::Cache<Vec<u8>>,
  calendar_location_updates: TrackingMap<Event::CalendarLocation>,
  calendar_locations: cache::Cache<BTreeSet<LocalTarget<String>>>,
  connection: ActiveConnection,
  direct_message_stats: cache::Cache<DirectMessageStats<String>>,
  direct_messages: HashMap<PlayerIdentifier<String>, direct_message::DirectMessages>,
  direct_messages_outstanding: TrackingMap<direct_message::Outstanding>,
  jwt: String,
  last_login: Option<DateTime<Utc>>,
  location_searches: TrackingMap<Event::LocationSearch>,
  location_visibility_updates: TrackingMap<Event::LocationVisibility>,
  name: String,
  peers: cache::Cache<BTreeSet<Peer>>,
  player_location: HashMap<PlayerIdentifier<String>, (DateTime<Utc>, OnlineState<String>)>,
  player_location_updates: TrackingMap<PlayerIdentifier<String>>,
  player_reset: TrackingMap<Event::PlayerReset>,
  public_key_updates: TrackingMap<Event::PublicKey>,
  public_keys: cache::Cache<BTreeMap<String, PublicKey>>,
  tasks: SelectAll<BoxStream<'static, Task<Event>>>,
}
pub enum ServerEvent<T> {
  Result(T),
  Disconnected,
  Reconnected,
  BadMessage,
}

impl<Event: EventKind, Store: AssetStore + 'static> Server<Event, Store> {
  pub async fn check_activity(&mut self, player: PlayerIdentifier<&str>, check: Event::ActivityCheck) -> active_connection::SendResult<()> {
    let message = self.activity_updates.add(check, |id, _| ClientRequest::<_, &[u8]>::Activity { id, player }.into());
    self.connection.send(message).await
  }
  pub async fn next(&mut self) -> ServerEvent<Event> {
    loop {
      enum Next<Event: EventKind> {
        Task(Task<Event>),
        Event(ServerEvent<ClientResponse<String, Vec<u8>>>),
      }
      let result: Next<Event> = tokio::select! {
        Some(v) = self.tasks.next(), if !self.tasks.is_empty() => Next::Task(v),
        Some(event) = self.connection.next() => Next::Event(event),
      };
      let response = match result {
        Next::Task(Task::Asset(Some(asset), callback)) => Event::asset_available(callback, asset).map(ServerEvent::Result),
        Next::Task(Task::Asset(None, callback)) => Event::asset_unavailable(callback).map(ServerEvent::Result),
        Next::Task(Task::Download(principal, download)) => {
          let message = self.asset_download.add(download, |id, _| ClientRequest::<_, &[u8]>::AssetPull { id, principal }.into());
          if let Err(e) = self.connection.send(message).await {
            eprintln!("Failed to send: {}", e);
          }
          continue;
        }
        Next::Task(Task::Send(message)) => {
          if let Err(e) = self.connection.send(message).await {
            eprintln!("Failed to send: {}", e);
          }
          continue;
        }
        Next::Event(ServerEvent::BadMessage) => Some(ServerEvent::BadMessage),
        Next::Event(ServerEvent::Disconnected) => Some(ServerEvent::Disconnected),
        Next::Event(ServerEvent::Reconnected) => Some(ServerEvent::Reconnected),
        Next::Event(ServerEvent::Result(message)) => self.process(message),
      };
      if let Some(response) = response {
        return response;
      }
    }
  }
  fn process(&mut self, response: ClientResponse<String, Vec<u8>>) -> Option<ServerEvent<Event>> {
    match response {
      ClientResponse::AccessChange { id, result } => {
        let callback = self.access_updates.finish(id)?;
        if result != UpdateResult::Success {
          self.access_default.invalidate();
          self.access_message.invalidate();
          self.access_online.invalidate();
        }
        Event::access_change(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::Activity { id, activity } => {
        let callback = self.activity_updates.finish(id)?;
        Event::activity_event(callback, activity).map(ServerEvent::Result)
      }
      ClientResponse::Administration { .. } => todo!(),
      ClientResponse::Announcements { announcements } => {
        self.announcements.set(announcements);
        Event::announcements_updated().map(ServerEvent::Result)
      }
      ClientResponse::AnnouncementUpdate { id, result } => {
        let callback = self.announcement_updates.finish(id)?;
        if result != UpdateResult::Success {
          self.announcements.invalidate();
        }
        Event::announcement_change(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::AssetCreationSucceeded { id } => {
        let callback = self.asset_upload.finish(id)?;
        Event::asset_uploaded(callback, Ok(())).map(ServerEvent::Result)
      }
      ClientResponse::AssetCreationFailed { id, error } => {
        let callback = self.asset_upload.finish(id)?;
        Event::asset_uploaded(callback, Err(error)).map(ServerEvent::Result)
      }
      ClientResponse::Asset { id, asset } => {
        let callback = self.asset_download.finish(id)?;
        let asset_store = self.asset_store.clone();
        self.tasks.push(
          async move {
            let principal = asset.principal_hash();
            asset_store.push(&principal, &asset.reference(ForPacket)).await;
            Task::Asset(Some(asset), callback)
          }
          .into_stream()
          .boxed(),
        );
        None
      }
      ClientResponse::AssetUnavailable { id } => {
        let callback = self.asset_download.finish(id)?;
        Event::asset_unavailable(callback).map(ServerEvent::Result)
      }
      ClientResponse::AvatarCurrent { avatar } => {
        self.avatar.set(avatar);
        Event::avatar_updated().map(ServerEvent::Result)
      }
      ClientResponse::AvatarUpdate { id, result } => {
        let callback = self.avatar_updates.finish(id)?;
        if result != UpdateResult::Success {
          self.avatar.invalidate();
        }
        Event::avatar_update(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::Bookmarks { bookmarks } => {
        self.bookmarks.set(bookmarks);
        Event::bookmarks_updated().map(ServerEvent::Result)
      }
      ClientResponse::BookmarkUpdate { id, success } => {
        let callback = self.bookmark_updates.finish(id)?;
        if !success {
          self.avatar.invalidate();
        }
        Event::bookmark_update(callback, success).map(ServerEvent::Result)
      }
      ClientResponse::Disconnect => {
        self.connection = ActiveConnection::Idle;
        Some(ServerEvent::Disconnected)
      }
      ClientResponse::Calendar { id } => {
        self.calendar_id.set(id);
        Event::calendar_updated().map(ServerEvent::Result)
      }
      ClientResponse::CalendarLocations { locations } => {
        self.calendar_locations.set(locations.into_iter().collect());
        Event::calendar_location_updated().map(ServerEvent::Result)
      }
      ClientResponse::CalendarLocationChange { id, success } => {
        let callback = self.calendar_location_updates.finish(id)?;
        Event::calendar_location_changed(callback, success).map(ServerEvent::Result)
      }
      ClientResponse::CurrentAccessDirectMessage { rules, default } => {
        self.access_message.set(AccessSetting { rules, default });
        Event::access_message_updated().map(ServerEvent::Result)
      }
      ClientResponse::CurrentAccessDefault { rules, default } => {
        self.access_default.set(AccessSetting { rules, default });
        Event::access_location_default_updated().map(ServerEvent::Result)
      }
      ClientResponse::CurrentAccessOnline { rules, default } => {
        self.access_online.set(AccessSetting { rules, default });
        Event::access_online_updated().map(ServerEvent::Result)
      }
      ClientResponse::DirectMessage { player, message } => {
        self.direct_messages.entry(player.clone()).or_default().add(message);
        Event::direct_message(player).map(ServerEvent::Result)
      }
      ClientResponse::DirectMessageReceipt { id, status } => {
        match direct_message::send_finish(&mut self.direct_messages_outstanding, id, status, &mut self.direct_messages) {
          Ok(Some(player)) => Event::direct_message(player).map(ServerEvent::Result),
          Ok(None) => None,
          Err((failure, recipient, body)) => Event::direct_message_failed(recipient, failure, body).map(ServerEvent::Result),
        }
      }
      ClientResponse::DirectMessageStats { stats, last_login } => {
        self.direct_message_stats.set(stats);
        self.last_login = Some(last_login);
        Event::direct_message_stats_updated().map(ServerEvent::Result)
      }
      ClientResponse::DirectMessages { from, to, player, messages } => {
        self.direct_messages.entry(player.clone()).or_default().add_range(from, to, messages);
        Event::direct_message(player).map(ServerEvent::Result)
      }
      ClientResponse::LocationChange { location } => Event::location_change(location).map(ServerEvent::Result),
      ClientResponse::NoOperation => None,
      ClientResponse::ToHost { event } => Event::hosting(event).map(ServerEvent::Result),
      ClientResponse::Peers { peers } => {
        self.peers.set(peers.into_iter().map(|peer| peer.into()).collect());
        Event::peers_updated().map(ServerEvent::Result)
      }
      ClientResponse::PeersBanned { bans } => {
        self.banned_peers.set(bans);
        Event::banned_peers_updated().map(ServerEvent::Result)
      }
      ClientResponse::PeersBannedUpdate { id, result } => {
        let callback = self.banned_peers_updates.finish(id)?;
        if result != UpdateResult::Success {
          self.banned_peers.invalidate();
        }
        Event::banned_peers_changed(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::PlayerReset { id, result } => {
        let callback = self.player_reset.finish(id)?;
        Event::player_reset_changed(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::PublicKeys { keys } => {
        self.public_keys.set(keys);
        Event::public_keys_updated().map(ServerEvent::Result)
      }
      ClientResponse::PublicKeyUpdate { id, result } => {
        let callback = self.public_key_updates.finish(id)?;
        if result != UpdateResult::Success {
          self.public_keys.invalidate();
        }
        Event::public_keys_changed(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::LocationVisibility { id, result } => {
        let callback = self.location_visibility_updates.finish(id)?;
        Event::location_visibility_changed(callback, result).map(ServerEvent::Result)
      }
      ClientResponse::LocationsAvailable { id, locations } => {
        let callback = self.location_searches.get_mut(id)?.clone();
        Event::location_search(callback, Ok(locations)).map(ServerEvent::Result)
      }
      ClientResponse::LocationsUnavailable { id, server } => {
        let callback = self.location_searches.get_mut(id)?.clone();
        Event::location_search(callback, Err(server)).map(ServerEvent::Result)
      }
      ClientResponse::PlayerOnlineState { id, state } => {
        self.player_location.insert(self.player_location_updates.finish(id)?, (Utc::now(), state));
        Event::player_online_state_updated().map(ServerEvent::Result)
      }
      ClientResponse::InLocation { response } => Event::in_location(response).map(ServerEvent::Result),
    }
  }
  pub fn pull_asset(&mut self, principal: String, download: Event::AssetDownload) {
    let asset_store = self.asset_store.clone();
    self.tasks.push(
      async move {
        match asset_store.pull(&principal).await {
          Ok(asset) => Task::Asset(Some(asset), download),
          Err(LoadError::Unknown) => Task::Download(principal, download),
          Err(e) => {
            eprintln!("Failed to load asset {}: {}", &principal, e);
            Task::Asset(None, download)
          }
        }
      }
      .into_stream()
      .boxed(),
    )
  }
  pub fn push_asset(&mut self, asset: Asset<String, Vec<u8>>, upload: Event::AssetUpload) {
    let asset_store = self.asset_store.clone();
    let message = self.asset_upload.add(upload, |id, _| ClientRequest::AssetUpload { id, asset: asset.reference(ForPacket) }.into());
    self.tasks.push(
      async move {
        let principal = asset.principal_hash();
        asset_store.push(&principal, &asset.reference(ForPacket)).await;
        Task::Send(message)
      }
      .into_stream()
      .boxed(),
    );
  }

  pub async fn reset_player(&mut self, player: &str, reset: Event::PlayerReset) -> active_connection::SendResult<()> {
    let message = self.player_reset.add(reset, |id, _| ClientRequest::<_, &[u8]>::PlayerReset { id, player }.into());
    self.connection.send(message).await
  }
  pub async fn search_locations(
    &mut self,
    source: Search<&str>,
    timeout: Duration,
    search: Event::LocationSearch,
  ) -> active_connection::SendResult<()> {
    let message = self.location_searches.add(search, |id, _| {
      ClientRequest::<_, &[u8]>::LocationsList { id, source, timeout: timeout.num_seconds().abs().try_into().unwrap_or_default() }.into()
    });
    self.connection.send(message).await
  }
  pub async fn access_direct_message<'a, E: Export<AccessSetting<String, SimpleAccess>>>(
    &'a mut self,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.access_message.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn access_direct_message_request(&mut self, update: Event::AccessChange) -> active_connection::SendResult<()> {
    if let Some(message) = self.access_message.modify(&mut self.access_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }

  pub async fn access_location_default<'a, E: Export<AccessSetting<String, Privilege>>>(
    &'a mut self,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.access_default.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn access_location_default_request(&mut self, update: Event::AccessChange) -> active_connection::SendResult<()> {
    if let Some(message) = self.access_default.modify(&mut self.access_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }

  pub async fn access_online_status<'a, E: Export<AccessSetting<String, OnlineAccess>>>(
    &'a mut self,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.access_online.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn access_online_request(&mut self, update: Event::AccessChange) -> active_connection::SendResult<()> {
    if let Some(message) = self.access_online.modify(&mut self.access_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }

  pub async fn announcements<'a, E: Export<Vec<Announcement<String>>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.announcements.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn announcements_request(&mut self, update: Event::Announcement) -> active_connection::SendResult<()> {
    if let Some(message) = self.announcements.modify(&mut self.announcement_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub async fn avatar<'a, E: Export<Avatar>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.avatar.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn avatar_request(&mut self, update: Event::Avatar) -> active_connection::SendResult<()> {
    if let Some(message) = self.avatar.modify(&mut self.avatar_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub async fn banned_peers<'a, E: Export<HashSet<BannedPeer<String>>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.banned_peers.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn banned_peers_request(&mut self, update: Event::BannedPeers) -> active_connection::SendResult<()> {
    if let Some(message) = self.banned_peers.modify(&mut self.banned_peers_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub async fn bookmarks<'a, E: Export<HashSet<Resource<String>>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.bookmarks.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn bookmarks_request(&mut self, update: Event::Bookmark) -> active_connection::SendResult<()> {
    if let Some(message) = self.bookmarks.modify(&mut self.bookmark_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub async fn calendar<'a, E: Export<Vec<u8>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.calendar_id.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn calendar_reset(&mut self) -> active_connection::SendResult<()> {
    self.connection.send(ClientRequest::<String, &[u8]>::CalendarReset.into()).await
  }
  pub async fn calendar_locations<'a, E: Export<BTreeSet<LocalTarget<String>>>>(
    &'a mut self,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.calendar_locations.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn calendar_locations_request(&mut self, update: Event::CalendarLocation) -> active_connection::SendResult<()> {
    if let Some(message) = self.calendar_locations.modify(&mut self.calendar_location_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub fn last_login(&self) -> Option<DateTime<Utc>> {
    self.last_login.clone()
  }
  pub async fn public_keys<'a, E: Export<BTreeMap<String, PublicKey>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.public_keys.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn public_keys_request(&mut self, update: Event::PublicKey) -> active_connection::SendResult<()> {
    if let Some(message) = self.public_keys.modify(&mut self.public_key_updates, update) {
      self.connection.send(message).await?;
    }
    Ok(())
  }
  pub async fn direct_message_stats<'a, E: Export<DirectMessageStats<String>>>(
    &'a mut self,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.direct_message_stats.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn peers<'a, E: Export<BTreeSet<Peer>>>(&'a mut self, export: E) -> active_connection::SendResult<E::Output<'a>> {
    let (message, result) = self.peers.get(export);
    if let Some(message) = message {
      self.connection.send(message).await?;
    }
    Ok(result)
  }
  pub async fn set_location_visibility(
    &mut self,
    callback: Event::LocationVisibility,
    rules: Vec<AccessControl<&str, Privilege>>,
    default: Privilege,
    selection: BulkLocationSelector<&str>,
  ) -> active_connection::SendResult<()> {
    self
      .connection
      .send(
        self
          .location_visibility_updates
          .add(callback, |id, _| ClientRequest::<_, &[u8]>::AccessSetLocationBulk { id, rules, default, selection }.into()),
      )
      .await
  }
  pub async fn player_location(&mut self, player: PlayerIdentifier<String>) -> active_connection::SendResult<OnlineState<&str>> {
    let (request, result) = match self.player_location.get(&player) {
      None => (true, OnlineState::Unknown),
      Some((last, status)) => (Utc::now() - last > Duration::minutes(2), status.reference(AsReference::<str>::default())),
    };
    if request {
      self
        .connection
        .send(self.player_location_updates.add(player, |id, player| {
          ClientRequest::<_, &[u8]>::PlayerOnlineCheck { id, player: player.reference(AsReference::<str>::default()) }.into()
        }))
        .await?;
    }
    Ok(result)
  }
  pub async fn direct_message_send(&mut self, recipient: PlayerIdentifier<String>, body: MessageBody<String>) -> active_connection::SendResult<()> {
    self.connection.send(direct_message::send(&mut self.direct_messages_outstanding, recipient, body)).await
  }
  pub async fn direct_message_read<'a, E: Export<[DirectMessage<String>]>>(
    &'a mut self,
    player: PlayerIdentifier<String>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    export: E,
  ) -> active_connection::SendResult<E::Output<'a>> {
    let (messages, output) =
      self.direct_messages.entry(player.clone()).or_default().get(&player.reference(AsReference::<str>::default()), from, to, export);
    for message in messages {
      self.connection.send(message).await?;
    }
    Ok(output)
  }
}
