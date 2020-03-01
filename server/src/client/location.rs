use futures::FutureExt;
use spadina_core::net::ToWebMessage;

pub(crate) enum Location {
  NoWhere,
  Realm {
    realm: Option<spadina_core::realm::RealmTarget<std::sync::Arc<str>>>,
    tx: tokio::sync::mpsc::Sender<crate::destination::DestinationRequest<spadina_core::realm::RealmRequest<crate::shstr::ShStr>>>,
    rx: tokio::sync::mpsc::Receiver<crate::destination::DestinationResponse<spadina_core::realm::RealmResponse<crate::shstr::ShStr>>>,
  },
  Hosting {
    tx: tokio::sync::mpsc::Sender<spadina_core::self_hosted::HostCommand<crate::shstr::ShStr>>,
    rx: tokio::sync::mpsc::Receiver<super::InternalClientRequest>,
  },
  Guest {
    host: crate::destination::SharedPlayerId,
    tx: tokio::sync::mpsc::Sender<crate::destination::DestinationRequest<spadina_core::self_hosted::GuestRequest<crate::shstr::ShStr>>>,
    rx: tokio::sync::mpsc::Receiver<crate::destination::DestinationResponse<spadina_core::self_hosted::GuestResponse<crate::shstr::ShStr>>>,
  },
}

impl Location {
  pub(crate) async fn new_guest(
    directory: &std::sync::Weak<crate::destination::Directory>,
    capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
    player_name: std::sync::Arc<str>,
    avatar: &crate::database::persisted::PersistedWatch<crate::avatar::Avatar>,
    host: spadina_core::player::PlayerIdentifier<impl AsRef<str> + Into<std::sync::Arc<str>>>,
    local_server: &std::sync::Arc<str>,
  ) -> (Location, spadina_core::location::LocationResponse<&'static str>) {
    match directory.upgrade() {
      Some(directory) => match host.localize(local_server.as_ref()) {
        spadina_core::player::PlayerIdentifier::Local(host) => {
          let host = host.into();
          match directory.hosting.get(&host) {
            Some(host) => {
              let (host_input, host_output) = tokio::sync::mpsc::channel(100);
              let (player_input, player_output) = tokio::sync::mpsc::channel(100);
              if host
                .add(crate::destination::PlayerHandle {
                  avatar: avatar.watch(),
                  capabilities,
                  is_superuser: false,
                  principal: spadina_core::player::PlayerIdentifier::Local(player_name),
                  tx: host_input,
                  rx: player_output,
                })
                .await
              {
                (
                  Location::Guest { host: spadina_core::player::PlayerIdentifier::Local(host.key().clone()), tx: player_input, rx: host_output },
                  spadina_core::location::LocationResponse::Resolving,
                )
              } else {
                (Location::NoWhere, spadina_core::location::LocationResponse::ResolutionFailed)
              }
            }
            None => (Location::NoWhere, spadina_core::location::LocationResponse::ResolutionFailed),
          }
        }
        spadina_core::player::PlayerIdentifier::Remote { server, player } => {
          let host = player.into();
          let avatar = avatar.watch();
          match directory
            .peer(server.as_ref(), |peer| {
              async move {
                let (host_input, host_output) = tokio::sync::mpsc::channel(100);
                let (player_input, player_output) = tokio::sync::mpsc::channel(100);
                peer
                  .send_host(
                    crate::destination::PlayerHandle {
                      avatar,
                      capabilities,
                      is_superuser: false,
                      principal: spadina_core::player::PlayerIdentifier::Local(player_name),
                      tx: host_input,
                      rx: player_output,
                    },
                    host.clone(),
                  )
                  .await;
                (
                  Location::Guest {
                    host: spadina_core::player::PlayerIdentifier::Remote { server: peer.name.clone(), player: host },
                    tx: player_input,
                    rx: host_output,
                  },
                  spadina_core::location::LocationResponse::Resolving,
                )
              }
              .boxed()
            })
            .await
          {
            None => (Location::NoWhere, spadina_core::location::LocationResponse::ResolutionFailed),
            Some(result) => result,
          }
        }
      },
      None => (Location::NoWhere, spadina_core::location::LocationResponse::InternalError),
    }
  }

