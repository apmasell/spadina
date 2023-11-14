pub mod active_search;
pub mod from_peer;
pub mod handshake;
pub mod message;
pub mod net;
pub mod on_peer;
pub mod outstanding_message;
pub mod reconnection_timer;

use crate::database::location_scope::{LocationListScope, LocationScope};
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use crate::directory::peer_directory::PeerRequest;
use crate::directory::Directory;
use crate::location_search;
use crate::metrics::{PeerLabel, SharedString};
use crate::peer::from_peer::PlayerFromPeer;
use crate::peer::message::{PeerLocationSearch, PeerMessage, VisitorTarget};
use crate::peer::net::PeerClaim;
use crate::peer::on_peer::PlayerOnPeer;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use crate::socket_entity::{ConnectionState, Incoming, Outgoing, SocketEntity};
use crate::stream_map::StreamsUnorderedMap;
use chrono::{Duration, Utc};
use diesel::QueryResult;
use futures::{FutureExt, Stream, StreamExt};
use outstanding_message::OutstandingMessage;
use spadina_core::asset::Asset;
use spadina_core::communication::DirectMessageStatus;
use spadina_core::location::directory::{Activity, Visibility};
use spadina_core::location::target::{LocalTarget, UnresolvedTarget};
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::player::{OnlineState, PlayerIdentifier};
use spadina_core::reference_converter::{AsArc, AsReference, AsSingle, ForPacket};
use spadina_core::shared_ref::SharedRef;
use spadina_core::tracking_map::oneshot_timeout::OneshotTimeout;
use spadina_core::tracking_map::TrackingMap;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio_stream::wrappers::WatchStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

pub struct Peer {
  activity_check: TrackingMap<OneshotTimeout<Activity>>,
  asset_requests: TrackingMap<OneshotTimeout<Asset<String, Vec<u8>>>>,
  calendar_requests: TrackingMap<String>,
  name: Arc<str>,
  online_status_response: TrackingMap<OneshotTimeout<OnlineState<SharedRef<str>>>>,
  outstanding_messages: TrackingMap<OutstandingMessage>,
  players_from_peer: StreamsUnorderedMap<BTreeMap<Arc<str>, PlayerFromPeer>>,
  players_on_peer: StreamsUnorderedMap<BTreeMap<Arc<str>, PlayerOnPeer>>,
  reconnection_timer: reconnection_timer::ReconnectionTimer,
  searches: TrackingMap<active_search::ActiveSearch>,
}

pub enum InternalEvent {
  InitiateConnection,
  Message(Message),
  RetryExceeded,
}

impl Stream for Peer {
  type Item = InternalEvent;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let peer = self.get_mut();
    if let Poll::Ready(Some(value)) = peer.players_on_peer.poll_next_unpin(cx) {
      return Poll::Ready(Some(InternalEvent::Message(value)));
    }
    if let Poll::Ready(Some(value)) = peer.players_from_peer.poll_next_unpin(cx) {
      return Poll::Ready(Some(InternalEvent::Message(value)));
    }
    peer.reconnection_timer.poll_next_unpin(cx)
  }
}

impl SocketEntity for Peer {
  const DIRECTORY_QUEUE_DEPTH: usize = 200;
  type Claim = PeerClaim<String>;
  type DirectoryRequest = PeerRequest;
  type ExternalRequest = PeerMessage<String, Vec<u8>>;

  fn establish(
    claim: Self::Claim,
    connection: WebSocketStream<MixedConnection>,
    directory: Directory,
  ) -> impl Future<Output = Result<(), ()>> + Send {
    async move { directory.register_peer(Arc::from(claim.name), connection).await }
  }

  fn new(name: Arc<str>, _database: &Database) -> QueryResult<Self> {
    Ok(Peer {
      activity_check: Default::default(),
      asset_requests: Default::default(),
      calendar_requests: Default::default(),
      name,
      online_status_response: Default::default(),
      outstanding_messages: Default::default(),
      players_from_peer: Default::default(),
      players_on_peer: Default::default(),
      reconnection_timer: Default::default(),
      searches: Default::default(),
    })
  }

