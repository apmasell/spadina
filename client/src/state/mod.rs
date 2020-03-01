use std::path::PathBuf;

use futures::StreamExt;
pub mod cache;
mod connection;
pub mod emote_cache;
pub mod jitter;
pub mod location;
pub mod mutator;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthServer(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DirectMessage {
  Incoming(spadina_core::communication::MessageBody<String>, chrono::DateTime<chrono::Utc>),
  Outgoing(spadina_core::communication::MessageBody<String>, chrono::DateTime<chrono::Utc>),
  InTransit(spadina_core::communication::MessageBody<String>, chrono::DateTime<chrono::Utc>),
  Pending(i32, spadina_core::communication::MessageBody<String>),
}
#[derive(Debug, Clone, Default)]
pub struct DirectMessageStats {
  pub stats: std::collections::HashMap<spadina_core::player::PlayerIdentifier<String>, spadina_core::communication::DirectMessageInfo>,
  pub last_login: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EmoteKind {
  Undirected,
  Directed(spadina_core::realm::Direction),
  Consensual(spadina_core::player::PlayerIdentifier<String>),
}
pub enum PlayerRequest {
  Follow(PlayerRequestFollow),
  Emote(PlayerRequestEmote),
}
pub struct PlayerRequestFollow {
  id: i32,
  player: spadina_core::player::PlayerIdentifier<String>,
}
pub struct PlayerRequestEmote {
  id: i32,
  player: spadina_core::player::PlayerIdentifier<String>,
  asset: spadina_core::asset::Asset,
}
pub struct PlayerResponse {
  id: i32,
  kind: PlayerResponseKind,
  agree: bool,
}
#[derive(Copy, Clone)]
enum PlayerResponseKind {
  Follow,
  Emote,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FollowRequest<'a> {
  id: i32,
  target: &'a spadina_core::player::PlayerIdentifier<String>,
}
type Shared<T> = std::sync::Arc<std::sync::Mutex<T>>;
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct RemoteServer(pub String);
#[derive(Clone, Default)]
struct CacheStates {
  access: cache::KeyCache<
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  >,
  account_locks: cache::KeyCache<spadina_core::ClientRequest<String>, spadina_core::access::AccountLockState>,
  announcements: cache::Cache<Vec<spadina_core::communication::Announcement<String>>>,
  asset_upload: mutator::Mutator<mutator::AssetUploadRequest>,
  avatar: cache::Cache<spadina_core::avatar::Avatar>,
  banned_servers: cache::Cache<std::collections::HashSet<spadina_core::access::BannedPeer<String>>>,
  bookmarks: cache::Cache<std::collections::HashSet<spadina_core::communication::Bookmark<String>>>,
  calendar_id: cache::Cache<Vec<u8>>,
  calendar_realms: cache::Cache<Vec<spadina_core::realm::RealmDirectoryEntry<String>>>,
  direct_message_stats: cache::Cache<DirectMessageStats>,
  direct_messages: cache::KeyCache<spadina_core::ClientRequest<String>, Vec<DirectMessage>>,
  failed_operations: Shared<Vec<FailedOperation>>,
  id: std::sync::Arc<std::sync::atomic::AtomicI32>,
  invitations: mutator::Mutator<()>,
  known_realms: cache::KeyCache<spadina_core::ClientRequest<String>, Vec<spadina_core::realm::RealmDirectoryEntry<String>>>,
  known_servers: cache::Cache<std::collections::BTreeSet<RemoteServer>>,
  location_access:
    cache::Cache<(Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess)>,
  outstanding: Shared<std::collections::HashMap<i32, InflightRequest>>,
  player_location: cache::KeyCache<spadina_core::ClientRequest<String>, spadina_core::player::PlayerLocationState<String>>,
  public_keys: cache::Cache<std::collections::HashSet<spadina_core::auth::PublicKey<String>>>,
  realm_deletion: mutator::Mutator<String>,
}
pub struct ServerConnection<S> {
  cache_state: CacheStates,
  outbound_tx: tokio::sync::mpsc::UnboundedSender<connection::ServerRequest>,
  screen: Shared<ScreenState<S, std::borrow::Cow<'static, str>>>,
  server: std::sync::Arc<std::sync::Mutex<String>>,
}

pub enum Target {
  Move(spadina_core::realm::Point),
  Interact(spadina_core::realm::Point, spadina_core::realm::InteractionKey<String>, spadina_core::realm::InteractionType<String>),
}

pub struct ScreenRef<'a, S> {
  client: &'a ServerConnection<S>,
  screen: std::sync::MutexGuard<'a, ScreenState<S, std::borrow::Cow<'static, str>>>,
}

pub enum ScreenState<S, M: AsRef<str>> {
  Quit,
  Error(M),
  Busy(M),
  InTransit,
  Loading { message: M, assets: std::sync::Arc<std::sync::atomic::AtomicUsize> },
  Lost(Option<M>),
  Login,
  LoginPassword,
  InWorld(S),
}

impl<S: location::LocationState> ServerConnection<S> {
  pub fn new<A: spadina_core::asset_store::AsyncAssetStore + 'static>(asset_store: A, runtime: &tokio::runtime::Runtime) -> Self {
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::unbounded_channel();
    let screen = std::sync::Arc::new(std::sync::Mutex::new(ScreenState::Login));
    let cache_state = CacheStates::default();
    let server_name: std::sync::Arc<std::sync::Mutex<String>> = Default::default();
    let result = ServerConnection { cache_state: cache_state.clone(), screen: screen.clone(), outbound_tx, server: server_name.clone() };
    runtime.spawn(async move {
      #[derive(Clone)]
      enum AssetDispatch {
        ConsensualEmote(i32, spadina_core::player::PlayerIdentifier<String>),
        RealmAsset,
      }
      let mut state = connection::ConnectionState::Idle;
      let mut current_location = location::Location::NoWhere;
      let mut asset_dispatch: std::collections::BTreeMap<String, Vec<AssetDispatch>> = Default::default();
      let mut emote_cache = emote_cache::EmoteCache::new();
      let mut static_events = None;
      loop {
        enum Event<S> {
          Location(Vec<location::LocationEvent<S>>),
          Server(spadina_core::ClientResponse<String>),
          ServerError(String),
          ServerQuit,
          Ignore,
          UserInterface(Option<connection::ServerRequest>),
        }
        impl<S> From<Option<Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>>> for Event<S> {
          fn from(value: Option<Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>>) -> Self {
            match value {
              None => Event::ServerQuit,
              Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(response))) => match rmp_serde::from_slice(&response) {
                Ok(response) => Event::Server(response),
                Err(e) => Event::ServerError(e.to_string()),
              },
              Some(Ok(tokio_tungstenite::tungstenite::Message::Text(response))) => match serde_json::from_str(&response) {
                Ok(response) => Event::Server(response),
                Err(e) => Event::ServerError(e.to_string()),
              },
              Some(Ok(_)) => Event::Ignore,
              Some(Err(e)) => Event::ServerError(e.to_string()),
            }
          }
        }
        let event = match static_events {
          Some(e) => Event::Location(e),
          None => tokio::select! {
            output = outbound_rx.recv() => Event::UserInterface(output),
            response = state.next() => response.map(|r| Event::Server(r)).unwrap_or(Event::ServerQuit),
            location_request = current_location.next() => location_request.map(|e| Event::Location(e)).unwrap_or(Event::Ignore)
          },
        };
        static_events = None;
        match event {
          Event::Ignore => (),
          Event::Location(requests) => {
            for request in requests {
              match request {
                location::LocationEvent::UpdateScreen(s) => {
                  *screen.lock().unwrap() = s;
                }
                location::LocationEvent::PullAsset(principal) => match asset_store.pull(&principal).await {
                  Ok(asset) => {
                    static_events = current_location.asset_available(&principal, &asset);
                  }
                  Err(spadina_core::asset_store::LoadError::Unknown) => {
                    asset_dispatch.entry(principal.clone()).or_default().push(AssetDispatch::RealmAsset);
                    state.deliver(spadina_core::ClientRequest::AssetPull { principal }).await;
                  }
                  Err(e) => {
                    state.deliver(spadina_core::ClientRequest::LocationChange { location: spadina_core::location::LocationRequest::NoWhere }).await;
                    current_location = location::Location::NoWhere;
                    *screen.lock().unwrap() =
                      ScreenState::Error(std::borrow::Cow::Owned(format!("Cannot load required asset “{}”: {}", &principal, e)));
                  }
                },
                location::LocationEvent::Leave => {
                  state.deliver(spadina_core::ClientRequest::LocationChange { location: spadina_core::location::LocationRequest::NoWhere }).await;
                }
              }
            }
          }
          Event::UserInterface(None) => break,
          Event::UserInterface(Some(output)) => {
            let new_screen = match state.process(output, &server_name).await {
              Ok(true) => ScreenState::LoginPassword,
              Ok(false) => ScreenState::InTransit,
              Err(message) => ScreenState::Error(message),
            };
            *screen.lock().unwrap() = new_screen;
          }
          Event::ServerError(e) => {
            *screen.lock().unwrap() = ScreenState::Error(std::borrow::Cow::Owned(e));
          }
          Event::ServerQuit => {
            *screen.lock().unwrap() = ScreenState::Quit;
          }
          Event::Server(response) => match response {
            spadina_core::ClientResponse::AccountLockChange { id, name, result } => {
              if let Some(operation) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created: operation.created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation: operation.operation,
                  });
                  cache_state.account_locks.update(name, |s| *s = spadina_core::access::AccountLockState::Unknown);
                }
              }
            }
            spadina_core::ClientResponse::AccountLockStatus { name, status } => {
              cache_state.account_locks.update(name, |s| *s = status);
            }
            spadina_core::ClientResponse::AccessChange { id, response } => {
              if let Some(operation) = cache_state.outstanding.lock().unwrap().remove(&id) {
                let failure = match response {
                  spadina_core::UpdateResult::Success => None,
                  spadina_core::UpdateResult::NotAllowed => Some(FailedOperation {
                    created: operation.created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed("Permission denied"),
                    operation: operation.operation,
                  }),
                  spadina_core::UpdateResult::InternalError => Some(FailedOperation {
                    created: operation.created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed("Unknown internal error"),
                    operation: operation.operation,
                  }),
                };
                if let Some(failure) = failure {
                  cache_state.failed_operations.lock().unwrap().push(failure);
                }
              }
            }
            spadina_core::ClientResponse::Announcements { announcements } => cache_state.announcements.update(|a| {
              a.clear();
              a.extend(announcements);
            }),
            spadina_core::ClientResponse::AnnouncementUpdate { id, result } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::AssetCreationSucceeded { id, hash } => cache_state.asset_upload.finish(mutator::AssetUpload(id), Ok(hash)),
            spadina_core::ClientResponse::AssetCreationFailed { id, error } => cache_state.asset_upload.finish(mutator::AssetUpload(id), Err(error)),
            spadina_core::ClientResponse::Asset { principal, asset } => {
              asset_store.push(&principal, &asset).await;
              for dispatch in asset_dispatch.remove(&principal).into_iter().flatten() {
                match dispatch {
                  AssetDispatch::ConsensualEmote(id, player) => match emote_cache.get(&principal, &asset_store).await {
                    emote_cache::EmoteResult::Emote(emote) => current_location.request_consent(id, player, emote),
                    _ => {
                      state.deliver(spadina_core::ClientRequest::ConsensualEmoteResponse { id, ok: false }).await;
                    }
                  },
                  AssetDispatch::RealmAsset => {
                    static_events = current_location.asset_available(&principal, &asset);
                  }
                }
              }
            }
            spadina_core::ClientResponse::AssetUnavailable { principal } => {
              for dispatch in asset_dispatch.remove(&principal).into_iter().flatten() {
                match dispatch {
                  AssetDispatch::ConsensualEmote(id, _) => {
                    state.deliver(spadina_core::ClientRequest::ConsensualEmoteResponse { id, ok: false }).await;
                  }
                  AssetDispatch::RealmAsset => {
                    state.deliver(spadina_core::ClientRequest::LocationChange { location: spadina_core::location::LocationRequest::NoWhere }).await;
                    current_location = location::Location::NoWhere;

                    *screen.lock().unwrap() = ScreenState::Error(std::borrow::Cow::Owned(format!("Asset “{}” is not available", &principal)));
                  }
                }
              }
            }
            spadina_core::ClientResponse::AvatarCurrent { avatar } => {
              cache_state.avatar.update(|a| *a = avatar);
            }
            spadina_core::ClientResponse::AvatarUpdate { id, success } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if !success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed("Failed to update avatar"),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::Bookmarks { bookmarks } => cache_state.bookmarks.update(|b| {
              *b = bookmarks;
            }),
            spadina_core::ClientResponse::BookmarkUpdate { id, success } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if !success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed("Failed to update bookmark"),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::Calendar { id } => cache_state.calendar_id.update(|i| {
              *i = id;
            }),
            spadina_core::ClientResponse::CalendarUpdate { id, result } => {
              if let Some(InflightRequest { operation: InflightOperation::CalendarReset, created, .. }) =
                cache_state.outstanding.lock().unwrap().remove(&id)
              {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation: InflightOperation::RealmCalendarChange,
                  })
                }
              }
            }
            spadina_core::ClientResponse::CalendarRealmChange { id, success } => {
              if let Some(InflightRequest { operation: InflightOperation::RealmCalendarChange, created, .. }) =
                cache_state.outstanding.lock().unwrap().remove(&id)
              {
                if !success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed("Failed to add realm to calendar subscription"),
                    operation: InflightOperation::RealmCalendarChange,
                  })
                }
              }
            }
            spadina_core::ClientResponse::CalendarRealmList { realms } => cache_state.calendar_realms.update(|r| {
              *r = realms;
            }),
            spadina_core::ClientResponse::Disconnect => {
              *screen.lock().unwrap() = ScreenState::Quit;
              state = connection::ConnectionState::Idle;
            }
            spadina_core::ClientResponse::ConsensualEmoteRequest { id, emote, player } => match emote_cache.get(&emote, &asset_store).await {
              emote_cache::EmoteResult::Emote(emote) => current_location.request_emote(id, emote, player),
              emote_cache::EmoteResult::Missing => {
                state.deliver(spadina_core::ClientRequest::AssetPull { principal: emote.clone() }).await;
                asset_dispatch.entry(emote).or_default().push(AssetDispatch::ConsensualEmote(id, player));
              }
              emote_cache::EmoteResult::Bad => {
                state.deliver(spadina_core::ClientRequest::ConsensualEmoteResponse { id, ok: false }).await;
              }
            },
            spadina_core::ClientResponse::FollowRequest { id, player } => {
              current_location.request_follow(id, player);
            }
            spadina_core::ClientResponse::LocationMessagePosted { sender, body, timestamp } => {
              current_location.message_posted(sender, body, timestamp);
            }
            spadina_core::ClientResponse::LocationMessages { messages, from, to } => {
              current_location.messages(messages, from, to);
            }

            spadina_core::ClientResponse::CurrentAccess { target, rules: acls, default } => {
              cache_state.access.update(target, |control| *control = (acls, default));
            }
            spadina_core::ClientResponse::DirectMessageReceipt { id, status } => {
              if let Some(InflightRequest { operation: InflightOperation::DirectMessageSend(player), created, .. }) =
                cache_state.outstanding.lock().unwrap().remove(&id)
              {
                cache_state.direct_messages.update(player.clone(), |messages| {
                  if let Some(index) = messages.iter().position(|m| match m {
                    DirectMessage::Pending(i, _) => *i == id,
                    _ => false,
                  }) {
                    if let DirectMessage::Pending(_, body) = messages.remove(index) {
                      if let Some(reason) = match status {
                        spadina_core::communication::DirectMessageStatus::Delivered(timestamp) => {
                          messages.push(DirectMessage::Outgoing(body, timestamp));
                          None
                        }
                        spadina_core::communication::DirectMessageStatus::UnknownRecipient => Some("Invalid recipient"),
                        spadina_core::communication::DirectMessageStatus::Queued(timestamp) => {
                          messages.push(DirectMessage::InTransit(body, timestamp));
                          None
                        }
                        spadina_core::communication::DirectMessageStatus::Forbidden => Some("Now allowed to contact this player"),
                        spadina_core::communication::DirectMessageStatus::InternalError => Some("Internal error"),
                      } {
                        cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                          created,
                          failed: chrono::Utc::now(),
                          reason: std::borrow::Cow::Borrowed(reason),
                          operation: InflightOperation::DirectMessageSend(player),
                        })
                      }
                    }
                  }
                })
              }
            }
            spadina_core::ClientResponse::DirectMessageStats { stats, last_login } => cache_state.direct_message_stats.update(|c| {
              c.stats = stats;
              c.last_login = last_login;
            }),
            spadina_core::ClientResponse::DirectMessages { player, messages } => cache_state.direct_messages.update(player, |m| {
              use itertools::Itertools;
              if let itertools::MinMaxResult::MinMax(min, max) = messages.iter().map(|m| m.timestamp).minmax() {
                // We assume the server is giving us everything authoritative in this range
                m.retain(|msg| match msg {
                  DirectMessage::Incoming(_, ts) | DirectMessage::Outgoing(_, ts) => *ts < min || *ts > max,
                  DirectMessage::Pending(_, _) => true,
                  DirectMessage::InTransit(_, ts) => *ts > max,
                });
              }
              m.extend(messages.into_iter().map(|m| {
                if m.inbound {
                  DirectMessage::Incoming(m.body, m.timestamp)
                } else {
                  DirectMessage::Outgoing(m.body, m.timestamp)
                }
              }));
              m.sort_by(|m1, m2| match (m1, m2) {
                (
                  DirectMessage::Incoming(_, time1) | DirectMessage::Outgoing(_, time1) | DirectMessage::InTransit(_, time1),
                  DirectMessage::Incoming(_, time2) | DirectMessage::Outgoing(_, time2) | DirectMessage::InTransit(_, time2),
                ) => time1.cmp(&time2),
                (DirectMessage::Pending(_, _), DirectMessage::Incoming(_, _) | DirectMessage::Outgoing(_, _) | DirectMessage::InTransit(_, _)) => {
                  std::cmp::Ordering::Less
                }
                (DirectMessage::Incoming(_, _) | DirectMessage::Outgoing(_, _) | DirectMessage::InTransit(_, _), DirectMessage::Pending(_, _)) => {
                  std::cmp::Ordering::Greater
                }
                (DirectMessage::Pending(id1, _), DirectMessage::Pending(id2, _)) => id1.cmp(&id2),
              });
            }),
            spadina_core::ClientResponse::InviteSuccess { id, url } => cache_state.invitations.finish(mutator::Invitation(id), Ok(url)),
            spadina_core::ClientResponse::InviteFailure { id, error } => cache_state.invitations.finish(mutator::Invitation(id), Err(error)),
            spadina_core::ClientResponse::PublicKeys { keys } => cache_state.public_keys.update(|k| {
              k.clear();
              k.extend(keys);
            }),
            spadina_core::ClientResponse::PublicKeyUpdate { id, result } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::LocationChange { location } => {
              asset_dispatch.clear();
              *screen.lock().unwrap() = ScreenState::InTransit;
              let (new_location, new_screen_state) = match location {
                spadina_core::location::LocationResponse::Resolving => (location::Location::NoWhere, ScreenState::InTransit),
                spadina_core::location::LocationResponse::Realm { owner, server, name, asset, in_directory, seed, settings } => {
                  let message = std::borrow::Cow::Owned(format!("Loading realm “{}”...", &name));
                  static_events = Some(vec![location::LocationEvent::PullAsset(asset.clone())]);
                  let (loc, assets) = location::Location::load_realm(owner, server, name, asset, in_directory, seed, settings);
                  (loc, ScreenState::Loading { message, assets })
                }
                spadina_core::location::LocationResponse::Hosting => {
                  let (location, world) = location::Location::new_host();
                  (location, ScreenState::InWorld(world))
                }
                spadina_core::location::LocationResponse::Guest { host } => {
                  let (location, world) = location::Location::new_guest(host);
                  (location, ScreenState::InWorld(world))
                }
                spadina_core::location::LocationResponse::NoWhere => (location::Location::NoWhere, ScreenState::Lost(None)),
                spadina_core::location::LocationResponse::InternalError => (
                  location::Location::NoWhere,
                  ScreenState::Lost(Some(std::borrow::Cow::Borrowed("An internal error occurred trying to access this realm."))),
                ),
                spadina_core::location::LocationResponse::PermissionDenied => {
                  (location::Location::NoWhere, ScreenState::Lost(Some(std::borrow::Cow::Borrowed("Not allowed to go to this realm."))))
                }
                spadina_core::location::LocationResponse::ResolutionFailed => {
                  (location::Location::NoWhere, ScreenState::Lost(Some(std::borrow::Cow::Borrowed("Could not find realm."))))
                }
                spadina_core::location::LocationResponse::MissingCapabilities { capabilities } => (
                  location::Location::NoWhere,
                  ScreenState::Lost(Some(std::borrow::Cow::Owned(format!(
                    "This realm requires capabilities this client does not have: {}",
                    capabilities.iter().map(|c| c.as_str()).intersperse(", ").collect::<String>()
                  )))),
                ),
              };
              current_location = new_location;
              *screen.lock().unwrap() = new_screen_state;
            }
            spadina_core::ClientResponse::RealmDelete { id, result } => cache_state.realm_deletion.finish(
              mutator::RealmDeletion(id),
              match result {
                spadina_core::UpdateResult::Success => Ok(()),
                spadina_core::UpdateResult::NotAllowed => Err(false),
                spadina_core::UpdateResult::InternalError => Err(true),
              },
            ),
            spadina_core::ClientResponse::RealmsAvailable { display, realms } => cache_state.known_realms.update(display, |v| {
              v.clear();
              v.extend(realms)
            }),
            spadina_core::ClientResponse::Peers { mut peers } => {
              peers.sort();
              cache_state.known_servers.update(|known_servers| {
                known_servers.clear();
                known_servers.extend(peers.into_iter().map(RemoteServer));
              });
            }
            spadina_core::ClientResponse::PeersBanned { bans } => {
              cache_state.banned_servers.update(|known_bans| {
                known_bans.clear();
                known_bans.extend(bans);
              });
            }
            spadina_core::ClientResponse::PeersBannedUpdate { id, result } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::PlayerState { player, state } => {
              cache_state.player_location.update(player, |s| *s = state);
            }
            spadina_core::ClientResponse::InRealm { response } => current_location.handle_realm(response),
            spadina_core::ClientResponse::CurrentAccessLocation { rules, default } => {
              cache_state.location_access.update(|access| *access = (rules, default))
            }
            spadina_core::ClientResponse::LocationAvatars { players } => current_location.update_avatars(players),
            spadina_core::ClientResponse::NoOperation => (),
            spadina_core::ClientResponse::ToHost { event } => current_location.handle_host_event(event),
            spadina_core::ClientResponse::FromHost { response } => current_location.handle_host(response),
            spadina_core::ClientResponse::PlayerReset { id, result } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::UpdateResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Borrowed(result.description()),
                    operation,
                  })
                }
              }
            }
            spadina_core::ClientResponse::TrainAdd { id, result } => {
              if let Some(InflightRequest { created, operation, .. }) = cache_state.outstanding.lock().unwrap().remove(&id) {
                if result != spadina_core::TrainAddResult::Success {
                  cache_state.failed_operations.lock().unwrap().push(FailedOperation {
                    created,
                    failed: chrono::Utc::now(),
                    reason: std::borrow::Cow::Owned(result.to_string()),
                    operation,
                  })
                }
              }
            }
          },
        }
      }
    });
    result
  }

  pub fn access(
    &self,
  ) -> cache::KeyCacheRef<
    '_,
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
    S,
  > {
    self.cache_state.access.capture(self)
  }
  pub fn account_locks(&self) -> cache::KeyCacheRef<'_, spadina_core::ClientRequest<String>, spadina_core::access::AccountLockState, S> {
    self.cache_state.account_locks.capture(self)
  }
  pub fn announcements(&self) -> cache::CacheRef<'_, Vec<spadina_core::communication::Announcement<String>>, S> {
    self.cache_state.announcements.capture(self)
  }
  pub fn assets_upload(&self) -> mutator::MutatorRef<'_, mutator::AssetUploadRequest, S> {
    self.cache_state.asset_upload.capture(self)
  }
  pub fn avatar(&self) -> cache::CacheRef<'_, spadina_core::avatar::Avatar, S> {
    self.cache_state.avatar.capture(self)
  }
  pub fn banned_servers(&self) -> cache::CacheRef<'_, std::collections::HashSet<spadina_core::access::BannedPeer<String>>, S> {
    self.cache_state.banned_servers.capture(self)
  }
  pub fn bookmarks(&self) -> cache::CacheRef<'_, std::collections::HashSet<spadina_core::communication::Bookmark<String>>, S> {
    self.cache_state.bookmarks.capture(self)
  }
  pub fn direct_messages(&self) -> cache::KeyCacheRef<'_, spadina_core::ClientRequest<String>, Vec<DirectMessage>, S> {
    self.cache_state.direct_messages.capture(self)
  }
  pub fn direct_message_stats(&self) -> cache::CacheRef<'_, DirectMessageStats, S> {
    self.cache_state.direct_message_stats.capture(self)
  }
  pub fn invitations(&self) -> mutator::MutatorRef<'_, (), S> {
    self.cache_state.invitations.capture(self)
  }
  pub fn known_realms(
    &self,
  ) -> cache::KeyCacheRef<'_, spadina_core::ClientRequest<String>, Vec<spadina_core::realm::RealmDirectoryEntry<String>>, S> {
    self.cache_state.known_realms.capture(self)
  }
  pub fn known_servers(&self) -> cache::CacheRef<'_, std::collections::BTreeSet<RemoteServer>, S> {
    self.cache_state.known_servers.capture(self)
  }
  pub fn player_location(&self) -> cache::KeyCacheRef<'_, spadina_core::ClientRequest<String>, spadina_core::player::PlayerLocationState<String>, S> {
    self.cache_state.player_location.capture(self)
  }
  pub fn public_keys(&self) -> cache::CacheRef<'_, std::collections::HashSet<spadina_core::auth::PublicKey<String>>, S> {
    self.cache_state.public_keys.capture(self)
  }
  pub fn realm_deletion(&self) -> mutator::MutatorRef<'_, String, S> {
    self.cache_state.realm_deletion.capture(self)
  }
  pub fn screen<'a>(&'a self) -> ScreenRef<'a, S> {
    ScreenRef { client: self, screen: self.screen.lock().unwrap() }
  }
  pub fn login(&self, insecure: bool, player: String, server: String, auth: connection::Auth) {
    self
      .outbound_tx
      .send(match auth {
        connection::Auth::Auto => connection::ServerRequest::TryLogin { insecure, player, server, key: None },
        connection::Auth::PublicKey(key) => connection::ServerRequest::TryLogin { insecure, player, server, key: Some(key) },
        connection::Auth::Password(password) => connection::ServerRequest::LoginPassword { insecure, player, password, server },
      })
      .unwrap();
  }
  pub fn login_socket(&self, socket: impl Into<PathBuf>, player: String, is_superuser: bool) {
    self.outbound_tx.send(connection::ServerRequest::LoginSocket { path: socket.into(), player, is_superuser }).unwrap();
  }
}

