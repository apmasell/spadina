use crate::asset_store::manager::RealmTemplate;
use crate::database::location_persistence::{LocationAccess, LocationAnnouncements, LocationName};
use crate::database::{persisted, Database};
use crate::directory::location_endpoint::{LocationEndpoint, LocationJoin};
use crate::directory::{location_endpoint, Directory};
use crate::join_request::JoinRequest;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use crate::server_controller_template::ServerControllerTemplate;
use crate::stream_map::{OutputMapper, StreamsUnorderedMap};
use chrono::Utc;
use diesel::QueryResult;
use serde_json::Value;
use spadina_core::access::{AccessSetting, Privilege};
use spadina_core::avatar::Avatar;
use spadina_core::controller::{Controller, ControllerInput, ControllerOutput, ControllerTemplate, GenericControllerTemplate, PlayerKind};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::communication::ChatMessage;
use spadina_core::location::directory::Visibility;
use spadina_core::location::protocol::{KickResult, LocationRequest, LocationResponse};
use spadina_core::location::Descriptor;
use spadina_core::player::{PlayerIdentifier, SharedPlayerIdentifier};
use spadina_core::reference_converter::{AsArc, AsReference, AsShared, ToClone};
use spadina_core::shared_ref::SharedRef;
use spadina_core::UpdateResult;
use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_stream::StreamExt;

pub enum Event {
  Add(JoinRequest),
  Ignore,
  Leave(SharedPlayerIdentifier),
  Player(SharedPlayerIdentifier, PlayerEvent),
  Quit,
  Timer,
  VisibilityChange(Visibility),
}
pub struct Player {
  id: u32,
  avatar: Avatar,
  principal: SharedPlayerIdentifier,
  output: mpsc::Sender<PlayerLocationUpdate>,
  kind: PlayerKind,
  input: mpsc::Receiver<PlayerEvent>,
}
impl futures::Stream for Player {
  type Item = PlayerEvent;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    self.get_mut().input.poll_recv(cx)
  }
}

impl OutputMapper<SharedPlayerIdentifier> for Player {
  type Output = Event;

  fn handle(&mut self, key: &SharedPlayerIdentifier, value: Self::Item) -> Option<Self::Output> {
    Some(Event::Player(key.clone(), value))
  }

  fn end(self, key: &SharedPlayerIdentifier) -> Option<Self::Output> {
    Some(Event::Leave(key.clone()))
  }
}

