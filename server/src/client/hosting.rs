use crate::access::AccessManagement;
use crate::directory::location_endpoint;
use crate::directory::location_endpoint::LocationEndpoint;
use crate::join_request::JoinRequest;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use crate::stream_map::{OutputMapper, StreamsUnorderedMap};
use chrono::Utc;
use futures::StreamExt;
use spadina_core::access::{AccessSetting, Privilege};
use spadina_core::avatar::Avatar;
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::communication::ChatMessage;
use spadina_core::location::protocol::LocationResponse;
use spadina_core::location::protocol::{KickResult, LocationRequest};
use spadina_core::location::DescriptorKind;
use spadina_core::net::server::hosting::{HostCommand, HostEvent};
use spadina_core::net::server::ClientResponse;
use spadina_core::player::PlayerIdentifier;
use spadina_core::player::SharedPlayerIdentifier;
use spadina_core::reference_converter::{AsArc, AsReference, AsSingle};
use spadina_core::UpdateResult;
use std::collections::{HashMap, HashSet, VecDeque};
use std::iter;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub enum HostInput {
  Command(HostCommand<String, Vec<u8>>),
  Request(PlayerEvent),
}
struct Player {
  avatar: Avatar,
  principal: SharedPlayerIdentifier,
  output: mpsc::Sender<PlayerLocationUpdate>,
  input: mpsc::Receiver<PlayerEvent>,
  is_admin: bool,
}
impl futures::Stream for Player {
  type Item = PlayerEvent;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    self.get_mut().input.poll_recv(cx)
  }
}
enum Event {
  Add(JoinRequest),
  Command(HostCommand<String, Vec<u8>>),
  Leave(SharedPlayerIdentifier),
  Player(Option<SharedPlayerIdentifier>, PlayerEvent),
}
impl From<HostInput> for Event {
  fn from(value: HostInput) -> Self {
    match value {
      HostInput::Command(c) => Event::Command(c),
      HostInput::Request(r) => Event::Player(None, r),
    }
  }
}
pub fn start_hosting(
  descriptor: DescriptorKind<Arc<str>>,
  owner_name: Arc<str>,
  auth: &AccessManagement,
  avatar: Avatar,
  acl: AccessSetting<String, Privilege>,
) -> (LocationEndpoint, mpsc::Receiver<Message>, mpsc::Sender<HostInput>) {
  let (location_endpoint, mut incoming_rx) = location_endpoint::new(auth.give_me_death());
  let (client_tx, client_rx) = mpsc::channel::<Message>(50);
  let (input_tx, mut input_rx) = mpsc::channel(50);
  let local_server = auth.server_name.clone();
  let mut acl = acl.convert(AsArc::<str>::default());
  tokio::spawn(async move {
    let mut players = StreamsUnorderedMap::<HashMap<SharedPlayerIdentifier, Player>>::default();
    let mut chat = VecDeque::<ChatMessage<Arc<str>>>::new();
    let mut announcements = Vec::new();
    let mut location_name = owner_name.clone();
    loop {
      let player_count = players.len();
      let mut incoming = incoming_rx.stream(player_count);
      let message = tokio::select! { biased;
        i = input_rx.recv() => i.map(Into::into).unwrap_or(Event::Command(HostCommand::Quit)),
        Some(player_event) = players.next() => player_event,
        p = incoming.next() => p.map(Event::Add).unwrap_or(Event::Command(HostCommand::Quit)),
      };
      let mut dead = HashSet::new();
      chat.truncate(100);
      match message {
        Event::Add(player) => {
          let acl_result = acl.check(&player.name, &local_server);
          if acl_result.can_access() {
            let is_admin = acl_result == Privilege::Admin;
            if client_tx
              .send(
                ClientResponse::<_, &[u8]>::ToHost {
                  event: HostEvent::PlayerEntered {
                    player: player.name.reference(AsReference::<str>::default()),
                    avatar: player.avatar.clone(),
                    is_admin,
                  },
                }
                .into(),
              )
              .await
              .is_err()
            {
              break;
            }
            let avatars = players
              .iter()
              .map(|(player, handle)| (player.clone(), handle.avatar.clone()))
              .chain(iter::once((PlayerIdentifier::Local(owner_name.clone()), avatar.clone())))
              .collect();
            if player
              .tx
              .try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::Guest {
                host: PlayerIdentifier::Remote { player: owner_name.clone(), server: local_server.clone() },
                descriptor: descriptor.clone(),
                name: location_name.clone(),
              }))
              .is_err()
              || player.tx.try_send(PlayerLocationUpdate::ResponseShared(LocationResponse::AvatarUpdate { avatars })).is_err()
            {
              if client_tx
                .send(
                  ClientResponse::<_, &[u8]>::ToHost {
                    event: HostEvent::PlayerLeft { player: player.name.reference(AsReference::<str>::default()) },
                  }
                  .into(),
                )
                .await
                .is_err()
              {
                break;
              }
            } else {
              let update = LocationResponse::AvatarUpdate { avatars: vec![(player.name.clone(), player.avatar.clone())] };
              for (update_player, handle) in players.iter() {
                if handle.output.try_send(PlayerLocationUpdate::ResponseShared(update.clone())).is_err() {
                  dead.insert(update_player.clone());
                }
              }
              players
                .mutate()
                .insert(player.name.clone(), Player { avatar: player.avatar, principal: player.name, output: player.tx, input: player.rx, is_admin });
            }
          } else {
            let _ = player.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::PermissionError));
          }
        }
        Event::Command(HostCommand::Broadcast { response }) => {
          let response = LocationResponse::Internal(Arc::from(response));
          for (player, handle) in players.iter_mut() {
            if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
              dead.insert(player.clone());
            }
          }
        }
        Event::Command(HostCommand::Drop { player }) => {
          let player = player.convert(AsArc::<str>::default());
          if let Some(handle) = players.mutate().remove(&player) {
            let _ = handle.output.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::PermissionError));
          }
        }
        Event::Command(HostCommand::Move { player, target }) => {
          let player = player.convert(AsArc::<str>::default());
          if let Some(handle) = players.get(&player) {
            if handle.output.try_send(PlayerLocationUpdate::Move(target.convert(AsSingle::<str>::default()))).is_err() {
              dead.insert(player);
            }
          }
        }
        Event::Command(HostCommand::Quit) => break,
        Event::Command(HostCommand::RequestError { player, request_id: id }) => {
          let player = player.convert(AsArc::<str>::default());
          if let Some(handle) = players.get(&player) {
            if handle.output.try_send(PlayerLocationUpdate::ResponseSingle(LocationResponse::RequestError { id })).is_err() {
              dead.insert(player);
            }
          }
        }
        Event::Command(HostCommand::Response { player, response }) => {
          let player = player.convert(AsArc::<str>::default());
          if let Some(handle) = players.get(&player) {
            if handle.output.try_send(PlayerLocationUpdate::ResponseSingle(LocationResponse::Internal(response))).is_err() {
              dead.insert(player);
            }
          }
        }
        Event::Leave(player) => {
          dead.insert(player);
        }
        Event::Player(player, PlayerEvent::Avatar(a)) => {
          let response =
            LocationResponse::AvatarUpdate { avatars: vec![(player.clone().unwrap_or(PlayerIdentifier::Local(owner_name.clone())), a.clone())] };
          for (update_player, handle) in players.iter_mut() {
            if player.as_ref() == Some(update_player) {
              handle.avatar = a.clone();
            } else {
              if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                dead.insert(update_player.clone());
              }
            }
          }
          if client_tx.send(ClientResponse::InLocation { response }.into()).await.is_err() {
            break;
          }
        }
        Event::Player(player, PlayerEvent::Request(r)) => {
          let response = match r {
            LocationRequest::AccessGet => Some(LocationResponse::AccessCurrent { rules: acl.rules.clone(), default: acl.default }),
            LocationRequest::AccessSet { id, default, rules } => Some(LocationResponse::AccessChange {
              id,
              result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                acl = AccessSetting { default, rules: rules.into_iter().map(|rule| rule.convert(AsArc::<str>::default())).collect() };
                for (player, handle) in &mut *players.mutate() {
                  handle.is_admin = match acl.check(&player, &local_server) {
                    Privilege::Access => false,
                    Privilege::Admin => true,
                    Privilege::Deny => {
                      dead.insert(player.clone());
                      false
                    }
                  };
                }
                UpdateResult::Success
              } else {
                UpdateResult::NotAllowed
              },
            }),
            LocationRequest::AnnouncementAdd { id, announcement } => Some(LocationResponse::AnnouncementUpdate {
              id,
              result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                announcements.push(announcement.convert(AsArc::<str>::default()));
                let response = LocationResponse::Announcements(announcements.clone());
                for (player, handle) in players.iter_mut() {
                  if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                    dead.insert(player.clone());
                  }
                }
                if client_tx.send(ClientResponse::InLocation { response }.into()).await.is_err() {
                  break;
                }
                UpdateResult::Success
              } else {
                UpdateResult::NotAllowed
              },
            }),
            LocationRequest::AnnouncementClear { id } => Some(LocationResponse::AnnouncementUpdate {
              id,
              result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                announcements.clear();
                let response = LocationResponse::Announcements(vec![]);
                for (player, handle) in players.iter_mut() {
                  if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                    dead.insert(player.clone());
                  }
                }
                if client_tx.send(ClientResponse::InLocation { response }.into()).await.is_err() {
                  break;
                }
                UpdateResult::Success
              } else {
                UpdateResult::NotAllowed
              },
            }),
            LocationRequest::AnnouncementList => Some(LocationResponse::Announcements(announcements.clone())),
            LocationRequest::ChangeName { id, name, .. } => Some(LocationResponse::NameChange {
              id,
              result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                location_name = Arc::from(name);
                let response = LocationResponse::NameChanged { name: location_name.clone() };
                for (player, handle) in players.iter_mut() {
                  if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                    dead.insert(player.clone());
                  }
                }
                if client_tx.send(ClientResponse::InLocation { response }.into()).await.is_err() {
                  break;
                }
                UpdateResult::Success
              } else {
                UpdateResult::NotAllowed
              },
            }),
            LocationRequest::Delete => {
              if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                break;
              } else {
                None
              }
            }
            LocationRequest::Kick { id, target } => {
              let target = target.localize(&local_server);
              Some(LocationResponse::Kick {
                id,
                result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                  match players.mutate().remove(&target.convert(AsArc::<str>::default())) {
                    None => KickResult::NotPresent,
                    Some(handle) => {
                      if client_tx
                        .send(ClientResponse::<_, &[u8]>::ToHost { event: HostEvent::PlayerLeft { player: handle.principal } }.into())
                        .await
                        .is_err()
                      {
                        break;
                      }
                      KickResult::Success
                    }
                  }
                } else {
                  KickResult::NotAllowed
                },
              })
            }
            LocationRequest::MessageClear { id, from, to } => Some(LocationResponse::MessageClear {
              id,
              result: if player.as_ref().map(|p| acl.check(p, &local_server) == Privilege::Admin).unwrap_or(true) {
                chat.retain(|m| m.timestamp < from || m.timestamp > to);
                UpdateResult::Success
              } else {
                UpdateResult::NotAllowed
              },
            }),
            LocationRequest::MessageSend { body } => {
              let message = ChatMessage::<Arc<str>> {
                body: body.convert(AsArc::<str>::default()),
                sender: player.clone().unwrap_or(PlayerIdentifier::Local(owner_name.clone())),
                timestamp: Utc::now(),
              };
              if !message.body.is_transient() {
                chat.push_back(message.clone());
              }
              let update = LocationResponse::MessagePosted(message);
              for (player, handle) in players.iter_mut() {
                if handle.output.try_send(PlayerLocationUpdate::ResponseShared(update.clone())).is_err() {
                  dead.insert(player.clone());
                }
              }
              if client_tx.send(ClientResponse::InLocation { response: update }.into()).await.is_err() {
                break;
              }

              None
            }
            LocationRequest::MessagesGet { from, to } => Some(LocationResponse::Messages {
              messages: chat.iter().filter(|message| (from..=to).contains(&message.timestamp)).cloned().collect(),
              from,
              to,
            }),
            LocationRequest::Internal(request_id, request) => {
              if let Some((player, handle)) = player.as_ref().map(|p| players.get_key_value(p)).flatten() {
                if client_tx
                  .send(
                    ClientResponse::ToHost {
                      event: HostEvent::PlayerRequest { request_id, player: player.clone(), is_admin: handle.is_admin, request },
                    }
                    .into(),
                  )
                  .await
                  .is_err()
                {
                  break;
                }
              }
              None
            }
          };
          if let Some(response) = response {
            match player {
              None => {
                if client_tx.send(ClientResponse::InLocation { response }.into()).await.is_err() {
                  break;
                }
              }
              Some(player) => {
                if let Some(handle) = players.get(&player) {
                  if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response)).is_err() {
                    dead.insert(player);
                  }
                }
              }
            }
          }
        }
      }
      {
        let mut players = players.mutate();
        for player in dead {
          if players.remove(&player).is_some() {
            if client_tx.send(ClientResponse::ToHost { event: HostEvent::<_, &[u8]>::PlayerLeft { player } }.into()).await.is_err() {
              break;
            }
          }
        }
      }
    }
  });
  (location_endpoint, client_rx, input_tx)
}

impl OutputMapper<PlayerIdentifier<Arc<str>>> for Player {
  type Output = Event;

  fn handle(&mut self, key: &PlayerIdentifier<Arc<str>>, value: Self::Item) -> Option<Self::Output> {
    Some(Event::Player(Some(key.clone()), value))
  }

  fn end(self, key: &PlayerIdentifier<Arc<str>>) -> Option<Self::Output> {
    Some(Event::Leave(key.clone()))
  }
}