impl CacheStates {
  pub(crate) fn add_operation(&self, operation: InflightOperation) -> i32 {
    let id = self.id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    self.outstanding.lock().unwrap().insert(id, InflightRequest { created: chrono::Utc::now(), operation });
    id
  }
}
impl PlayerRequest {
  pub fn accept(&self) -> PlayerResponse {
    match self {
      PlayerRequest::Follow(f) => f.accept(),
      PlayerRequest::Emote(e) => e.accept(),
    }
  }
  pub fn reject(&self) -> PlayerResponse {
    match self {
      PlayerRequest::Follow(f) => f.reject(),
      PlayerRequest::Emote(e) => e.reject(),
    }
  }
}
impl PlayerRequestFollow {
  pub fn accept(&self) -> PlayerResponse {
    PlayerResponse { id: self.id, kind: PlayerResponseKind::Follow, agree: true }
  }
  pub fn reject(&self) -> PlayerResponse {
    PlayerResponse { id: self.id, kind: PlayerResponseKind::Follow, agree: true }
  }
  pub fn player(&self) -> &spadina_core::player::PlayerIdentifier<String> {
    &self.player
  }
}
impl PlayerRequestEmote {
  pub fn accept(&self) -> PlayerResponse {
    PlayerResponse { id: self.id, kind: PlayerResponseKind::Emote, agree: true }
  }
  pub fn reject(&self) -> PlayerResponse {
    PlayerResponse { id: self.id, kind: PlayerResponseKind::Emote, agree: true }
  }
  pub fn player(&self) -> &spadina_core::player::PlayerIdentifier<String> {
    &self.player
  }
  pub fn emote_asset(&self) -> &spadina_core::asset::Asset {
    &self.asset
  }
}

pub enum SettingError {
  BadName,
  BadType,
}
#[derive(Clone)]
pub enum InflightOperation {
  AccessChange(spadina_core::access::AccessTarget),
  AccessChangeLocation,
  AccountLock(String),
  Announcements,
  AssetCreation,
  Avatar,
  BookmarkChange,
  BookmarkList,
  Calendar,
  CalendarReset,
  DirectMessageSend(spadina_core::player::PlayerIdentifier<String>),
  DirectMessageStats,
  InvitationCreation,
  PlayerLocation(spadina_core::player::PlayerIdentifier<String>),
  PublicKey,
  RealmAccessChange(spadina_core::realm::RealmAccessTarget),
  RealmAnnouncements,
  RealmCalendarChange,
  RealmCalendarList,
  RealmCreation,
  RealmDeletion,
  RealmList(spadina_core::realm::RealmSource<String>),
  RemoteServer,
}
struct InflightRequest {
  created: chrono::DateTime<chrono::Utc>,
  operation: InflightOperation,
}
pub struct FailedOperation {
  pub created: chrono::DateTime<chrono::Utc>,
  pub failed: chrono::DateTime<chrono::Utc>,
  pub reason: std::borrow::Cow<'static, str>,
  pub operation: InflightOperation,
}