  async fn process(
    &mut self,
    incoming: Incoming<Self>,
    directory: &Directory,
    database: &Database,
    connection_state: ConnectionState,
  ) -> Vec<Outgoing<Self>> {
    self.activity_check.purge_expired();
    self.asset_requests.purge_expired();
    self.online_status_response.purge_expired();
    self.outstanding_messages.purge_expired();
    self.reconnection_timer.active(connection_state == ConnectionState::Disconnected);
    self.searches.purge_expired();

    match incoming {
      Incoming::Delayed(InternalEvent::InitiateConnection) => {
        if let Err(()) = handshake::initiate(&self.name, &directory.access_management).await {
          self.reconnection_timer.back_off();
        }
        vec![]
      }
      Incoming::Delayed(InternalEvent::Message(message)) => vec![Outgoing::Send(message)],
      Incoming::Delayed(InternalEvent::RetryExceeded) => vec![Outgoing::Break],
      Incoming::Directory(request) => match request {
        PeerRequest::Activity(player, output) => {
          let message =
            self.activity_check.add(output.into(), |id, _| Outgoing::Send(PeerMessage::<_, &[u8]>::HostActivityRequest { id, player }.into()));
          vec![message]
        }
        PeerRequest::Asset(asset, output) => {
          let message = self.asset_requests.add(output.into(), |id, _| Outgoing::Send(PeerMessage::<_, &[u8]>::AssetRequest { id, asset }.into()));
          vec![message]
        }
        PeerRequest::Available { query, timeout, output } => {
          let message = self.searches.add(active_search::ActiveSearch::new(output, timeout), |id, _| {
            Outgoing::Send(PeerMessage::<_, &[u8]>::LocationsList { id, query }.into())
          });
          vec![message]
        }
        PeerRequest::CheckOnline { requester, target, output } => {
          let message = self.online_status_response.add(output.into(), |id, _| {
            Outgoing::Send(PeerMessage::<_, &[u8]>::OnlineStatusRequest { id, requester: SharedRef::Shared(requester), target }.into())
          });
          vec![message]
        }
        PeerRequest::Connect(connection) => vec![Outgoing::Connect(connection)],
        PeerRequest::DirectMessage { sender, recipient, body, status } => {
          let message = self.outstanding_messages.add(
            OutstandingMessage { sender, recipient, body, output: status, timeout: Utc::now() + Duration::minutes(2) },
            |id, outstanding| {
              Outgoing::Send(
                PeerMessage::<_, &[u8]>::DirectMessage {
                  id,
                  sender: outstanding.sender.as_ref(),
                  recipient: outstanding.recipient.as_ref(),
                  body: outstanding.body.reference(AsReference::<str>::default()),
                }
                .into(),
              )
            },
          );
          vec![message]
        }
        PeerRequest::Host(host, join_request) => {
          PlayerOnPeer::create(join_request, VisitorTarget::Host { host: host.as_ref() }, &mut self.players_on_peer)
        }
        PeerRequest::Location { player, descriptor, request } => PlayerOnPeer::create(
          request,
          VisitorTarget::Location { owner: player.as_ref(), descriptor: descriptor.reference(AsReference::<str>::default()) },
          &mut self.players_on_peer,
        ),
        PeerRequest::RefreshCalendar { player } => {
          let locations = database.calender_cache_fetch_locations_by_server(&player, &self.name);
          match locations {
            Ok(locations) => {
              let message = Outgoing::Send(self.calendar_requests.add(player, |id, player| {
                PeerMessage::<_, Vec<u8>>::CalendarRequest {
                  id,
                  player: player.as_str(),
                  locations: locations.iter().map(|l| l.reference(AsReference::default())).collect(),
                }
                .into()
              }));
              vec![message]
            }
            Err(e) => {
              eprintln!("Failed to get remote locations for {} on {}: {}", player, &self.name, e);
              vec![]
            }
          }
        }
      },
      Incoming::External(message) => match message {
        PeerMessage::AssetRequest { id, asset } => match directory.pull_asset(asset.into(), false).await {
          Ok(rx) => {
            let task = Outgoing::SideTask(
              async move {
                let message = Outgoing::Send(match rx.await {
                  Ok(asset) => PeerMessage::AssetResponseOk { id, asset: asset.reference(ForPacket) }.into(),
                  Err(_) => PeerMessage::<String, &[u8]>::AssetResponseMissing { id }.into(),
                });
                vec![message]
              }
              .into_stream()
              .boxed(),
            );
            vec![task]
          }
          Err(()) => {
            let message = Outgoing::Send(PeerMessage::<String, &[u8]>::AssetResponseMissing { id }.into());
            vec![message]
          }
        },
        PeerMessage::AssetResponseMissing { id } => {
          self.asset_requests.finish(id);
          vec![]
        }
        PeerMessage::AssetResponseOk { id, asset } => {
          if let Some(output) = self.asset_requests.finish(id) {
            let _ = output.send(asset);
          }
          vec![]
        }
        PeerMessage::AvatarSet { avatar, player } => {
          let remove = if let Some(state) = self.players_from_peer.get(player.as_str()) {
            state.send(PlayerEvent::Avatar(avatar)).await.is_err()
          } else {
            false
          };
          if remove {
            self.players_from_peer.remove(player.as_str());
            let message = Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorRelease { player, target: UnresolvedTarget::NoWhere }.into());
            vec![message]
          } else {
            vec![]
          }
        }
        PeerMessage::CalendarRequest { id, player, locations } => {
          match database.location_announcements_fetch_for_remote(
            locations,
            PlayerIdentifier::Remote { player: player.as_str(), server: &self.name },
            &directory.access_management.server_name,
          ) {
            Ok(entries) => {
              let message = Outgoing::Send(PeerMessage::<_, Vec<u8>>::CalendarResponse { id, entries }.into());
              vec![message]
            }
            Err(e) => {
              eprintln!("Failed to get calendar entries for {} on {}: {}", &player, &self.name, e);
              vec![]
            }
          }
        }
        PeerMessage::CalendarResponse { id, entries } => {
          if let Some(player) = self.calendar_requests.finish(id) {
            if let Err(e) = database.calendar_cache_update(&player, &self.name, entries) {
              eprintln!("Failed to update calendar cache from {} for {}: {}", &self.name, player, e)
            }
          }
          vec![]
        }
        PeerMessage::DirectMessage { id, sender, recipient, body } => {
          let message = if body.is_valid_at(&Utc::now())
            && directory
              .access_management
              .check_access("peer_dm", &PlayerIdentifier::Remote { server: self.name.as_ref(), player: sender.as_ref() })
              .await
          {
            match directory
              .send_dm(
                PlayerIdentifier::Local(SharedRef::Single(recipient)),
                PlayerIdentifier::Remote { server: SharedRef::Shared(self.name.clone()), player: SharedRef::Single(sender) },
                body,
              )
              .await
            {
              Err(status) => Outgoing::Send(PeerMessage::<String, Vec<u8>>::DirectMessageResponse { id, status }.into()),
              Ok(rx) => Outgoing::SideTask(
                WatchStream::new(rx)
                  .map(move |status| vec![Outgoing::Send(PeerMessage::<String, Vec<u8>>::DirectMessageResponse { id, status }.into())])
                  .boxed(),
              ),
            }
          } else {
            Outgoing::Send(PeerMessage::<String, Vec<u8>>::DirectMessageResponse { id, status: DirectMessageStatus::Forbidden }.into())
          };
          vec![message]
        }
        PeerMessage::DirectMessageResponse { id, status } => {
          if let Some(OutstandingMessage { sender, recipient, body, output, .. }) = self.outstanding_messages.finish(id) {
            if let DirectMessageStatus::Delivered(ts) = &status {
              if let Err(e) =
                database.remote_direct_message_write(PlayerReference::Name(sender.as_ref()), recipient.as_ref(), &self.name, &body, Some(*ts))
              {
                eprintln!("Failed to store direct message response: {}", e);
              }
            }
            let _ = output.send(status);
          }
          vec![]
        }
        PeerMessage::HostActivityRequest { id, player } => {
          let result = match directory.check_host_activity(SharedRef::Single(player)).await {
            Ok(activity) => Outgoing::Send(PeerMessage::<String, &[u8]>::HostActivityResponse { id, activity }.into()),
            Err(rx) => Outgoing::SideTask(
              async move {
                let message =
                  Outgoing::Send(PeerMessage::<String, &[u8]>::HostActivityResponse { id, activity: rx.await.unwrap_or(Activity::Unknown) }.into());
                vec![message]
              }
              .into_stream()
              .boxed(),
            ),
          };
          vec![result]
        }
        PeerMessage::HostActivityResponse { id, activity } => {
          if let Some(output) = self.activity_check.finish(id) {
            let _ = output.send(activity);
          }
          vec![]
        }
        PeerMessage::OnlineStatusRequest { id, requester, target } => {
          let state = if directory
            .access_management
            .check_access("peer_online_status", &PlayerIdentifier::Remote { server: self.name.as_ref(), player: requester.as_ref() })
            .await
          {
            directory
              .check_online(
                PlayerIdentifier::Remote { server: SharedRef::Shared(self.name.clone()), player: SharedRef::Single(requester) },
                SharedRef::Single(target),
              )
              .await
          } else {
            Ok(OnlineState::Unknown)
          };
          let result = match state {
            Ok(state) => Outgoing::Send(PeerMessage::<_, &[u8]>::OnlineStatusResponse { id, state }.into()),
            Err(rx) => Outgoing::SideTask(
              async move {
                let response =
                  Outgoing::Send(PeerMessage::<_, &[u8]>::OnlineStatusResponse { id, state: rx.await.unwrap_or(OnlineState::Unknown) }.into());
                vec![response]
              }
              .into_stream()
              .boxed(),
            ),
          };
          vec![result]
        }
        PeerMessage::OnlineStatusResponse { id, state } => {
          if let Some(output) = self.online_status_response.finish(id) {
            let _ = output.send(state.convert(AsSingle::<str>::default()));
          }
          vec![]
        }
        PeerMessage::LocationChange { player, response } => {
          let mut output = Vec::new();
          let remove_player = if let Some(state) = self.players_on_peer.get(player.as_str()) {
            let is_released = response.is_released();
            let is_err = state.send(PlayerLocationUpdate::ResolveUpdate(response.convert(AsArc::<str>::default()))).await.is_err();
            if is_err && !is_released {
              output.push(Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorYank { player: player.as_str() }.into()));
            }
            is_err || is_released
          } else {
            false
          };
          if remove_player {
            self.players_on_peer.remove(player.as_str());
          }
          output
        }
        PeerMessage::LocationRequest { player, request } => {
          let yank_player = if let Some(state) = self.players_from_peer.get(player.as_str()) {
            state.send(PlayerEvent::Request(request)).await.is_err()
          } else {
            true
          };
          let mut output = Vec::new();
          if yank_player {
            output.push(Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorYank { player }.into()));
          }
          output
        }
        PeerMessage::LocationResponse { player, response } => {
          let release_player = if let Some(state) = self.players_on_peer.get(player.as_str()) {
            state.send(PlayerLocationUpdate::ResponseSingle(response)).await.is_err()
          } else {
            true
          };
          let mut output = Vec::new();
          if release_player {
            output.push(Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorRelease { player, target: UnresolvedTarget::NoWhere }.into()));
          }
          output
        }
        PeerMessage::LocationsAvailable { id, locations: available } => {
          if let Some(output) = self.searches.get_mut(id) {
            output.send(available);
          }
          vec![]
        }
        PeerMessage::LocationsUnavailable { id } => {
          self.searches.finish(id);
          vec![]
        }
        PeerMessage::LocationsList { id, query } => location_search::local_query(
          active_search::SearchRequest(id),
          match query {
            PeerLocationSearch::Public => LocationListScope::Visibility(vec![Visibility::Public]),
            PeerLocationSearch::Search { query } => {
              LocationListScope::And(vec![LocationListScope::Visibility(vec![Visibility::Public]), query.into()])
            }
            PeerLocationSearch::Specific { locations } => LocationListScope::And(vec![
              LocationListScope::Visibility(vec![Visibility::Public]),
              LocationListScope::Or(
                locations
                  .into_iter()
                  .map(|LocalTarget { owner, descriptor }| {
                    LocationListScope::Exact(LocationScope { owner: PlayerReference::Name(owner), descriptor })
                  })
                  .collect(),
              ),
            ]),
          },
          database,
          directory,
        ),

        PeerMessage::VisitorRelease { player, target } => {
          if let Some(handle) = self.players_on_peer.remove(player.as_str()) {
            if handle.send(PlayerLocationUpdate::Move(target.convert(AsSingle::<str>::default()))).await.is_err() {
              eprintln!("Release issued for {}, but failed to tell player", player);
            }
          }
          vec![]
        }
        PeerMessage::VisitorSend { player, target, avatar } => {
          let release_message =
            Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorRelease { player: player.as_str(), target: UnresolvedTarget::NoWhere }.into());
          let mut output = Vec::new();
          if directory
            .access_management
            .check_access("visitor_send", &PlayerIdentifier::Remote { player: player.as_str(), server: self.name.as_ref() })
            .await
          {
            let join_request = PlayerFromPeer::create(Arc::from(player), self.name.clone(), avatar, &mut self.players_from_peer);
            match target {
              VisitorTarget::Location { descriptor, owner } => {
                directory
                  .join_location(
                    LocalTarget { owner: SharedRef::Single(owner), descriptor: descriptor.convert(AsSingle::<str>::default()) },
                    join_request,
                  )
                  .await
              }
              VisitorTarget::Host { host } => directory.join_host(SharedRef::Single(host), join_request).await,
            }
          } else {
            output.push(release_message);
          }

          output
        }
        PeerMessage::VisitorYank { player } => {
          self.players_from_peer.remove(player.as_str());
          vec![]
        }
      },
      Incoming::StateChange => vec![],
    }
  }

  fn show_decode_error(&self, error: rmp_serde::decode::Error) {
    eprintln!("Decode error for {}: {}", &self.name, error);
    crate::metrics::BAD_PEER_REQUESTS.get_or_create(&PeerLabel { peer: SharedString(self.name.clone()) }).inc();
  }

  fn show_socket_error(&self, error: tokio_tungstenite::tungstenite::Error) {
    eprintln!("Socket error for {}: {}", &self.name, error);
    crate::metrics::BAD_PEER_REQUESTS.get_or_create(&PeerLabel { peer: SharedString(self.name.clone()) }).inc();
  }
}