  pub(crate) fn new_hosting(
    name: std::sync::Arc<str>,
    server_name: std::sync::Arc<str>,
    capabilities: std::collections::BTreeSet<&'static str>,
    acl: crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
    directory: &std::sync::Weak<crate::destination::Directory>,
  ) -> (Self, spadina_core::location::LocationResponse<&'static str>) {
    match directory.upgrade() {
      None => (Location::NoWhere, spadina_core::location::LocationResponse::InternalError),
      Some(directory) => {
        let (tx, commands) = tokio::sync::mpsc::channel(100);
        let (events, rx) = tokio::sync::mpsc::channel(100);
        directory.hosting.insert(
          name.clone(),
          crate::destination::manager::DestinationManager::new(
            spadina_core::player::PlayerIdentifier::Remote { server: server_name.clone(), player: name.clone() },
            server_name.clone(),
            std::sync::Arc::downgrade(&directory),
            tokio::spawn(std::future::ready(Ok(crate::destination::self_hosted::SelfHosted::new(
              acl,
              name,
              server_name,
              capabilities,
              commands,
              events,
            )))),
          ),
        );
        (Location::Hosting { tx, rx }, spadina_core::location::LocationResponse::Resolving)
      }
    }
  }

  pub async fn new_realm(
    directory: &std::sync::Weak<crate::destination::Directory>,
    capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
    is_superuser: bool,
    player: std::sync::Arc<str>,
    avatar: &crate::database::persisted::PersistedWatch<crate::avatar::Avatar>,
    realm: spadina_core::realm::RealmTarget<impl AsRef<str> + Into<std::sync::Arc<str>>>,
    debuted: bool,
    local_server: &str,
  ) -> (Self, spadina_core::location::LocationResponse<&'static str>) {
    enum ResolvedRealm {
      Local(crate::destination::LaunchTarget),
      Remote(std::sync::Arc<str>, spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>),
    }

    let realm = realm.localize(local_server).convert_str::<std::sync::Arc<str>>();

    let resolved_realm = match &realm {
      spadina_core::realm::RealmTarget::Home => ResolvedRealm::Local(crate::destination::LaunchTarget::ByTrain { owner: player.clone(), train: 0 }),
      spadina_core::realm::RealmTarget::LocalRealm { asset, owner } => {
        ResolvedRealm::Local(crate::destination::LaunchTarget::ByAsset { owner: owner.clone(), asset: asset.clone() })
      }
      spadina_core::realm::RealmTarget::PersonalRealm { asset } => {
        ResolvedRealm::Local(crate::destination::LaunchTarget::ByAsset { owner: player.clone(), asset: asset.clone() })
      }
      spadina_core::realm::RealmTarget::RemoteRealm { asset, owner, server } => {
        ResolvedRealm::Remote(server.clone(), spadina_core::realm::LocalRealmTarget { owner: owner.clone(), asset: asset.clone() })
      }
    };
    if debuted
      || match &resolved_realm {
        ResolvedRealm::Local(crate::destination::LaunchTarget::ByAsset { owner, .. })
        | ResolvedRealm::Local(crate::destination::LaunchTarget::ByTrain { owner, .. }) => owner == &player,
        ResolvedRealm::Remote(_, _) => false,
      }
    {
      match directory.upgrade() {
        None => (Location::NoWhere, spadina_core::location::LocationResponse::InternalError),
        Some(directory) => {
          let (realm_input, realm_output) = tokio::sync::mpsc::channel(100);
          let (player_input, player_output) = tokio::sync::mpsc::channel(100);
          let handle = crate::destination::PlayerHandle {
            avatar: avatar.watch(),
            capabilities,
            is_superuser,
            principal: spadina_core::player::PlayerIdentifier::Local(player),
            tx: player_input,
            rx: realm_output,
          };
          match resolved_realm {
            ResolvedRealm::Local(realm) => {
              if let Err(_) = directory.launch(crate::destination::LaunchRequest::Move(handle, realm)).await {
                return (Location::NoWhere, spadina_core::location::LocationResponse::InternalError);
              }
            }
            ResolvedRealm::Remote(peer_name, realm) => {
              if let None = directory.peer(peer_name.as_ref(), |peer| peer.send_realm(handle, realm).boxed()).await {
                return (Location::NoWhere, spadina_core::location::LocationResponse::InternalError);
              }
            }
          }
          (Location::Realm { tx: realm_input, rx: player_output, realm: Some(realm) }, spadina_core::location::LocationResponse::Resolving)
        }
      }
    } else {
      (Location::NoWhere, spadina_core::location::LocationResponse::PermissionDenied)
    }
  }
  pub async fn new_realm_train(
    directory: &std::sync::Weak<crate::destination::Directory>,
    capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
    is_superuser: bool,
    player: std::sync::Arc<str>,
    avatar: &crate::database::persisted::PersistedWatch<crate::avatar::Avatar>,
    owner: std::sync::Arc<str>,
    train: u16,
    debuted: bool,
  ) -> (Self, spadina_core::location::LocationResponse<&'static str>) {
    if debuted || &owner == &player {
      match directory.upgrade() {
        None => (Location::NoWhere, spadina_core::location::LocationResponse::InternalError),
        Some(directory) => {
          let (realm_input, realm_output) = tokio::sync::mpsc::channel(100);
          let (player_input, player_output) = tokio::sync::mpsc::channel(100);
          let handle = crate::destination::PlayerHandle {
            avatar: avatar.watch(),
            capabilities,
            is_superuser,
            principal: spadina_core::player::PlayerIdentifier::Local(player),
            tx: player_input,
            rx: realm_output,
          };
          if let Err(_) =
            directory.launch(crate::destination::LaunchRequest::Move(handle, crate::destination::LaunchTarget::ByTrain { owner, train })).await
          {
            return (Location::NoWhere, spadina_core::location::LocationResponse::InternalError);
          }

          (Location::Realm { tx: realm_input, rx: player_output, realm: None }, spadina_core::location::LocationResponse::Resolving)
        }
      }
    } else {
      (Location::NoWhere, spadina_core::location::LocationResponse::PermissionDenied)
    }
  }
  pub fn get_state(&self) -> spadina_core::player::PlayerLocationState<std::sync::Arc<str>> {
    match self {
      Location::NoWhere => spadina_core::player::PlayerLocationState::InTransit,
      Location::Realm { realm: Some(realm), .. } => spadina_core::player::PlayerLocationState::Realm { realm: realm.clone() },
      Location::Realm { realm: None, .. } => spadina_core::player::PlayerLocationState::Online,
      Location::Hosting { .. } => spadina_core::player::PlayerLocationState::Hosting,
      Location::Guest { host, .. } => spadina_core::player::PlayerLocationState::Guest { host: host.clone() },
    }
  }
  pub async fn send_realm(&self, request: spadina_core::realm::RealmRequest<crate::shstr::ShStr>) {
    if let Location::Realm { tx, .. } = self {
      if let Err(_) = tx.send(crate::destination::DestinationRequest::Request(request)).await {
        eprintln!("Failed to send request to realm");
      }
    }
  }