async fn run<C: Controller<Input = Vec<u8>, Output = Vec<u8>>>(
  mut controller: C,
  owner_name: Arc<str>,
  local_server: Arc<str>,
  descriptor: Descriptor<Arc<str>>,
  mut location_join: LocationJoin,
  db_id: i32,
  database: Database,
  mut waiting: Vec<JoinRequest>,
) -> QueryResult<()> {
  let mut acl = persisted::PersistedLocal::new(database.clone(), LocationAccess(db_id))?;
  let mut announcements = persisted::PersistedLocal::new(database.clone(), LocationAnnouncements(db_id))?;
  let mut location_name = persisted::PersistedLocal::new(database.clone(), LocationName(db_id))?;
  let (mut visibility, mut visibility_updates) = database.location_visibility(db_id)?;

  let mut players = StreamsUnorderedMap::<BTreeMap<SharedPlayerIdentifier, Player>>::default();
  let mut identifiers = BTreeMap::new();
  let mut id_generator = 0_u32;
  let mut output = Vec::new();

  loop {
    let message = if let Some(join_request) = waiting.pop() {
      if join_request.tx.is_closed() {
        continue;
      }
      Event::Add(join_request)
    } else if output.is_empty() {
      let timer = controller.next_timer();
      let timer = async {
        match timer {
          None => futures::future::pending().await,
          Some(time) => tokio::time::sleep(time).await,
        }
      };
      let mut incoming = location_join.stream(players.len());
      tokio::select! { biased;
        Some(player_event) = players.next() => player_event,
        _ = timer => Event::Timer,
        p = incoming.next() => p.map(Event::Add).unwrap_or(Event::Quit),
        Some(v) = visibility_updates.next() => Event::VisibilityChange(v),
      }
    } else {
      Event::Ignore
    };
    let mut dead = HashSet::new();
    match message {
      Event::Add(player) => {
        let acl_result = if player.name.reference(AsReference::<str>::default()) == PlayerIdentifier::Local(&owner_name) {
          Some(PlayerKind::Owner)
        } else {
          match acl.read().check(&player.name, &local_server) {
            Privilege::Access => Some(PlayerKind::Regular),
            Privilege::Admin => Some(PlayerKind::Admin),
            Privilege::Deny => None,
          }
        };
        if let Some(kind) = acl_result {
          let id = id_generator;
          id_generator = id_generator.wrapping_add(1);

          output.extend(controller.process(ControllerInput::Add {
            player: player.name.reference(AsReference::<str>::default()),
            player_kind: kind,
            player_id: id,
          }));
          let avatars = players.iter().map(|(player, handle)| (player.clone(), handle.avatar.clone())).collect();
          let name = location_name.read();
          if player
            .tx
            .try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::Location {
              owner: owner_name.clone(),
              server: local_server.clone(),
              name: name.clone(),
              descriptor: descriptor.clone(),
            }))
            .is_err()
            || player.tx.try_send(PlayerLocationUpdate::ResponseShared(LocationResponse::AvatarUpdate { avatars })).is_err()
          {
            output.extend(controller.process(ControllerInput::Remove {
              player: player.name.reference(AsReference::<str>::default()),
              player_kind: kind,
              player_id: id,
            }));
          } else {
            let update = LocationResponse::AvatarUpdate { avatars: vec![(player.name.clone(), player.avatar.clone())] };
            for (update_player, handle) in players.iter() {
              if handle.output.try_send(PlayerLocationUpdate::ResponseShared(update.clone())).is_err() {
                dead.insert(update_player.clone());
              }
            }
            identifiers.insert(id, player.name.clone());
            players
              .mutate()
              .insert(player.name.clone(), Player { id, avatar: player.avatar, principal: player.name, kind, output: player.tx, input: player.rx });
          }
        } else {
          let _ = player.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::PermissionError));
        }
      }
      Event::Ignore => (),
      Event::Leave(player) => {
        dead.insert(player);
      }
      Event::Player(player, PlayerEvent::Avatar(a)) => {
        let response = LocationResponse::AvatarUpdate { avatars: vec![(player.clone(), a.clone())] };
        for (update_player, handle) in players.iter_mut() {
          if &player == update_player {
            handle.avatar = a.clone();
          } else {
            if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
              dead.insert(update_player.clone());
            }
          }
        }
      }
      Event::Player(player, PlayerEvent::Request(r)) => {
        let response = match r {
          LocationRequest::AccessGet => {
            let AccessSetting { default, rules } = acl.read();
            Some(PlayerLocationUpdate::ResponseShared(LocationResponse::AccessCurrent { rules: rules.clone(), default: *default }))
          }
          LocationRequest::AccessSet { id, default, rules } => Some(PlayerLocationUpdate::ResponseShared(LocationResponse::AccessChange {
            id,
            result: if acl.read().check(&player, &local_server) == Privilege::Admin {
              let new = AccessSetting { default, rules };
              for (player, handle) in players.iter_mut() {
                if player.reference(AsReference::<str>::default()) != PlayerIdentifier::Local(owner_name.as_ref()) {
                  handle.kind = match new.check(player, &local_server) {
                    Privilege::Access => PlayerKind::Regular,
                    Privilege::Admin => PlayerKind::Admin,
                    Privilege::Deny => {
                      dead.insert(player.clone());
                      PlayerKind::Regular
                    }
                  };
                }
              }
              acl.mutate(|a| {
                *a = new.convert(AsArc::<str>::default());
                UpdateResult::Success
              })
            } else {
              UpdateResult::NotAllowed
            },
          })),
          LocationRequest::AnnouncementAdd { id, announcement } => Some(PlayerLocationUpdate::ResponseShared(LocationResponse::AnnouncementUpdate {
            id,
            result: if acl.read().check(&player, &local_server) == Privilege::Admin {
              let result = announcements.mutate(|a| {
                a.push(announcement.convert(AsArc::<str>::default()));
                UpdateResult::Success
              });
              let response = LocationResponse::Announcements(announcements.read().clone());
              for (player, handle) in players.iter_mut() {
                if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                  dead.insert(player.clone());
                }
              }
              result
            } else {
              UpdateResult::NotAllowed
            },
          })),
          LocationRequest::AnnouncementClear { id } => Some(PlayerLocationUpdate::ResponseShared(LocationResponse::AnnouncementUpdate {
            id,
            result: if acl.read().check(&player, &local_server) == Privilege::Admin {
              let result = announcements.mutate(|a| {
                a.clear();
                UpdateResult::Success
              });
              let response = LocationResponse::Announcements(vec![]);
              for (player, handle) in players.iter_mut() {
                if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                  dead.insert(player.clone());
                }
              }
              result
            } else {
              UpdateResult::NotAllowed
            },
          })),
          LocationRequest::AnnouncementList => {
            Some(PlayerLocationUpdate::ResponseShared(LocationResponse::Announcements(announcements.read().clone())))
          }
          LocationRequest::ChangeName { id, name } => Some(PlayerLocationUpdate::ResponseShared(LocationResponse::NameChange {
            id,
            result: if acl.read().check(&player, &local_server) == Privilege::Admin {
              let result = location_name.mutate(|location_name| {
                *location_name = Arc::from(name);
                UpdateResult::Success
              });
              let name = location_name.read();
              let response = LocationResponse::NameChanged { name: name.clone() };
              for (player, handle) in players.iter_mut() {
                if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
                  dead.insert(player.clone());
                }
              }

              result
            } else {
              UpdateResult::NotAllowed
            },
          })),
          LocationRequest::Delete => {
            if acl.read().check(&player, &local_server) == Privilege::Admin {
              output.push(ControllerOutput::Quit);
            }
            None
          }
          LocationRequest::Kick { id, target } => {
            let target = target.localize(&local_server);
            Some(PlayerLocationUpdate::ResponseShared(LocationResponse::Kick {
              id,
              result: if acl.read().check(&player, &local_server) == Privilege::Admin {
                match players.remove(&target.convert(AsArc::<str>::default())) {
                  None => KickResult::NotPresent,
                  Some(handle) => {
                    let _ = handle.output.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::PermissionError));
                    identifiers.remove(&handle.id);
                    output.extend(controller.process(ControllerInput::Remove {
                      player: handle.principal.reference(AsReference::<str>::default()),
                      player_id: handle.id,
                      player_kind: handle.kind,
                    }));
                    KickResult::Success
                  }
                }
              } else {
                KickResult::NotAllowed
              },
            }))
          }
          LocationRequest::MessageClear { id, from, to } => Some(PlayerLocationUpdate::ResponseShared(LocationResponse::MessageClear {
            id,
            result: if acl.read().check(&player, &local_server) == Privilege::Admin {
              match database.location_chat_delete(db_id, from, to) {
                Ok(()) => UpdateResult::Success,
                Err(e) => {
                  eprintln!("Failed to delete chat for location {:?} (id={}): {}", &descriptor, db_id, e);
                  UpdateResult::InternalError
                }
              }
            } else {
              UpdateResult::NotAllowed
            },
          })),
          LocationRequest::MessageSend { body } => {
            let result = if body.is_transient() { Ok(Utc::now()) } else { database.location_chat_write(db_id, &player, &body) };
            match result {
              Ok(timestamp) => {
                let message = ChatMessage::<Arc<str>> { body: body.convert(AsArc::<str>::default()), sender: player.clone(), timestamp };
                let update = LocationResponse::MessagePosted(message);
                for (player, handle) in players.iter_mut() {
                  if handle.output.try_send(PlayerLocationUpdate::ResponseShared(update.clone())).is_err() {
                    dead.insert(player.clone());
                  }
                }
              }
              Err(e) => {
                eprintln!("Failed to write chat for location {:?} (id={}): {}", &descriptor, db_id, e);
              }
            }
            None
          }
          LocationRequest::MessagesGet { from, to } => match database.location_messages(db_id, from, to) {
            Err(e) => {
              eprintln!("Failed to read chat for location {:?} (id={}): {}", &descriptor, db_id, e);
              None
            }
            Ok(messages) => Some(PlayerLocationUpdate::ResponseSingle(LocationResponse::Messages { messages, from, to })),
          },
          LocationRequest::Internal(request_id, request) => {
            if let Some(handle) = players.get(&player) {
              output.extend(controller.process(ControllerInput::Input {
                request_id,
                player: player.reference(AsReference::<str>::default()),
                player_id: handle.id,
                player_kind: handle.kind,
                request,
              }))
            }

            None
          }
        };
        if let Some(response) = response {
          if let Some(handle) = players.get(&player) {
            if handle.output.try_send(response).is_err() {
              dead.insert(player);
            }
          }
        }
      }
      Event::Quit => break,
      Event::Timer => output.extend(controller.process(ControllerInput::Timer)),
      Event::VisibilityChange(v) => visibility = v,
    }
    for output in output.drain(..) {
      match output {
        ControllerOutput::Broadcast { response } => {
          let response = LocationResponse::Internal(Arc::<[u8]>::from(response));
          for (player, handle) in players.iter_mut() {
            if handle.output.try_send(PlayerLocationUpdate::ResponseShared(response.clone())).is_err() {
              dead.insert(player.clone());
            }
          }
        }
        ControllerOutput::Move { player, target } => {
          if let Some(handle) = identifiers.get(&player).map(|p| players.get(&p)).flatten() {
            if handle.output.try_send(PlayerLocationUpdate::Move(target.convert(AsShared::<str>::default()))).is_err() {
              dead.insert(handle.principal.clone());
            }
          }
        }
        ControllerOutput::Quit => {
          if let Err(e) = database.location_delete(db_id) {
            eprintln!("Failed to delete location {:?} (id={}): {}", &descriptor, db_id, e);
          }
          break;
        }
        ControllerOutput::Response { player, response } => {
          if let Some(handle) = identifiers.get(&player).map(|p| players.get(p)).flatten() {
            {
              if handle.output.try_send(PlayerLocationUpdate::ResponseSingle(LocationResponse::Internal(response))).is_err() {
                dead.insert(handle.principal.clone());
              }
            }
          }
        }
      }
    }
    {
      let mut players = players.mutate();
      for player in dead {
        if let Some(handle) = players.remove(&player) {
          let _ = handle.output.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::NoWhere));
          identifiers.remove(&handle.id);
          output.extend(controller.process(ControllerInput::Remove {
            player: handle.principal.reference(AsReference::<str>::default()),
            player_kind: handle.kind,
            player_id: handle.id,
          }));
        }
      }
    }
    if visibility.is_writable() {
      let json = match controller.to_json() {
        Ok(json) => json,
        Err(e) => {
          eprintln!("Failed to serialize location state for {:?} (id={}): {}", &descriptor, db_id, e);
          continue;
        }
      };
      if let Err(e) = database.location_state_write(db_id, json) {
        eprintln!("Failed to write location state for {:?} (id={}): {}", &descriptor, db_id, e);
      }
    }
  }
  Ok(())
}

