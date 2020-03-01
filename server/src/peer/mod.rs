pub(crate) mod message;
pub(crate) mod net;
pub(crate) mod stream;
use futures::{FutureExt, StreamExt};
use spadina_core::net::ToWebMessage;

#[derive(Debug)]
pub(crate) enum AvailableRealmSink {
  Watch(
    std::sync::Arc<
      crate::prometheus_locks::mutex::PrometheusLabelledMutex<
        'static,
        tokio::sync::watch::Sender<Vec<spadina_core::realm::RealmDirectoryEntry<std::sync::Arc<str>>>>,
        (),
      >,
    >,
  ),
  OneShot(tokio::sync::oneshot::Sender<Vec<spadina_core::realm::RealmDirectoryEntry<std::sync::Arc<str>>>>),
}
#[derive(Debug)]
pub(crate) enum PeerRequest {
  CheckOnline {
    requester: std::sync::Arc<str>,
    target: std::sync::Arc<str>,
    output: tokio::sync::oneshot::Sender<spadina_core::player::PlayerLocationState<crate::shstr::ShStr>>,
  },
  CheckRealms(message::PeerRealmSource<std::sync::Arc<str>>, AvailableRealmSink),
  Connect(tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>, std::collections::BTreeSet<&'static str>),
  DirectMessage {
    sender: std::sync::Arc<str>,
    db_id: i32,
    recipient: std::sync::Arc<str>,
    body: spadina_core::communication::MessageBody<String>,
    output: tokio::sync::oneshot::Sender<spadina_core::communication::DirectMessageStatus>,
  },
  InitiateConnection,
  Ignore,
  Message(tokio_tungstenite::tungstenite::Message),
  SendPlayer {
    player: crate::destination::PlayerHandle<crate::realm::Realm>,
    realm: spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>,
  },
  SendPlayerTrain {
    player: std::sync::Arc<str>,
    owner: std::sync::Arc<str>,
    train: u16,
  },
  SendPlayerToHost {
    player: crate::destination::PlayerHandle<crate::destination::self_hosted::SelfHosted>,
    host: std::sync::Arc<str>,
  },
  Quit,
}
struct OutstandingMessage {
  db_id: i32,
  recipient: std::sync::Arc<str>,
  body: spadina_core::communication::MessageBody<String>,
  output: tokio::sync::oneshot::Sender<spadina_core::communication::DirectMessageStatus>,
}

#[derive(Clone)]
pub(crate) struct Peer {
  pub(crate) name: std::sync::Arc<str>,
  output: tokio::sync::mpsc::Sender<PeerRequest>,
}

impl Peer {
  pub(crate) async fn new(
    peer_name: std::sync::Arc<str>,
    directory: std::sync::Weak<crate::destination::Directory>,
    database: std::sync::Arc<crate::database::Database>,
    authnz: std::sync::Arc<crate::access::AuthNZ>,
  ) -> Self {
    let (output, mut internal_input) = tokio::sync::mpsc::channel(500);
    let result = Peer { name: peer_name.clone(), output };
    tokio::spawn(async move {
      enum Message {
        Internal(PeerRequest),
        External(message::PeerMessage<crate::shstr::ShStr>),
      }
      enum FromPeer {
        Realm(
          tokio::sync::mpsc::Sender<crate::destination::DestinationRequest<spadina_core::realm::RealmRequest<crate::shstr::ShStr>>>,
          tokio::sync::watch::Sender<spadina_core::avatar::Avatar>,
          std::sync::Arc<std::collections::BTreeSet<&'static str>>,
        ),
        Hosted(
          tokio::sync::mpsc::Sender<crate::destination::DestinationRequest<spadina_core::self_hosted::GuestRequest<crate::shstr::ShStr>>>,
          tokio::sync::watch::Sender<spadina_core::avatar::Avatar>,
          std::sync::Arc<std::collections::BTreeSet<&'static str>>,
        ),
      }
      enum OnPeer {
        Realm(tokio::sync::mpsc::Sender<crate::destination::DestinationResponse<spadina_core::realm::RealmResponse<crate::shstr::ShStr>>>),
        Guest(tokio::sync::mpsc::Sender<crate::destination::DestinationResponse<spadina_core::self_hosted::GuestResponse<crate::shstr::ShStr>>>),
      }
      let mut capabilities = std::sync::Arc::new(std::collections::BTreeSet::new());
      let mut connection = net::PeerConnection::Dead(chrono::Utc::now(), Vec::new(), peer_name.clone());
      let mut death = authnz.give_me_death();
      let mut next_id = 0;
      let mut online_status_response = std::collections::HashMap::new();
      let mut oustanding_messages = std::collections::HashMap::new();
      let mut players_on_peer = std::collections::HashMap::<crate::shstr::ShStr, OnPeer>::new();
      let mut players_from_peer = std::collections::HashMap::<crate::shstr::ShStr, FromPeer>::new();
      let mut players_in_realm = crate::map::StreamsUnorderedMap::<crate::shstr::ShStr, stream::RealmStream>::new();
      let mut players_in_guest = crate::map::StreamsUnorderedMap::<crate::shstr::ShStr, stream::GuestStream>::new();
      let mut player_proxies = crate::map::StreamsUnorderedMap::<crate::shstr::ShStr, stream::PlayerProxy>::new();
      let mut side_tasks = futures::stream::FuturesUnordered::new();
      let mut waiting_realm_lists = std::collections::BTreeMap::new();
      loop {
        let message = tokio::select! {
          r = connection.next() => r.map(Message::External).unwrap_or(Message::Internal(PeerRequest::Quit)),
          _ = death.recv() => Message::Internal(PeerRequest::Quit),
          r = internal_input.recv() => Message::Internal(r.unwrap_or(PeerRequest::Quit)),
          r = side_tasks.next(), if !side_tasks.is_empty() => match r { Some(Ok(r)) => Message::Internal(r), _ => Message::Internal(PeerRequest::Ignore) },
          Some((_, r)) = players_in_realm.next() => Message::Internal(r.unwrap_or(PeerRequest::Ignore)),
          Some((_, r)) = players_in_guest.next() => Message::Internal(r.unwrap_or(PeerRequest::Ignore)),
          Some((_, r)) = player_proxies.next() => Message::Internal(r.unwrap_or(PeerRequest::Ignore)),

        };
        match message {
          Message::Internal(PeerRequest::Connect(c, caps)) => {
            connection.establish(c, peer_name.clone()).await;
            capabilities = std::sync::Arc::new(caps);
          }
          Message::Internal(PeerRequest::Ignore) => (),
          Message::Internal(PeerRequest::InitiateConnection) => match peer_name.parse::<http::uri::Authority>() {
            Err(e) => {
              println!("Bad peer server name {}: {}", peer_name, e);
            }
            Ok(authority) => {
              match hyper::Uri::builder().scheme(http::uri::Scheme::HTTPS).path_and_query("/api/server/v1").authority(authority).build() {
                Err(e) => {
                  println!("Bad URL construction for server name {}: {}", peer_name, e);
                }
                Ok(uri) => match jsonwebtoken::encode(
                  &jsonwebtoken::Header::default(),
                  &net::PeerClaim { exp: crate::http::jwt::expiry_time(3600), name: peer_name.as_ref() },
                  &authnz.jwt_encoding_key,
                ) {
                  Ok(token) => {
                    let request =
                      serde_json::to_vec(&net::PeerHttpRequestBody { token: token.as_str(), server: authnz.server_name.as_ref() }).unwrap();
                    let connector = hyper_tls::HttpsConnector::new();
                    let client = hyper::client::Client::builder().build::<_, hyper::Body>(connector);

                    match client
                      .request(
                        spadina_core::capabilities::add_header(hyper::Request::post(&uri).version(http::Version::HTTP_11))
                          .body(hyper::Body::from(request.clone()))
                          .unwrap(),
                      )
                      .await
                    {
                      Err(e) => {
                        eprintln!("Failed contact to {}: {}", &peer_name, e)
                      }
                      Ok(response) => {
                        if response.status() != http::StatusCode::OK {
                          eprintln!("Failed to connect to peer server {}: {}", &peer_name, response.status());
                        }
                      }
                    }
                  }
                  Err(e) => {
                    crate::metrics::BAD_JWT.get_or_create(&()).inc();
                    eprintln!("Error generation JWT: {}", e);
                  }
                },
              }
            }
          },
          Message::Internal(PeerRequest::CheckOnline { requester, target, output }) => {
            online_status_response.insert((requester.clone(), target.clone()), output);
            connection.send(message::PeerMessage::OnlineStatusRequest { requester, target }.as_wsm()).await;
          }
          Message::Internal(PeerRequest::CheckRealms(source, output)) => {
            let id = next_id;
            next_id += 1;
            waiting_realm_lists.insert(id, output);
            connection.send(message::PeerMessage::RealmsList { id, source }.as_wsm()).await;
          }
          Message::Internal(PeerRequest::DirectMessage { sender, db_id, recipient, body, output }) => {
            let id = next_id;
            next_id += 1;
            connection
              .send(message::PeerMessage::DirectMessage { id, sender: sender.as_ref(), recipient: recipient.as_ref(), body: body.as_ref() }.as_wsm())
              .await;
            oustanding_messages.insert(id, OutstandingMessage { db_id, recipient, body, output });
          }
          Message::Internal(PeerRequest::Quit) => break,
          Message::Internal(PeerRequest::SendPlayer { player, realm }) => {
            if let spadina_core::player::PlayerIdentifier::Local(name) = player.principal {
              let avatar = player.avatar.borrow().clone();
              connection
                .send(
                  message::PeerMessage::VisitorSend {
                    capabilities: capabilities.iter().filter(|&&cap| player.capabilities.contains(cap)).copied().collect(),
                    player: name.as_ref(),
                    target: message::VisitorTarget::Realm { owner: realm.owner, asset: realm.asset },
                    avatar,
                  }
                  .as_wsm(),
                )
                .await;
              players_on_peer.insert(name.clone().into(), OnPeer::Realm(player.tx));
              player_proxies.insert(
                name.clone().into(),
                stream::PlayerProxy::Realm {
                  player: name,
                  realm: player.rx,
                  avatar: tokio_stream::wrappers::WatchStream::new(player.avatar),
                  local_server: authnz.server_name.clone(),
                },
              );
            }
          }
          Message::Internal(PeerRequest::SendPlayerToHost { player, host }) => {
            if let spadina_core::player::PlayerIdentifier::Local(name) = player.principal {
              let avatar = player.avatar.borrow().clone();
              connection
                .send(
                  message::PeerMessage::VisitorSend {
                    capabilities: capabilities.iter().filter(|&&cap| player.capabilities.contains(cap)).copied().collect(),
                    player: name.as_ref(),
                    target: message::VisitorTarget::Host { host },
                    avatar,
                  }
                  .as_wsm(),
                )
                .await;
              players_on_peer.insert(name.clone().into(), OnPeer::Guest(player.tx));
              player_proxies.insert(
                name.clone().into(),
                stream::PlayerProxy::Guest {
                  player: name,
                  realm: player.rx,
                  avatar: tokio_stream::wrappers::WatchStream::new(player.avatar),
                  local_server: authnz.server_name.clone(),
                },
              );
            }
          }
          Message::Internal(PeerRequest::SendPlayerTrain { player, owner, train }) => {
            match (directory.upgrade(), players_from_peer.remove(&crate::shstr::ShStr::Shared(player.clone()))) {
              (Some(directory), Some(on_peer)) => {
                let (tx, player_input) = tokio::sync::mpsc::channel(100);
                let (player_output, rx) = tokio::sync::mpsc::channel(100);
                let (avatar_tx, capabilities) = match on_peer {
                  FromPeer::Realm(_, avatar_tx, capabilities) => (avatar_tx, capabilities),
                  FromPeer::Hosted(_, avatar_tx, capabilities) => (avatar_tx, capabilities),
                };
                match directory
                  .launch(crate::destination::LaunchRequest::Move(
                    crate::destination::PlayerHandle {
                      avatar: avatar_tx.subscribe(),
                      capabilities: capabilities.clone(),
                      is_superuser: false,
                      principal: spadina_core::player::PlayerIdentifier::Remote { player: player.clone(), server: peer_name.clone() },
                      tx,
                      rx,
                    },
                    crate::destination::LaunchTarget::ByTrain { owner, train },
                  ))
                  .await
                {
                  Ok(()) => {
                    players_in_realm.insert(player.clone().into(), stream::RealmStream { player: player.clone(), rx: player_input });
                    players_from_peer.insert(crate::shstr::ShStr::Shared(player), FromPeer::Realm(player_output, avatar_tx, capabilities));
                  }
                  Err(()) => connection.send(message::PeerMessage::VisitorRelease { player: player.as_ref(), target: None }.as_wsm()).await,
                }
              }
              _ => connection.send(message::PeerMessage::VisitorRelease { player: player.as_ref(), target: None }.as_wsm()).await,
            }
          }
          Message::Internal(PeerRequest::Message(message)) => {
            connection.send(message).await;
          }
          Message::External(message) => {
            use message::PeerMessage;
            use message::VisitorTarget;
            match message {
              PeerMessage::AssetsPull { assets } => match directory.upgrade() {
                None => (),
                Some(directory) => directory.asset_manager().pull_assets(peer_name.clone(), assets).await,
              },
              PeerMessage::AssetsPush { assets } => match directory.upgrade() {
                None => (),
                Some(directory) => directory.asset_manager().push_assets(assets).await,
              },
              PeerMessage::AvatarSet { avatar, player } => {
                if let Some(state) = players_from_peer.get(&player) {
                  if let Err(_) = match state {
                    FromPeer::Realm(_, avatar_tx, _) => avatar_tx,
                    FromPeer::Hosted(_, avatar_tx, _) => avatar_tx,
                  }
                  .send(avatar)
                  {
                    eprintln!("Failed to update avatar for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::ConsensualEmoteRequestInitiate { player, emote, recipient } => {
                if let Some(state) = players_from_peer.get(&player) {
                  let recipient = recipient.globalize(&peer_name).localize(&authnz.server_name);
                  if match state {
                    FromPeer::Realm(tx, _, _) => {
                      tx.send(crate::destination::DestinationRequest::ConsensualEmoteRequest { emote, player: recipient }).await.is_err()
                    }
                    FromPeer::Hosted(tx, _, _) => {
                      tx.send(crate::destination::DestinationRequest::ConsensualEmoteRequest { emote, player: recipient }).await.is_err()
                    }
                  } {
                    eprintln!("Failed to send emote initiation request for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::ConsensualEmoteRequestFromLocation { id, player, emote, sender } => {
                if let Some(state) = players_on_peer.get(&player) {
                  if match state {
                    OnPeer::Realm(tx) => {
                      tx.send(crate::destination::DestinationResponse::ConsensualEmoteRequest(sender.convert_str(), id, emote.into())).await.is_err()
                    }
                    OnPeer::Guest(tx) => {
                      tx.send(crate::destination::DestinationResponse::ConsensualEmoteRequest(sender.convert_str(), id, emote.into())).await.is_err()
                    }
                  } {
                    eprintln!("Failed to send emote request for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::ConsensualEmoteResponse { player, id, ok } => {
                if let Some(state) = players_from_peer.get(&player) {
                  if match state {
                    FromPeer::Realm(tx, _, _) => tx.send(crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok }).await.is_err(),
                    FromPeer::Hosted(tx, _, _) => tx.send(crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok }).await.is_err(),
                  } {
                    eprintln!("Failed to send emote response for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::DirectMessage { id, sender, recipient, body } => {
                let recipient = recipient.to_arc();
                let status = if body.is_valid_at(&chrono::Utc::now())
                  && authnz
                    .check_access("peer_dm", &spadina_core::player::PlayerIdentifier::Remote { server: peer_name.as_ref(), player: sender.as_ref() })
                    .await
                {
                  match directory.upgrade() {
                    None => Some(spadina_core::communication::DirectMessageStatus::InternalError),
                    Some(directory) => match directory.players.get(&recipient) {
                      Some(recipient_player) => {
                        let sender = sender.to_arc();
                        let output = recipient_player
                          .send_dm(spadina_core::player::PlayerIdentifier::Remote { server: peer_name.clone(), player: sender }, body)
                          .await;
                        side_tasks.push(tokio::spawn(async move {
                          PeerRequest::Message(
                            PeerMessage::<String>::DirectMessageResponse {
                              id,
                              status: match output.await {
                                Ok(v) => v,
                                Err(_) => spadina_core::communication::DirectMessageStatus::InternalError,
                              },
                            }
                            .as_wsm(),
                          )
                        }));
                        None
                      }
                      None => match database.player_load(&recipient, false) {
                        Ok(Some(recipient_info)) => {
                          if recipient_info.message_acl.check(
                            &spadina_core::player::PlayerIdentifier::Remote { server: peer_name.as_ref(), player: sender.as_ref() },
                            &authnz.server_name,
                          ) == spadina_core::access::SimpleAccess::Allow
                          {
                            match database.remote_direct_message_write(recipient_info.db_id, sender.as_ref(), &peer_name, &body, None) {
                              Ok(ts) => Some(spadina_core::communication::DirectMessageStatus::Delivered(ts)),
                              Err(e) => {
                                eprintln!("Failed to deliver direct message from {} on {} to {}: {}", &sender, &peer_name, &recipient, e);
                                Some(spadina_core::communication::DirectMessageStatus::InternalError)
                              }
                            }
                          } else {
                            Some(spadina_core::communication::DirectMessageStatus::Forbidden)
                          }
                        }
                        Ok(None) => Some(spadina_core::communication::DirectMessageStatus::UnknownRecipient),
                        Err(e) => {
                          eprintln!("Failed to deliver direct message from {} on {} to {}: {}", &sender, &peer_name, &recipient, e);
                          Some(spadina_core::communication::DirectMessageStatus::InternalError)
                        }
                      },
                    },
                  }
                } else {
                  Some(spadina_core::communication::DirectMessageStatus::Forbidden)
                };
                if let Some(status) = status {
                  connection.send(PeerMessage::<String>::DirectMessageResponse { id, status }.as_wsm()).await
                }
              }
              PeerMessage::DirectMessageResponse { id, status } => {
                if let Some(OutstandingMessage { db_id, recipient, body, output }) = oustanding_messages.remove(&id) {
                  if let spadina_core::communication::DirectMessageStatus::Delivered(ts) = &status {
                    if let Err(e) = database.remote_direct_message_write(db_id, &recipient, &peer_name, &body, Some(*ts)) {
                      eprintln!("Failed to store direct message response: {}", e);
                    }
                  }
                  std::mem::drop(output.send(status));
                }
              }
              PeerMessage::FollowRequestInitiate { player, target } => {
                if let Some(state) = players_from_peer.get(&player) {
                  let target = target.globalize(&peer_name).localize(&authnz.server_name);
                  if match state {
                    FromPeer::Realm(tx, _, _) => tx.send(crate::destination::DestinationRequest::FollowRequest(target)).await.is_err(),
                    FromPeer::Hosted(tx, _, _) => tx.send(crate::destination::DestinationRequest::FollowRequest(target)).await.is_err(),
                  } {
                    eprintln!("Failed to send follow initiation request for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::FollowRequestFromLocation { id, player, source } => {
                if let Some(state) = players_on_peer.get(&player) {
                  if match state {
                    OnPeer::Realm(tx) => tx.send(crate::destination::DestinationResponse::FollowRequest(source.convert_str(), id)).await.is_err(),
                    OnPeer::Guest(tx) => tx.send(crate::destination::DestinationResponse::FollowRequest(source.convert_str(), id)).await.is_err(),
                  } {
                    eprintln!("Failed to send follow request for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::FollowResponse { player, id, ok } => {
                if let Some(state) = players_from_peer.get(&player) {
                  if match state {
                    FromPeer::Realm(tx, _, _) => tx.send(crate::destination::DestinationRequest::FollowResponse(id, ok)).await.is_err(),
                    FromPeer::Hosted(tx, _, _) => tx.send(crate::destination::DestinationRequest::FollowResponse(id, ok)).await.is_err(),
                  } {
                    eprintln!("Failed to send follow response for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::LocationMessagePosted { player, message } => {
                if let Some(state) = players_on_peer.get(&player) {
                  if match state {
                    OnPeer::Realm(tx) => tx.send(crate::destination::DestinationResponse::MessagePosted(message.convert_str())).await.is_err(),
                    OnPeer::Guest(tx) => tx.send(crate::destination::DestinationResponse::MessagePosted(message.convert_str())).await.is_err(),
                  } {
                    eprintln!("Failed to post message for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::LocationMessageSend { player, body } => {
                if let Some(state) = players_from_peer.get(&player) {
                  if match state {
                    FromPeer::Realm(tx, _, _) => tx.send(crate::destination::DestinationRequest::SendMessage(body)).await.is_err(),
                    FromPeer::Hosted(tx, _, _) => tx.send(crate::destination::DestinationRequest::SendMessage(body)).await.is_err(),
                  } {
                    eprintln!("Failed to send message for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::LocationMessages { player, messages, from, to } => {
                if let Some(state) = players_on_peer.get(&player) {
                  let messages = messages.into_iter().map(|m| m.convert_str()).collect();
                  if match state {
                    OnPeer::Realm(tx) => tx.send(crate::destination::DestinationResponse::Messages { messages, from, to }).await.is_err(),
                    OnPeer::Guest(tx) => tx.send(crate::destination::DestinationResponse::Messages { messages, from, to }).await.is_err(),
                  } {
                    eprintln!("Failed to send messages for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::LocationMessagesGet { player, from, to } => {
                if let Some(state) = players_from_peer.get(&player) {
                  if match state {
                    FromPeer::Realm(tx, _, _) => tx.send(crate::destination::DestinationRequest::Messages { from, to }).await.is_err(),
                    FromPeer::Hosted(tx, _, _) => tx.send(crate::destination::DestinationRequest::Messages { from, to }).await.is_err(),
                  } {
                    eprintln!("Failed to request messages for {} from {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::OnlineStatusRequest { requester, target } => {
                if authnz
                  .check_access(
                    "peer_online_status",
                    &spadina_core::player::PlayerIdentifier::Remote { server: peer_name.as_ref(), player: requester.as_ref() },
                  )
                  .await
                {
                  match directory.upgrade() {
                    None => {
                      connection
                        .send(
                          PeerMessage::OnlineStatusResponse { requester, target, state: spadina_core::player::PlayerLocationState::Unknown }.as_wsm(),
                        )
                        .await;
                    }
                    Some(directory) => match directory.players.get(target.as_ref()) {
                      Some(player) => {
                        let requester = requester.to_arc();
                        let rx = player
                          .value()
                          .check_online_status(spadina_core::player::PlayerIdentifier::Remote {
                            server: peer_name.clone(),
                            player: requester.clone(),
                          })
                          .await;
                        side_tasks.push(tokio::spawn(async move {
                          let state = match rx.await {
                            Ok(s) => s,
                            Err(_) => spadina_core::player::PlayerLocationState::Unknown,
                          };
                          PeerRequest::Message(
                            PeerMessage::OnlineStatusResponse { requester: requester.as_ref(), target: target.as_ref(), state: state.as_ref() }
                              .as_wsm(),
                          )
                        }))
                      }
                      None => {
                        let state = match database.player_load(target.as_str(), false) {
                          Ok(None) => spadina_core::player::PlayerLocationState::Invalid,
                          Ok(Some(info)) => match info.location_acl.check(
                            &spadina_core::player::PlayerIdentifier::Remote { server: peer_name.as_ref(), player: requester.as_ref() },
                            &authnz.server_name,
                          ) {
                            spadina_core::access::LocationAccess::Location | spadina_core::access::LocationAccess::OnlineOnly => {
                              spadina_core::player::PlayerLocationState::Offline
                            }
                            spadina_core::access::LocationAccess::Deny => spadina_core::player::PlayerLocationState::Unknown,
                          },
                          Err(e) => {
                            eprintln!("Failed to load player information for {} when requested by {}: {}", &target, &peer_name, e);
                            spadina_core::player::PlayerLocationState::Unknown
                          }
                        };
                        connection.send(PeerMessage::OnlineStatusResponse { requester, target, state }.as_wsm()).await;
                      }
                    },
                  }
                } else {
                  connection
                    .send(PeerMessage::OnlineStatusResponse { requester, target, state: spadina_core::player::PlayerLocationState::Unknown }.as_wsm())
                    .await;
                }
              }
              PeerMessage::OnlineStatusResponse { requester, target, state } => {
                let requester = requester.to_arc();
                let target = target.to_arc();
                if let Some(output) = online_status_response.remove(&(requester.clone(), target.clone())) {
                  if let Err(_) = output.send(state) {
                    eprintln!("Failed to send online status response for {} on {} from {}", &target, &peer_name, &requester);
                  }
                }
              }
              PeerMessage::LocationChange { player, response } => {
                let remove_player = if let Some(state) = players_on_peer.get(&player) {
                  let is_released = response.is_released();
                  let is_err = match state {
                    OnPeer::Realm(realm) => realm.send(crate::destination::DestinationResponse::Location(response)).await.is_err(),
                    OnPeer::Guest(host) => host.send(crate::destination::DestinationResponse::Location(response)).await.is_err(),
                  };
                  if is_err && !is_released {
                    connection.send(PeerMessage::VisitorYank { player: player.as_str() }.as_wsm()).await;
                  }
                  is_err || is_released
                } else {
                  false
                };
                if remove_player {
                  players_on_peer.remove(&player);
                }
              }
              PeerMessage::RealmRequest { player, request } => {
                if let Some(FromPeer::Realm(output, _, _)) = players_from_peer.get(&player) {
                  let is_err = output.send(crate::destination::DestinationRequest::Request(request)).await.is_err();
                  if is_err {
                    eprintln!("Failed to send request from {} on {}", &player, &peer_name);
                  }
                } else {
                  connection.send(PeerMessage::VisitorYank { player }.as_wsm()).await;
                }
              }
              PeerMessage::RealmResponse { player, response } => {
                if let Some(OnPeer::Realm(output)) = players_on_peer.get(&player) {
                  let is_err = output.send(crate::destination::DestinationResponse::Response(response)).await.is_err();
                  if is_err {
                    eprintln!("Failed to send request from {} visiting {}", &player, &peer_name);
                  }
                }
              }
              PeerMessage::RealmsAvailable { id, available } => {
                if let Some(output) = waiting_realm_lists.remove(&id) {
                  let available = available.into_iter().map(|r| r.convert_str()).collect();
                  match output {
                    AvailableRealmSink::Watch(output) => output.lock("bookmarks_update_from_peer").await.send_modify(|r| r.extend(available)),
                    AvailableRealmSink::OneShot(output) => std::mem::drop(output.send(available)),
                  }
                }
              }
              PeerMessage::RealmsList { id, source } => {
                let mut available = database.realm_list(
                  &authnz.server_name,
                  false,
                  match source {
                    message::PeerRealmSource::InDirectory => crate::database::realm_scope::RealmListScope::InDirectory,
                    message::PeerRealmSource::Specific { realms } => crate::database::realm_scope::RealmListScope::Any(
                      realms
                        .into_iter()
                        .map(|spadina_core::realm::LocalRealmTarget { owner, asset }| {
                          crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::NamedAsset { owner, asset })
                        })
                        .collect(),
                    ),
                  },
                );
                match directory.upgrade() {
                  Some(directory) => side_tasks.push(tokio::spawn(async move {
                    {
                      for realm in &mut available {
                        realm.activity = directory
                          .realm_activity(spadina_core::realm::LocalRealmTarget { asset: realm.asset.clone(), owner: realm.owner.clone() })
                          .await;
                      }
                    }
                    PeerRequest::Message(PeerMessage::RealmsAvailable { id, available }.as_wsm())
                  })),
                  None => connection.send(PeerMessage::RealmsAvailable { id, available }.as_wsm()).await,
                }
              }

              PeerMessage::VisitorRelease { player, target } => {
                if let Some(location) = players_on_peer.remove(&player) {
                  let target = target.map(|t| t.convert_str());
                  let is_err = match location {
                    OnPeer::Guest(output) => output.send(crate::destination::DestinationResponse::Move(target)).await.is_err(),
                    OnPeer::Realm(output) => output.send(crate::destination::DestinationResponse::Move(target)).await.is_err(),
                  };
                  if is_err {
                    eprintln!("Release issued for {}, but failed to tell player", player);
                  }
                }
              }
              PeerMessage::VisitorSend { capabilities, player, target, avatar } => {
                let player_name: std::sync::Arc<str> = std::sync::Arc::from(player);
                let player_id = spadina_core::player::PlayerIdentifier::Remote { server: peer_name.clone(), player: player_name.clone() };
                let result = match spadina_core::capabilities::all_supported(capabilities) {
                  Ok(capabilities) => {
                    if authnz.check_access("visitor_send", &player_id).await {
                      match directory.upgrade() {
                        Some(directory) => match target {
                          VisitorTarget::Realm { owner, asset } => {
                            let (tx, player_input) = tokio::sync::mpsc::channel(100);
                            let (player_output, rx) = tokio::sync::mpsc::channel(100);
                            let (avatar_tx, avatar_rx) = tokio::sync::watch::channel(avatar);
                            let capabilities = std::sync::Arc::new(capabilities);
                            match directory
                              .launch(crate::destination::LaunchRequest::Move(
                                crate::destination::PlayerHandle {
                                  avatar: avatar_rx,
                                  capabilities: capabilities.clone(),
                                  is_superuser: false,
                                  principal: player_id,
                                  tx,
                                  rx,
                                },
                                crate::destination::LaunchTarget::ByAsset { owner, asset },
                              ))
                              .await
                            {
                              Ok(()) => {
                                players_in_realm
                                  .insert(player_name.clone().into(), stream::RealmStream { player: player_name.clone(), rx: player_input });
                                Ok(FromPeer::Realm(player_output, avatar_tx, capabilities))
                              }
                              Err(()) => Err(()),
                            }
                          }
                          VisitorTarget::Host { host } => match directory.hosting.get(&host) {
                            None => Err(()),
                            Some(host) => {
                              let (tx, player_input) = tokio::sync::mpsc::channel(100);
                              let (player_output, rx) = tokio::sync::mpsc::channel(100);
                              let (avatar_tx, avatar_rx) = tokio::sync::watch::channel(avatar);
                              let capabilities = std::sync::Arc::new(capabilities);
                              if host
                                .add(crate::destination::PlayerHandle {
                                  avatar: avatar_rx,
                                  capabilities: capabilities.clone(),
                                  is_superuser: false,
                                  principal: player_id,
                                  tx,
                                  rx,
                                })
                                .await
                              {
                                Err(())
                              } else {
                                players_in_guest
                                  .insert(player_name.clone().into(), stream::GuestStream { player: player_name.clone(), rx: player_input });
                                Ok(FromPeer::Hosted(player_output, avatar_tx, capabilities))
                              }
                            }
                          },
                        },
                        None => Err(()),
                      }
                    } else {
                      Err(())
                    }
                  }
                  Err(_) => Err(()),
                };
                match result {
                  Err(()) => connection.send(PeerMessage::VisitorRelease { player: player_name.as_ref(), target: None }.as_wsm()).await,
                  Ok(location) => {
                    players_from_peer.insert(player_name.into(), location);
                  }
                }
              }
              PeerMessage::VisitorYank { player } => {
                players_from_peer.remove(&player);
                players_in_realm.remove(&player);
                players_in_guest.remove(&player);
              }
              PeerMessage::GuestRequest { player, request } => {
                if let Some(FromPeer::Hosted(output, _, _)) = players_from_peer.get(&player) {
                  let is_err = output.send(crate::destination::DestinationRequest::Request(request)).await.is_err();
                  if is_err {
                    eprintln!("Failed to send request from {} on {}", &player, &peer_name);
                  }
                } else {
                  connection.send(PeerMessage::VisitorYank { player }.as_wsm()).await;
                }
              }
              PeerMessage::GuestResponse { player, response } => {
                if let Some(OnPeer::Guest(output)) = players_on_peer.get(&player) {
                  let is_err = output.send(crate::destination::DestinationResponse::Response(response)).await.is_err();
                  if is_err {
                    eprintln!("Failed to send request from {} visiting {}", &player, &peer_name);
                  }
                }
              }
            }
          }
        }
      }
      internal_input.close();
      if let Some(directory) = directory.upgrade() {
        directory.clean_peer(&peer_name);
      }
    });
    result
  }
  pub(crate) fn is_dead(&self) -> bool {
    self.output.is_closed()
  }
  pub(crate) async fn check_online_status(
    &self,
    requester: std::sync::Arc<str>,
    target: std::sync::Arc<str>,
  ) -> tokio::sync::oneshot::Receiver<spadina_core::player::PlayerLocationState<crate::shstr::ShStr>> {
    let (output, input) = tokio::sync::oneshot::channel();
    if let Err(tokio::sync::mpsc::error::SendError(PeerRequest::CheckOnline { requester, target, output })) =
      self.output.send(PeerRequest::CheckOnline { requester, target, output }).await
    {
      if output.send(spadina_core::player::PlayerLocationState::Unknown).is_err() {
        eprintln!("Failed to fail sending player online stat for {} for {} on {}", requester, target, &self.name);
      }
    }
    input
  }
  pub(crate) async fn initiate_connection(&self) {
    if let Err(e) = self.output.send(PeerRequest::InitiateConnection).await {
      eprintln!("Failed to pass of handle for {}: {}", &self.name, e);
    }
  }
  pub(crate) async fn finish_connection(
    &self,
    socket: tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>,
    capabilities: std::collections::BTreeSet<&'static str>,
  ) {
    if let Err(e) = self.output.send(PeerRequest::Connect(socket, capabilities)).await {
      eprintln!("Failed to pass of handle for {}: {}", &self.name, e);
    }
  }
  pub(crate) async fn fetch_realms(&self, source: message::PeerRealmSource<std::sync::Arc<str>>, output: AvailableRealmSink) {
    if let Err(_) = self.output.send(PeerRequest::CheckRealms(source, output)).await {
      eprintln!("Failed to fail realm list request to {}", &self.name);
    }
  }
  pub(crate) async fn send_raw(&self, message: tokio_tungstenite::tungstenite::Message) {
    if let Err(_) = self.output.send(PeerRequest::Message(message)).await {
      eprintln!("Failed to request to {}", &self.name);
    }
  }
  pub(crate) async fn send_dm(
    &self,
    sender: std::sync::Arc<str>,
    db_id: i32,
    recipient: std::sync::Arc<str>,
    body: spadina_core::communication::MessageBody<String>,
  ) -> tokio::sync::oneshot::Receiver<spadina_core::communication::DirectMessageStatus> {
    let (output, rx) = tokio::sync::oneshot::channel();
    if let Err(tokio::sync::mpsc::error::SendError(PeerRequest::DirectMessage { output, .. })) =
      self.output.send(PeerRequest::DirectMessage { sender, db_id, recipient, body, output }).await
    {
      std::mem::drop(output.send(spadina_core::communication::DirectMessageStatus::InternalError));
    }
    rx
  }
  pub(crate) async fn send_host(
    &self,
    player: crate::destination::PlayerHandle<crate::destination::self_hosted::SelfHosted>,
    host: std::sync::Arc<str>,
  ) {
    if let Err(_) = self.output.send(PeerRequest::SendPlayerToHost { player, host }).await {
      eprintln!("Failed to player to remote realm on{}", &self.name);
    }
  }
  pub(crate) async fn send_realm(
    &self,
    player: crate::destination::PlayerHandle<crate::realm::Realm>,
    realm: spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>,
  ) {
    if let Err(_) = self.output.send(PeerRequest::SendPlayer { player, realm }).await {
      eprintln!("Failed to player to remote realm on{}", &self.name);
    }
  }
  pub(crate) async fn kill(&self) {
    if let Err(_) = self.output.send(PeerRequest::Quit).await {
      eprintln!("Failed to send quit command to {}", &self.name);
    }
  }
}

impl crate::http::websocket::WebSocketClient for Peer {
  type Claim = net::PeerClaim<String>;

  fn accept(
    directory: &std::sync::Arc<crate::destination::Directory>,
    claim: Self::Claim,
    capabilities: std::collections::BTreeSet<&'static str>,
    socket: tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    let directory = directory.clone();
    async move {
      directory.peer(&claim.name, |peer| peer.finish_connection(socket, capabilities).boxed()).await;
    }
    .boxed()
  }
}