  pub async fn message_send(&self, body: spadina_core::communication::MessageBody<crate::shstr::ShStr>) {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::SendMessage(body)).await {
          eprintln!("Failed to send chat message request to realm");
        }
      }
      Location::Hosting { tx, .. } => {
        if let Err(_) = tx.send(spadina_core::self_hosted::HostCommand::SendMessage { body }).await {
          eprintln!("Failed to send chat message request to guests");
        }
      }
      Location::Guest { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::SendMessage(body)).await {
          eprintln!("Failed to send chat message request to host");
        }
      }
    }
  }

  pub async fn message_get(&self, from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>) {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::Messages { from, to }).await {
          eprintln!("Failed to send chat download to realm");
        }
      }
      Location::Hosting { .. } => (),
      Location::Guest { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::Messages { from, to }).await {
          eprintln!("Failed to send chat download to host");
        }
      }
    }
  }

  pub async fn consensual_emote_request(&self, emote: String, player: spadina_core::player::PlayerIdentifier<String>) -> () {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) =
          tx.send(crate::destination::DestinationRequest::ConsensualEmoteRequest { emote: emote.into(), player: player.convert_str() }).await
        {
          eprintln!("Failed to send consensual emote request to realm");
        }
      }
      Location::Hosting { .. } => (),
      Location::Guest { tx, .. } => {
        if let Err(_) =
          tx.send(crate::destination::DestinationRequest::ConsensualEmoteRequest { emote: emote.into(), player: player.convert_str() }).await
        {
          eprintln!("Failed to send consensual emote request to host");
        }
      }
    }
  }

  pub(crate) async fn consensual_emote_response(&self, id: i32, ok: bool) -> () {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok }).await {
          eprintln!("Failed to send consensual emote response to host");
        }
      }
      Location::Hosting { .. } => (),
      Location::Guest { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok }).await {
          eprintln!("Failed to send consensual emote response to host");
        }
      }
    }
  }

  pub async fn follow_request(&self, player: spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>) -> () {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::FollowRequest(player)).await {
          eprintln!("Failed to send follow request to realm");
        }
      }
      Location::Hosting { .. } => (),
      Location::Guest { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::FollowRequest(player)).await {
          eprintln!("Failed to send follow request to host");
        }
      }
    }
  }

  pub(crate) async fn follow_response(&self, id: i32, ok: bool) -> () {
    match self {
      Location::NoWhere => (),
      Location::Realm { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::FollowResponse(id, ok)).await {
          eprintln!("Failed to send follow response to realm");
        }
      }
      Location::Hosting { .. } => (),
      Location::Guest { tx, .. } => {
        if let Err(_) = tx.send(crate::destination::DestinationRequest::FollowResponse(id, ok)).await {
          eprintln!("Failed to send follow response to host");
        }
      }
    }
  }

  pub(crate) async fn send_guest_request(&mut self, request: spadina_core::self_hosted::GuestRequest<crate::shstr::ShStr>) -> () {
    let reset =
      if let Location::Guest { tx, .. } = self { tx.send(crate::destination::DestinationRequest::Request(request)).await.is_err() } else { false };
    if reset {
      *self = Location::NoWhere;
    }
  }

  pub(crate) async fn send_host_command(&mut self, request: spadina_core::self_hosted::HostCommand<crate::shstr::ShStr>) -> () {
    let reset = if let Location::Hosting { tx, .. } = self { tx.send(request).await.is_err() } else { false };
    if reset {
      *self = Location::NoWhere;
    }
  }
}
impl futures::Stream for Location {
  type Item = super::InternalClientRequest;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    let location = self.get_mut();
    let (result, reset) = match location {
      Location::NoWhere => (std::task::Poll::Pending, false),
      Location::Realm { rx, .. } => match rx.poll_recv(cx) {
        std::task::Poll::Pending => (std::task::Poll::Pending, false),
        std::task::Poll::Ready(None) => (std::task::Poll::Ready(Some(super::InternalClientRequest::Redirect(None))), true),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::ConsensualEmoteRequest(sender, id, emote))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::ConsensualEmoteRequest { id, emote, player: sender }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::FollowRequest(player, id))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::FollowRequest { id, player }.as_wsm()))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Location(location))) => {
          let reset = location.is_released();
          (
            std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::LocationChange { location }.as_wsm()))),
            reset,
          )
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::MessagePosted(spadina_core::location::LocationMessage {
          sender,
          body,
          timestamp,
        }))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::LocationMessagePosted { sender, body, timestamp }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Messages { from, to, messages })) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::LocationMessages { messages, from, to }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Move(target))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::Redirect(target))), true)
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::MoveTrain(owner, train))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::RedirectTrain(owner, train))), true)
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Response(response))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::InRealm { response }.as_wsm()))), false)
        }
      },
      Location::Hosting { rx, .. } => match rx.poll_recv(cx) {
        std::task::Poll::Pending => (std::task::Poll::Pending, false),
        std::task::Poll::Ready(None) => (std::task::Poll::Ready(Some(super::InternalClientRequest::Redirect(None))), true),
        std::task::Poll::Ready(Some(message)) => (std::task::Poll::Ready(Some(message)), true),
      },
      Location::Guest { rx, .. } => match rx.poll_recv(cx) {
        std::task::Poll::Pending => (std::task::Poll::Pending, false),
        std::task::Poll::Ready(None) => (std::task::Poll::Ready(Some(super::InternalClientRequest::Redirect(None))), true),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::ConsensualEmoteRequest(sender, id, emote))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::ConsensualEmoteRequest { id, emote, player: sender }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::FollowRequest(player, id))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::FollowRequest { id, player }.as_wsm()))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Location(location))) => {
          let reset = location.is_released();
          (
            std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::LocationChange { location }.as_wsm()))),
            reset,
          )
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::MessagePosted(spadina_core::location::LocationMessage {
          sender,
          body,
          timestamp,
        }))) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::LocationMessagePosted { sender, body, timestamp }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Messages { from, to, messages })) => (
          std::task::Poll::Ready(Some(super::InternalClientRequest::Message(
            spadina_core::ClientResponse::LocationMessages { messages, from, to }.as_wsm(),
          ))),
          true,
        ),
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Move(target))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::Redirect(target))), true)
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::MoveTrain(owner, train))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::RedirectTrain(owner, train))), true)
        }
        std::task::Poll::Ready(Some(crate::destination::DestinationResponse::Response(response))) => {
          (std::task::Poll::Ready(Some(super::InternalClientRequest::Message(spadina_core::ClientResponse::FromHost { response }.as_wsm()))), false)
        }
      },
    };
    if reset {
      *location = Location::NoWhere;
    }
    result
  }
}