pub async fn create_location(
  template: ServerControllerTemplate,
  owner: Arc<str>,
  descriptor: Descriptor<SharedRef<str>>,
  database: &Database,
  directory: &Directory,
) -> LocationEndpoint {
  let (location_endpoint, location_join) = location_endpoint::new(directory.access_management.give_me_death());
  if let Some(future) =
    create_new_location(template, location_join, owner, descriptor.reference(ToClone::<str>::default()), database.clone(), directory, Vec::new())
  {
    spawn(future);
  }
  location_endpoint
}

fn create_new_location(
  template: ServerControllerTemplate,
  location_join: LocationJoin,
  owner: Arc<str>,
  descriptor: Descriptor<Arc<str>>,
  database: Database,
  directory: &Directory,
  waiting: Vec<JoinRequest>,
) -> Option<impl Future<Output = ()> + Send + Sync + 'static> {
  let controller = template.blank();
  let state = match controller.to_json() {
    Ok(state) => state,
    Err(e) => {
      eprintln!("Failed to serialize state for new location: {}", e);
      location_join.into_black_hole(LocationChangeResponse::InternalError);
      return None;
    }
  };
  match database.location_create(&descriptor.reference(AsReference::<str>::default()), &owner, template.name(&owner).as_ref(), state) {
    Ok(db_id) => {
      let server_name = directory.access_management.server_name.clone();
      Some(async move {
        if let Err(e) = run(controller, owner, server_name, descriptor, location_join, db_id, database, waiting).await {
          eprintln!("Failed to load state for new location (id={}): {}", db_id, e);
        }
      })
    }
    Err(e) => {
      eprintln!("Failed to serialize state for new location: {}", e);
      location_join.into_black_hole(LocationChangeResponse::InternalError);
      None
    }
  }
}

