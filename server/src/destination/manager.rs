use crate::destination::Owner;
use futures::{FutureExt, StreamExt};
use spadina_core::asset_store::AsyncAssetStore;

use crate::realm::puzzle::PlayerKey;
enum ControlRequest<D: crate::destination::Destination>
where
  D::Item: Send + Sync + 'static,
{
  AddPlayer(crate::destination::PlayerHandle<D>),
  Delete(Option<super::SharedPlayerId>, tokio::sync::oneshot::Sender<spadina_core::UpdateResult>),
  Quit,
}

pub(crate) struct DestinationManager<D: crate::destination::Destination>
where
  D::Item: Send + Sync + 'static,
{
  tx: tokio::sync::mpsc::Sender<ControlRequest<D>>,
  identifier: D::Identifier,
  activity: crate::destination::activity::AtomicActivity,
}
pub(crate) enum PlayerEvent<D: crate::destination::Destination>
where
  D::Item: Send + Sync + 'static,
{
  Avatar(spadina_core::avatar::Avatar),
  Request(crate::destination::DestinationRequest<D::Request>),
}

#[pin_project::pin_project(project = PlayerInformationProjection)]
pub(crate) struct PlayerInformation<D: crate::destination::Destination>
where
  D::Item: Send + Sync + 'static,
{
  #[pin]
  avatar: tokio_stream::wrappers::WatchStream<spadina_core::avatar::Avatar>,
  pub capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
  pub is_superuser: bool,
  pub principal: crate::destination::SharedPlayerId,
  output: tokio::sync::mpsc::Sender<crate::destination::DestinationResponse<D::Response>>,
  #[pin]
  input: tokio::sync::mpsc::Receiver<crate::destination::DestinationRequest<D::Request>>,
}

impl<D: crate::destination::Destination> PlayerInformation<D>
where
  D::Item: Send + Sync + 'static,
{
  fn new(handle: crate::destination::PlayerHandle<D>, owner: &str) -> PlayerInformation<D> {
    let crate::destination::PlayerHandle { avatar, capabilities, is_superuser, principal, tx, rx } = handle;
    let is_superuser = is_superuser || principal.as_ref() == spadina_core::player::PlayerIdentifier::Local(&owner);
    PlayerInformation { avatar: tokio_stream::wrappers::WatchStream::new(avatar), capabilities, is_superuser, principal, output: tx, input: rx }
  }
}
impl<D: crate::destination::Destination> futures::Stream for PlayerInformation<D>
where
  D::Item: Send + Sync + 'static,
{
  type Item = PlayerEvent<D>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    let mut project = self.project();
    match project.input.poll_recv(cx) {
      std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
      std::task::Poll::Ready(Some(r)) => std::task::Poll::Ready(Some(PlayerEvent::Request(r))),
      std::task::Poll::Pending => match project.avatar.poll_next_unpin(cx) {
        std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
        std::task::Poll::Ready(Some(a)) => std::task::Poll::Ready(Some(PlayerEvent::Avatar(a))),
        std::task::Poll::Pending => std::task::Poll::Pending,
      },
    }
  }
}