async fn load_location<CT: ControllerTemplate>(
  template: CT,
  owner: Arc<str>,
  descriptor: Descriptor<Arc<str>>,
  db_id: i32,
  state: Value,
  location_join: LocationJoin,
  database: Database,
  directory: Directory,
  waiting: Vec<JoinRequest>,
) where
  CT::Controller: Controller<Input = Vec<u8>, Output = Vec<u8>>,
{
  match template.load_json(state) {
    Ok(controller) => {
      if let Err(e) =
        run(controller, owner, directory.access_management.server_name.clone(), descriptor, location_join, db_id, database, waiting).await
      {
        eprintln!("Failed to load location (id={}): {}", db_id, e);
      }
    }
    Err(e) => {
      eprintln!("Corrupt state for location (id={}): {:?}", db_id, e);
      location_join.into_black_hole(LocationChangeResponse::UnsupportedError);
    }
  }
}

pub fn load(db_id: i32, owner: Arc<str>, descriptor: Descriptor<Arc<str>>, database: Database, directory: Directory) -> LocationEndpoint {
  let (location_endpoint, location_join) = location_endpoint::new(directory.access_management.give_me_death());
  spawn(async move {
    let state = match database.location_state_read(db_id) {
      Ok(state) => state,
      Err(e) => {
        eprintln!("Failed to load location data (id={}): {}", db_id, e);
        location_join.into_black_hole(LocationChangeResponse::InternalError);
        return;
      }
    };

    let (template, location_join, waiting) = match &descriptor {
      Descriptor::Asset(asset) => match find_asset(asset, &directory, location_join).await {
        Ok((template, location_join, waiting)) => (ServerControllerTemplate::Asset(template), location_join, waiting),
        Err(()) => return,
      },
      Descriptor::Application(a, _) => (ServerControllerTemplate::Application(*a), location_join, Vec::new()),
      Descriptor::Unsupported(s, _) => {
        eprintln!("Unsupported location type in database (id={}): {}", db_id, s);
        location_join.into_black_hole(LocationChangeResponse::UnsupportedError);
        return;
      }
    };
    load_location(template, owner, descriptor, db_id, state, location_join, database, directory, waiting).await
  });
  location_endpoint
}

pub async fn find_and_create_realm(player: Arc<str>, asset: Arc<str>, database: Database, directory: Directory) -> LocationEndpoint {
  let (location_endpoint, location_join) = location_endpoint::new(directory.access_management.give_me_death());
  spawn(async move {
    if let Ok((template, location_join, waiting)) = find_asset(&asset, &directory, location_join).await {
      if let Some(future) =
        create_new_location(ServerControllerTemplate::Asset(template), location_join, player, Descriptor::Asset(asset), database, &directory, waiting)
      {
        future.await
      }
    }
  });
  location_endpoint
}

async fn find_asset(
  asset: &Arc<str>,
  directory: &Directory,
  mut location_join: LocationJoin,
) -> Result<(GenericControllerTemplate, LocationJoin, Vec<JoinRequest>), ()> {
  let mut rx = match directory.pull_realm(asset.clone()).await {
    Ok(rx) => rx,
    Err(()) => {
      location_join.into_black_hole(LocationChangeResponse::InternalError);
      return Err(());
    }
  };
  let sleep = sleep(Duration::from_secs(15 * 60));
  tokio::pin!(sleep);
  let mut waiting = Vec::<JoinRequest>::new();
  let template: RealmTemplate = loop {
    let mut players = location_join.stream(waiting.len());
    tokio::select! {biased;
      t = &mut rx => match t {
        Ok(t) => break t,
        Err(_) => {
          for join_request in waiting {
            let _ = join_request.tx.send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::ResolutionError));
          }
          return Err(());
        }
      },
      join_request = players.next() => match join_request {
        Some(join_request) => {
          let _ = join_request.tx.send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::WaitingForAsset));
          waiting.push(join_request);
        }
        None => {
          return Err(());
        }
      },
      _ = &mut sleep => {
        return Err(());
      }
    }
  };
  match template {
    RealmTemplate::Found(template) => Ok((template, location_join, waiting)),
    RealmTemplate::Invalid => {
      for join_request in waiting {
        let _ = join_request.tx.send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::UnsupportedError));
      }
      location_join.into_black_hole(LocationChangeResponse::UnsupportedError);
      Err(())
    }
    RealmTemplate::MissingCapabilities(capabilities) => {
      for join_request in waiting {
        let _ = join_request
          .tx
          .send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::MissingCapabilitiesError { capabilities: capabilities.clone() }));
      }
      location_join.into_black_hole(LocationChangeResponse::MissingCapabilitiesError { capabilities });
      Err(())
    }

    RealmTemplate::NotFound(missing) => {
      for join_request in waiting {
        let _ =
          join_request.tx.send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::MissingAssetError { assets: vec![missing.clone()] }));
      }
      // Don't black hole to allow a retry
      Err(())
    }
  }
}