impl<D: crate::destination::Destination> DestinationManager<D>
where
  D::Item: Send + Sync + 'static,
{
  pub fn new(
    identifier: D::Identifier,
    server_name: std::sync::Arc<str>,
    directory: std::sync::Weak<crate::destination::Directory>,
    mut initializer: tokio::task::JoinHandle<Result<D, spadina_core::location::LocationResponse<crate::shstr::ShStr>>>,
  ) -> Self {
    let activity = crate::destination::activity::AtomicActivity::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(500);
    let result = Self { identifier: identifier.clone(), tx, activity: activity.clone() };
    tokio::task::spawn(async move {
      let mut avatars = std::collections::HashMap::new();
      let mut players = crate::map::StreamsUnorderedMap::<crate::realm::puzzle::PlayerKey, PlayerInformation<D>>::new();
      let mut id_generator = 0_u64;
      let state = loop {
        enum Event<D: crate::destination::Destination>
        where
          D::Item: Send + Sync + 'static,
        {
          Control(ControlRequest<D>),
          Player(PlayerKey, std::option::Option<PlayerEvent<D>>),
        }
        let next = tokio::select! {
          init = &mut initializer => break init,
          control = rx.recv() => match control { Some(control) => Event::Control(control), None => return },
          Some((player, event)) = players.next() => Event::Player(player, event)
        };
        match next {
          Event::Control(ControlRequest::Quit) => {
            for (_, player) in players.iter_mut() {
              if let Err(_) = player
                .output
                .send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::ResolutionFailed))
                .await
              {
                eprintln!("Failed to move {} from {}", &player.principal, &identifier);
              }
            }
            return;
          }
          Event::Control(ControlRequest::Delete(_, _)) => (),
          Event::Control(ControlRequest::AddPlayer(mut handle)) => {
            let id = id_generator;
            id_generator += 1;
            handle.is_superuser |= spadina_core::player::PlayerIdentifier::Local(identifier.owner().as_ref()) == handle.principal.as_ref();
            players.insert(PlayerKey(id), PlayerInformation::new(handle, identifier.owner()));
          }
          Event::Player(key, None) => {
            players.remove(&key);
            avatars.remove(&key);
          }
          Event::Player(key, Some(PlayerEvent::Avatar(avatar))) => {
            avatars.insert(key, avatar);
          }
          Event::Player(_, Some(PlayerEvent::Request(_))) => (),
        }
      };
      let mut state = match state.unwrap_or_else(|e| {
        eprintln!("Failed to join initializer {}: {}", &identifier, e);
        Err(spadina_core::location::LocationResponse::InternalError)
      }) {
        Ok(state) => state,
        Err(e) => {
          for (_, player) in players.iter_mut() {
            if let Err(_) = player.output.send(crate::destination::DestinationResponse::Location(e.clone())).await {
              eprintln!("Failed to move {} from {}", &player.principal, &identifier);
            }
          }
          return;
        }
      };
      let mut unauthorized_players = Vec::new();
      let mut controls = Vec::new();
      for (key, info) in players.iter() {
        let missing_capabilities: Vec<_> =
          state.capabilities().iter().copied().filter(|&c| !info.capabilities.contains(c)).map(|c| c.to_string()).collect();
        if missing_capabilities.is_empty() {
          match state.try_add(&key, &info.principal, info.is_superuser).await {
            Ok((location, control)) => {
              if let Err(_) = info.output.send(crate::destination::DestinationResponse::Location(location)).await {
                eprintln!("Failed to add {} from {}", &info.principal, &identifier);
              }

              controls.extend(control);
            }
            Err(()) => {
              if let Err(_) =
                info.output.send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::PermissionDenied)).await
              {
                eprintln!("Failed to eject {} from {}", &info.principal, &identifier);
              }
              unauthorized_players.push(key.clone());
            }
          }
        } else {
          if let Err(_) = info
            .output
            .send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::MissingCapabilities {
              capabilities: missing_capabilities,
            }))
            .await
          {
            eprintln!("Failed to eject {} from {}", &info.principal, &identifier);
          }
          unauthorized_players.push(key.clone());
        }
      }
      unauthorized_players.into_iter().for_each(|key| {
        players.remove(&key);
      });
      let mut activity_next = tokio::time::Instant::now();
      let mut follow_requests = std::collections::HashMap::new();
      let mut follow_id_generator = 0_i32;
      let mut consensual_emote_requests = std::collections::HashMap::new();
      let mut consensual_emote_id_generator = 0_i32;
      let mut side_tasks = futures::stream::FuturesUnordered::new();
      loop {
        enum Event<D: crate::destination::Destination>
        where
          D::Item: Send + Sync + 'static,
        {
          ActivityTick,
          ConsensualEmoteRequest {
            source: crate::destination::SharedPlayerId,
            target: crate::realm::puzzle::PlayerKey,
            id: i32,
            emote: std::sync::Arc<str>,
          },
          Control(ControlRequest<D>),
          Ignore,
          Internal(D::Item),
          Player(PlayerKey, std::option::Option<PlayerEvent<D>>),
        }
        let next = tokio::select! {
          control = rx.recv() => match control { Some(control) => Event::Control(control), None => return },
          Some(control) = state.next() => Event::Internal(control),
          Some((player, event)) = players.next() => Event::Player(player, event),
          Some(result) = side_tasks.next(), if !side_tasks.is_empty() => result,
          _ = tokio::time::sleep_until(activity_next) => Event::ActivityTick,
        };
        controls.extend(match next {
          Event::Ignore => Vec::new(),
          Event::ActivityTick => {
            activity.update(players.len());
            activity_next += tokio::time::Duration::from_millis(900_000);
            let now = chrono::Utc::now();
            follow_requests.retain(|_, (_, t)| *t < now);
            consensual_emote_requests.retain(|_, (_, _, t)| *t < now);
            Vec::new()
          }
          Event::Control(ControlRequest::AddPlayer(mut handle)) => {
            let missing_capabilities: Vec<_> =
              state.capabilities().iter().copied().filter(|&c| !handle.capabilities.contains(c)).map(|c| c.to_string()).collect();
            if missing_capabilities.is_empty() {
              let id = crate::realm::puzzle::PlayerKey(id_generator);
              id_generator += 1;
              handle.is_superuser |= spadina_core::player::PlayerIdentifier::Local(identifier.owner().as_ref()) == handle.principal.as_ref();
              match state.try_add(&id, &handle.principal, handle.is_superuser).await {
                Ok((location, controls)) => {
                  let info = PlayerInformation::new(handle, identifier.owner());
                  if let Err(_) = info.output.send(crate::destination::DestinationResponse::Location(location)).await {
                    eprintln!("Failed to add {} from {}", &info.principal, &identifier);
                  }
                  players.insert(id, info);
                  controls
                }
                Err(()) => {
                  if let Err(_) = handle
                    .tx
                    .send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::PermissionDenied))
                    .await
                  {
                    eprintln!("Failed to eject {} from {}", &handle.principal, &identifier);
                  }
                  Vec::new()
                }
              }
            } else {
              if let Err(_) = handle
                .tx
                .send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::MissingCapabilities {
                  capabilities: missing_capabilities,
                }))
                .await
              {
                eprintln!("Failed to eject {} from {}", &handle.principal, &identifier);
              }
              Vec::new()
            }
          }
          Event::Control(ControlRequest::Delete(requester, output)) => {
            let result = state.delete(requester);
            let _ = output.send(result);
            if result == spadina_core::UpdateResult::Success {
              return;
            }
            Vec::new()
          }
          Event::Control(ControlRequest::Quit) => {
            for (_, info) in players.iter() {
              if let Err(_) = info.output.send(crate::destination::DestinationResponse::Move(None)).await {
                eprintln!("Failed to eject {} from {}", &info.principal, &identifier);
              }
            }
            state.quit();
            return;
          }
          Event::Internal(events) => state.process_events(events).await,
          Event::Player(key, None) => {
            avatars.remove(&key);
            if let Some((_, active_player)) = players.remove(&key) {
              state.remove_player(&key, &active_player.principal).await
            } else {
              Vec::new()
            }
          }
          Event::Player(key, Some(PlayerEvent::Avatar(avatar))) => {
            avatars.insert(key, avatar);
            Vec::new()
          }
          Event::Player(
            key,
            Some(PlayerEvent::Request(crate::destination::DestinationRequest::ConsensualEmoteRequest { emote, player: target })),
          ) => {
            if let Some(info) = players.get(&key) {
              let target = target.localize(&server_name);
              if info.principal.as_ref() != target.as_ref() {
                if let (Some((target_key, _)), Some(directory)) =
                  (players.iter().filter(|(_, ti)| ti.principal.as_ref() == target.as_ref()).next(), directory.upgrade())
                {
                  let emote = emote.to_arc();
                  let id = consensual_emote_id_generator;
                  consensual_emote_id_generator = consensual_emote_id_generator.wrapping_add(1);
                  consensual_emote_requests
                    .insert((target_key.clone(), id), (key.clone(), emote.clone(), chrono::Utc::now() + chrono::Duration::minutes(5)));
                  let source = info.principal.clone();
                  let target = target_key.clone();
                  side_tasks.push(
                    async move {
                      match directory.asset_manager().pull(&emote).await {
                        Err(e) => {
                          eprintln!("Failed to get consensual emote {}: {}", emote, e);
                          Event::Ignore
                        }
                        Ok(asset) => {
                          if asset.asset_type == "consensual-emote" {
                            Event::ConsensualEmoteRequest { source, target, id, emote }
                          } else {
                            eprintln!("Asset {} is not a consensual emote", emote);
                            Event::Ignore
                          }
                        }
                      }
                    }
                    .boxed(),
                  );
                }
              }
            }
            Vec::new()
          }
          Event::ConsensualEmoteRequest { source, target, id, emote } => {
            if let Some(target_info) = players.get(&target) {
              if let Err(_) = target_info.output.send(crate::destination::DestinationResponse::ConsensualEmoteRequest(source, id, emote)).await {
                eprintln!("Failed to send consensual_emote request");
              }
            }
            Vec::new()
          }
          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok }))) => {
            if let Some((requester_key, emote, _)) = consensual_emote_requests.remove(&(key.clone(), id)) {
              if ok {
                match (players.get(&key), players.get(&requester_key)) {
                  (Some(target), Some(requester)) => {
                    state.consensual_emote(&requester_key, &requester.principal, &key, &target.principal, emote).await;
                  }
                  _ => (),
                }
              }
            }
            Vec::new()
          }
          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::FollowRequest(target)))) => {
            if let Some(info) = players.get(&key) {
              let target = target.localize(&server_name);
              if info.principal.as_ref() != target.as_ref() {
                if let Some((target_key, target_info)) = players.iter().filter(|(_, ti)| ti.principal.as_ref() == target.as_ref()).next() {
                  let id = follow_id_generator;
                  follow_id_generator = follow_id_generator.wrapping_add(1);
                  if let Err(_) = target_info.output.send(crate::destination::DestinationResponse::FollowRequest(info.principal.clone(), id)).await {
                    eprintln!("Failed to send follow request");
                  } else {
                    follow_requests.insert((target_key.clone(), id), (key.clone(), chrono::Utc::now() + chrono::Duration::minutes(5)));
                  }
                }
              }
            }
            Vec::new()
          }
          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::FollowResponse(id, ok)))) => {
            if let Some((requester_key, _)) = follow_requests.remove(&(key.clone(), id)) {
              if ok {
                match (players.get(&key), players.get(&requester_key)) {
                  (Some(target), Some(requester)) => state.follow(&requester_key, &requester.principal, &key, &target.principal).await,
                  _ => Vec::new(),
                }
              } else {
                Vec::new()
              }
            } else {
              Vec::new()
            }
          }
          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::Messages { from, to }))) => {
            if let Some(info) = players.get(&key) {
              if let Err(_) =
                info.output.send(crate::destination::DestinationResponse::Messages { from, to, messages: state.get_messages(from, to) }).await
              {
                eprintln!("Failed to message {} from {}", &info.principal, &identifier);
              }
            }
            Vec::new()
          }
          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::SendMessage(body)))) => {
            if body.is_valid_at(&chrono::Utc::now()) {
              if let Some(info) = players.get(&key) {
                if let Some(timestamp) = state.send_message(Some(&key), &info.principal, &body).await {
                  let message = spadina_core::location::LocationMessage { body: body.convert_str(), sender: info.principal.clone(), timestamp };
                  for (_, info) in players.iter() {
                    if let Err(_) = info.output.send(crate::destination::DestinationResponse::MessagePosted(message.clone())).await {
                      eprintln!("Failed to message {} from {}", &info.principal, &identifier);
                    }
                  }
                }
              }
            }
            Vec::new()
          }

          Event::Player(key, Some(PlayerEvent::Request(crate::destination::DestinationRequest::Request(request)))) => match players.get(&key) {
            Some(info) => state.handle(&key, &info.principal, info.is_superuser, request).await,
            None => Vec::new(),
          },
        });
        for control in controls.drain(..) {
          match control {
            crate::destination::DestinationControl::Broadcast(message) => {
              for (_, info) in players.iter() {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::Response(message.clone())).await {
                  eprintln!("Failed to message {} from {}", &info.principal, &identifier);
                }
              }
            }
            crate::destination::DestinationControl::Move(player, target) => {
              if let Some((_, info)) = players.remove(&player) {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::Move(target)).await {
                  eprintln!("Failed to move {} from {}", &info.principal, &identifier);
                }
              }
            }
            crate::destination::DestinationControl::MoveTrain(player, owner, train) => {
              if let Some((_, info)) = players.remove(&player) {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::MoveTrain(owner, train)).await {
                  eprintln!("Failed to move {} from {}", &info.principal, &identifier);
                }
              }
            }
            crate::destination::DestinationControl::Quit => {
              for (_, info) in players.iter() {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::Move(None)).await {
                  eprintln!("Failed to eject {} from {}", &info.principal, &identifier);
                }
              }
              return;
            }
            crate::destination::DestinationControl::Response(player, message) => {
              if let Some(info) = players.get(&player) {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::Response(message)).await {
                  eprintln!("Failed to message {} from {}", &info.principal, &identifier);
                }
              }
            }
            crate::destination::DestinationControl::SendMessage(message) => {
              for (_, info) in players.iter() {
                if let Err(_) = info.output.send(crate::destination::DestinationResponse::MessagePosted(message.clone())).await {
                  eprintln!("Failed to message {} from {}", &info.principal, &identifier);
                }
              }
            }
          }
        }
      }
    });
    result
  }
  pub fn activity(&self) -> spadina_core::realm::RealmActivity {
    self.activity.get()
  }
  pub fn add<'a>(&'a self, player: crate::destination::PlayerHandle<D>) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + 'a>> {
    async {
      if let Err(tokio::sync::mpsc::error::SendError(ControlRequest::AddPlayer(player))) = self.tx.send(ControlRequest::AddPlayer(player)).await {
        eprintln!("Failed to send add player {} to {}", &player.principal, &self.identifier);
        std::mem::drop(
          player.tx.send(crate::destination::DestinationResponse::Location(spadina_core::location::LocationResponse::InternalError)).await,
        );
        true
      } else {
        false
      }
    }
    .boxed()
  }
  pub fn kill<'a>(&'a self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    async {
      if let Err(_) = self.tx.send(ControlRequest::Quit).await {
        eprintln!("Failed to send kill to {}", &self.identifier);
      }
    }
    .boxed()
  }
  pub fn delete<'a>(
    &'a self,
    requester: Option<super::SharedPlayerId>,
    output: tokio::sync::oneshot::Sender<spadina_core::UpdateResult>,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    async {
      if let Err(_) = self.tx.send(ControlRequest::Delete(requester, output)).await {
        eprintln!("Failed to send kill to {}", &self.identifier);
      }
    }
    .boxed()
  }
}
