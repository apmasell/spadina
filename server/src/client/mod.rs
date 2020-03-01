use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use spadina_core::asset_store::AsyncAssetStore;
use spadina_core::net::ToWebMessage;

pub mod location;

pub(crate) struct Client {
  output: tokio::sync::mpsc::Sender<InternalClientRequest>,
}
pub(crate) enum InternalClientRequest {
  CheckOnline(crate::destination::SharedPlayerId, tokio::sync::oneshot::Sender<spadina_core::player::PlayerLocationState<std::sync::Arc<str>>>),
  DirectMessage(
    crate::destination::SharedPlayerId,
    spadina_core::communication::MessageBody<crate::shstr::ShStr>,
    tokio::sync::oneshot::Sender<spadina_core::communication::DirectMessageStatus>,
  ),
  Ignore,
  Message(tokio_tungstenite::tungstenite::Message),
  Redirect(Option<spadina_core::realm::RealmTarget<std::sync::Arc<str>>>),
  RedirectTrain(std::sync::Arc<str>, u16),
  Quit,
}

impl Client {
  pub fn new(
    player_name: std::sync::Arc<str>,
    authnz: std::sync::Arc<crate::access::AuthNZ>,
    database: std::sync::Arc<crate::database::Database>,
    is_superuser: bool,
    capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
    directory: std::sync::Weak<crate::destination::Directory>,
    mut connection: tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>,
  ) -> Self {
    let (output, mut internal_input) = tokio::sync::mpsc::channel(100);
    let result = Client { output };
    tokio::spawn(async move {
      let mut player_info = match database.player_load(&player_name, true) {
        Ok(Some(info)) => info,
        Ok(None) => {
          eprintln!("No player record could be created for {}", &player_name);
          std::mem::drop(connection.send(spadina_core::ClientResponse::<String>::Disconnect.as_wsm()).await);
          return;
        }
        Err(e) => {
          eprintln!("Failed to load player {}: {}", &player_name, e);
          std::mem::drop(connection.send(spadina_core::ClientResponse::<String>::Disconnect.as_wsm()).await);
          return;
        }
      };
      let current_avatar = match crate::database::persisted::PersistedWatch::new(database.clone(), crate::avatar::Avatar(player_info.db_id)) {
        Ok(avatar) => avatar,
        Err(e) => {
          eprintln!("Failed to load player's avatar {}: {}", &player_name, e);
          std::mem::drop(connection.send(spadina_core::ClientResponse::<String>::Disconnect.as_wsm()).await);
          return;
        }
      };
      enum Message {
        Internal(InternalClientRequest),
        External(spadina_core::ClientRequest<String>),
        ClearBookmarks,
      }
      let mut death = authnz.give_me_death();
      let mut bookmark_realms: Option<tokio::sync::watch::Receiver<Vec<spadina_core::realm::RealmDirectoryEntry<std::sync::Arc<str>>>>> = None;
      let mut current_location = location::Location::NoWhere;
      let mut side_tasks = futures::stream::FuturesUnordered::new();
      loop {
        fn parse_client(
          player_name: &str,
          message: Option<Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>>,
        ) -> Message {
          match message {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(data))) => match rmp_serde::from_slice(&data) {
              Ok(message) => Message::External(message),
              Err(e) => {
                eprintln!("Malformed message from {}: {}", player_name, e);
                Message::Internal(InternalClientRequest::Ignore)
              }
            },
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(data))) => match serde_json::from_str(&data) {
              Ok(message) => Message::External(message),
              Err(e) => {
                eprintln!("Malformed message from {}: {}", player_name, e);
                Message::Internal(InternalClientRequest::Ignore)
              }
            },
            Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(data))) => {
              Message::Internal(InternalClientRequest::Message(tokio_tungstenite::tungstenite::Message::Pong(data)))
            }
            Some(Ok(tokio_tungstenite::tungstenite::Message::Pong(_))) | Some(Ok(tokio_tungstenite::tungstenite::Message::Frame(_))) => {
              Message::Internal(InternalClientRequest::Ignore)
            }
            None
            | Some(Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed | tokio_tungstenite::tungstenite::Error::AlreadyClosed))
            | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => Message::Internal(InternalClientRequest::Quit),
            Some(Err(e)) => {
              eprintln!("Error on client web socket for {}: {}", player_name, e);
              Message::Internal(InternalClientRequest::Ignore)
            }
          }
        }
        let message = tokio::select! {
          r = async { bookmark_realms.as_mut().unwrap().changed().await }, if bookmark_realms.is_some() => match r {
            Ok(_) => Message::Internal(InternalClientRequest::Message(spadina_core::ClientResponse::RealmsAvailable { display: spadina_core::realm::RealmSource::Bookmarks, realms: bookmark_realms.as_ref().unwrap().borrow().clone() }.as_wsm())), Err(_) => Message::ClearBookmarks},
          r = connection.next() => parse_client(&player_name, r),
          r = current_location.next() => r.map(Message::Internal).unwrap_or(Message::Internal(InternalClientRequest::Ignore)),
          _ = death.recv() => Message::Internal(InternalClientRequest::Quit),
          r = internal_input.recv() => r.map(Message::Internal).unwrap_or(Message::Internal(InternalClientRequest::Quit)),
          r = side_tasks.next(), if !side_tasks.is_empty() => match r { Some(Ok(r)) => Message::Internal(r), _ => Message::Internal(InternalClientRequest::Ignore) },
        };
        match message {
          Message::ClearBookmarks => bookmark_realms = None,
          Message::Internal(InternalClientRequest::CheckOnline(requester, output)) => {
            let state = match player_info.location_acl.check(&requester, &authnz.server_name) {
              spadina_core::access::LocationAccess::Location => current_location.get_state(),
              spadina_core::access::LocationAccess::OnlineOnly => spadina_core::player::PlayerLocationState::Online,
              spadina_core::access::LocationAccess::Deny => spadina_core::player::PlayerLocationState::Unknown,
            };
            if let Err(_) = output.send(state) {
              eprintln!("Failed to send client state for {}", &player_name);
            }
          }

          Message::Internal(InternalClientRequest::DirectMessage(sender, body, status)) => {
            let result = if body.is_valid_at(&chrono::Utc::now())
              && player_info.message_acl.check(&sender, &authnz.server_name) == spadina_core::access::SimpleAccess::Allow
            {
              let result = if body.is_transient() {
                match &sender {
                  spadina_core::player::PlayerIdentifier::Local(sender) => database.direct_message_last_read_set(player_info.db_id, sender.as_ref()),
                  spadina_core::player::PlayerIdentifier::Remote { server, player } => {
                    database.remote_direct_message_last_read_set(player_info.db_id, &player, &server)
                  }
                }
              } else {
                match &sender {
                  spadina_core::player::PlayerIdentifier::Local(sender) => database.direct_message_write(sender.as_ref(), &player_name, &body),
                  spadina_core::player::PlayerIdentifier::Remote { server, player } => {
                    database.remote_direct_message_write(player_info.db_id, &player, &server, &body, None)
                  }
                }
              };
              match result {
                Ok(ts) => spadina_core::communication::DirectMessageStatus::Delivered(ts),
                Err(_) => spadina_core::communication::DirectMessageStatus::InternalError,
              }
            } else {
              spadina_core::communication::DirectMessageStatus::Forbidden
            };
            if let spadina_core::communication::DirectMessageStatus::Delivered(timestamp) = &result {
              let message = spadina_core::communication::DirectMessage { inbound: true, body, timestamp: *timestamp };
              if let Err(e) = connection
                .send(spadina_core::ClientResponse::DirectMessages { player: sender.as_ref(), messages: vec![message.as_ref()] }.as_wsm())
                .await
              {
                eprintln!("Failed to send to player {}: {}", &player_name, e);
              }
            }
            std::mem::drop(status.send(result));
          }
          Message::Internal(InternalClientRequest::Ignore) => (),
          Message::Internal(InternalClientRequest::Message(message)) => {
            if let Err(e) = connection.send(message).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::Internal(InternalClientRequest::Redirect(None)) => {
            current_location = location::Location::NoWhere;
            if let Err(e) = connection
              .send(spadina_core::ClientResponse::LocationChange { location: spadina_core::location::LocationResponse::<String>::NoWhere }.as_wsm())
              .await
            {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::Internal(InternalClientRequest::Redirect(Some(realm))) => {
            let (location, reason) = location::Location::new_realm(
              &directory,
              capabilities.clone(),
              is_superuser,
              player_name.clone(),
              &current_avatar,
              realm,
              player_info.debuted || is_superuser,
              &authnz.server_name,
            )
            .await;

            current_location = location;
            if let Err(e) = connection.send(spadina_core::ClientResponse::LocationChange { location: reason }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::Internal(InternalClientRequest::RedirectTrain(owner, train)) => {
            if !player_info.debuted {
              player_info.debuted = true;
              if let Err(e) = database.player_debut(player_info.db_id) {
                eprintln!("Failed to debut player {}: {}", &player_name, e);
              }
            }
            let (location, reason) = location::Location::new_realm_train(
              &directory,
              capabilities.clone(),
              is_superuser,
              player_name.clone(),
              &current_avatar,
              owner,
              train,
              player_info.debuted || is_superuser,
            )
            .await;

            current_location = location;
            if let Err(e) = connection.send(spadina_core::ClientResponse::LocationChange { location: reason }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }

          Message::Internal(InternalClientRequest::Quit) => {
            break;
          }
          Message::External(spadina_core::ClientRequest::AccountLockChange { id, name, locked }) => {
            if let Err(e) = connection
              .send(
                spadina_core::ClientResponse::AccountLockChange {
                  id,
                  result: if (is_superuser
                    || authnz.check_admin("account_lock_change", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await)
                    && !(locked && name.as_str() == player_name.as_ref())
                  {
                    authnz.authentication.lock(&name, locked, &database).await
                  } else {
                    spadina_core::UpdateResult::NotAllowed
                  },
                  name,
                }
                .as_wsm(),
              )
              .await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccountLockStatus { name }) => {
            if let Err(e) = connection
              .send(
                spadina_core::ClientResponse::AccountLockStatus {
                  status: if is_superuser
                    || authnz.check_admin("account_lock_check", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await
                  {
                    authnz.authentication.is_locked(&name, &database).await
                  } else {
                    spadina_core::access::AccountLockState::NotAllowed
                  },
                  name,
                }
                .as_wsm(),
              )
              .await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccessGet { target }) => {
            let acls = match &target {
              spadina_core::access::AccessTarget::AccessServer => authnz.access.read("access_get", Clone::clone).await,
              spadina_core::access::AccessTarget::AdminServer => authnz.admin.read("access_get", Clone::clone).await,
              spadina_core::access::AccessTarget::CreateOnServer => authnz.creating.read("access_get", Clone::clone).await,
              spadina_core::access::AccessTarget::DirectMessages => player_info.message_acl.clone(),
              spadina_core::access::AccessTarget::NewRealmDefaultAccess => {
                match database.player_acl(player_info.db_id, crate::database::schema::player::dsl::new_realm_access_acl) {
                  Ok(acl) => acl,
                  Err(e) => {
                    eprintln!("Failed to get new realm default access for {}: {}", &player_name, e);
                    crate::access::AccessSetting::default()
                  }
                }
              }
              spadina_core::access::AccessTarget::NewRealmDefaultAdmin => {
                match database.player_acl(player_info.db_id, crate::database::schema::player::dsl::new_realm_admin_acl) {
                  Ok(acl) => acl,
                  Err(e) => {
                    eprintln!("Failed to get new realm default access for {}: {}", &player_name, e);
                    crate::access::AccessSetting::default()
                  }
                }
              }
            };
            if let Err(e) = connection
              .send(spadina_core::ClientResponse::<String>::CurrentAccess { target, rules: acls.rules, default: acls.default }.as_wsm())
              .await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccessGetLocation) => {
            if let Err(e) = connection
              .send(
                spadina_core::ClientResponse::<String>::CurrentAccessLocation {
                  rules: player_info.location_acl.rules.clone(),
                  default: player_info.location_acl.default.clone(),
                }
                .as_wsm(),
              )
              .await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccessSet { id, target, rules, default }) => {
            let rules = rules.into_iter().filter_map(|rule| rule.as_local(&authnz.server_name)).collect();
            let response = match target {
              spadina_core::access::AccessTarget::AccessServer => {
                if is_superuser || authnz.check_admin("access_set", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                  authnz
                    .access
                    .write("access_set", |a| {
                      a.default = default;
                      a.rules = rules
                    })
                    .await
                } else {
                  spadina_core::UpdateResult::InternalError
                }
              }
              spadina_core::access::AccessTarget::AdminServer => {
                if is_superuser || authnz.check_admin("access_set", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                  authnz
                    .admin
                    .write("access_set", |a| {
                      a.default = default;
                      a.rules = rules
                    })
                    .await
                } else {
                  spadina_core::UpdateResult::InternalError
                }
              }
              spadina_core::access::AccessTarget::CreateOnServer => {
                if is_superuser || authnz.check_admin("access_set", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                  authnz
                    .creating
                    .write("access_set", |a| {
                      a.default = default;
                      a.rules = rules
                    })
                    .await
                } else {
                  spadina_core::UpdateResult::InternalError
                }
              }
              spadina_core::access::AccessTarget::DirectMessages => {
                player_info.message_acl.default = default;
                player_info.message_acl.rules = rules;
                match database.player_acl_write(player_info.db_id, crate::database::schema::player::dsl::message_acl, &player_info.message_acl) {
                  Ok(_) => spadina_core::UpdateResult::Success,
                  Err(e) => {
                    eprintln!("Failed to update DM ACL for {}: {}", &player_name, e);
                    spadina_core::UpdateResult::InternalError
                  }
                }
              }
              spadina_core::access::AccessTarget::NewRealmDefaultAccess => {
                match database.player_acl_write(
                  player_info.db_id,
                  crate::database::schema::player::dsl::new_realm_access_acl,
                  &crate::access::AccessSetting { rules, default },
                ) {
                  Ok(_) => spadina_core::UpdateResult::Success,
                  Err(e) => {
                    eprintln!("Failed to update new realm access ACL for {}: {}", &player_name, e);
                    spadina_core::UpdateResult::InternalError
                  }
                }
              }
              spadina_core::access::AccessTarget::NewRealmDefaultAdmin => {
                match database.player_acl_write(
                  player_info.db_id,
                  crate::database::schema::player::dsl::new_realm_admin_acl,
                  &crate::access::AccessSetting { rules, default },
                ) {
                  Ok(_) => spadina_core::UpdateResult::Success,
                  Err(e) => {
                    eprintln!("Failed to update new realm admin ACL for {}: {}", &player_name, e);
                    spadina_core::UpdateResult::InternalError
                  }
                }
              }
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AccessChange { id, response }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccessSetBulk { id, targets, realms }) => {
            let response = if targets.is_empty() {
              spadina_core::UpdateResult::Success
            } else {
              let realms = match realms {
                spadina_core::access::BulkRealmSelector::AllMine => Some(crate::database::realm_scope::RealmListScope::Owner(player_info.db_id)),
                spadina_core::access::BulkRealmSelector::AllForOther { player } => {
                  if player.as_str() == player_name.as_ref() {
                    Some(crate::database::realm_scope::RealmListScope::Owner(player_info.db_id))
                  } else if is_superuser || authnz.check_admin("access_set", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                    Some(crate::database::realm_scope::RealmListScope::OwnerByName(player))
                  } else {
                    None
                  }
                }
                spadina_core::access::BulkRealmSelector::AllServer => Some(crate::database::realm_scope::RealmListScope::All),
                spadina_core::access::BulkRealmSelector::MineByAsset { assets } => Some(crate::database::realm_scope::RealmListScope::Any(
                  assets
                    .into_iter()
                    .map(|asset| {
                      crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::Asset {
                        owner: player_info.db_id,
                        asset,
                      })
                    })
                    .collect(),
                )),
                spadina_core::access::BulkRealmSelector::OtherPlayerByAsset { assets, player } => {
                  if is_superuser || authnz.check_admin("access_set", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                    Some(crate::database::realm_scope::RealmListScope::Any(
                      assets
                        .into_iter()
                        .map(|asset| {
                          crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::NamedAsset {
                            owner: player.clone(),
                            asset,
                          })
                        })
                        .collect(),
                    ))
                  } else {
                    None
                  }
                }
              };
              match realms {
                Some(filter) => match database.realm_acl_write_bulk(filter, targets) {
                  Ok(()) => spadina_core::UpdateResult::Success,
                  Err(e) => {
                    eprintln!("Failed to bulk update realm ACLs: {}", e);
                    spadina_core::UpdateResult::InternalError
                  }
                },
                None => spadina_core::UpdateResult::NotAllowed,
              }
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AccessChange { id, response }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AccessLocationSet { id, rules, default }) => {
            player_info.location_acl.default = default;
            player_info.location_acl.rules = rules.into_iter().filter_map(|rule| rule.as_local(&authnz.server_name)).collect();

            let response =
              match database.player_acl_write(player_info.db_id, crate::database::schema::player::dsl::online_acl, &player_info.location_acl) {
                Ok(_) => spadina_core::UpdateResult::Success,
                Err(e) => {
                  eprintln!("Failed to update new realm access ACL for {}: {}", &player_name, e);
                  spadina_core::UpdateResult::InternalError
                }
              };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AccessChange { id, response }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AssetCreate { id, asset_type, name, tags, licence, compression, data }) => {
            if authnz
              .creating
              .read("create_asset", |acl| {
                acl.check(&spadina_core::player::PlayerIdentifier::Local(&player_name), &authnz.server_name)
                  == spadina_core::access::SimpleAccess::Allow
              })
              .await
            {
              match directory.upgrade() {
                None => {
                  if let Err(e) = connection
                    .send(
                      spadina_core::ClientResponse::<String>::AssetCreationFailed { id, error: spadina_core::AssetError::PermissionError }.as_wsm(),
                    )
                    .await
                  {
                    eprintln!("Failed to send to player {}: {}", &player_name, e);
                  }
                }
                Some(directory) => {
                  let db_id = player_info.db_id;
                  let database = database.clone();
                  let player_name = player_name.clone();
                  let author =
                    spadina_core::player::PlayerIdentifier::Remote { server: authnz.server_name.as_ref(), player: player_name.as_ref() }.to_string();
                  side_tasks.push(tokio::spawn(async move {
                    use spadina_core::asset::AssetKind;
                    let result =
                      match asset_type.as_str() {
                        spadina_core::asset::SimpleRealmDescription::<String, String, String, String>::KIND => {
                          spadina_core::asset::verify_submission::<
                            spadina_core::asset::SimpleRealmDescription<String, String, String, String>,
                            _,
                            String,
                          >(directory.asset_manager(), compression, &data)
                          .await
                        }
                        spadina_core::asset::PuzzleCustom::<String, String, String>::KIND => {
                          spadina_core::asset::verify_submission::<spadina_core::asset::PuzzleCustom<String, String, String>, _, _>(
                            directory.asset_manager(),
                            compression,
                            &data,
                          )
                          .await
                        }
                        <spadina_core::asset::SimpleSprayModel<spadina_core::asset::Mesh, u32, u32, u32> as spadina_core::asset::AssetKind<
                          String,
                        >>::KIND => {
                          spadina_core::asset::verify_submission::<
                            spadina_core::asset::SimpleSprayModel<spadina_core::asset::Mesh, u32, u32, u32>,
                            _,
                            String,
                          >(directory.asset_manager(), compression, &data)
                          .await
                        }
                        _ => Err(spadina_core::AssetError::UnknownKind),
                      };
                    InternalClientRequest::Message(
                      match result {
                        Ok(details) => {
                          let asset = spadina_core::asset::Asset {
                            asset_type,
                            author,
                            capabilities: details.capabilities,
                            children: details.children,
                            data,
                            compression,
                            licence,
                            name,
                            tags,
                            created: chrono::Utc::now(),
                          };
                          let principal = asset.principal_hash();
                          directory.asset_manager().push(&principal, &asset).await;
                          if let Err(e) = database.bookmark_add(
                            db_id,
                            &spadina_core::communication::Bookmark::Asset { kind: asset.asset_type.as_str(), asset: principal.as_str() },
                          ) {
                            eprint!("Failed to bookmark newly created asset {} for {}: {}", &principal, player_name, e);
                          }
                          spadina_core::ClientResponse::AssetCreationSucceeded { id, hash: principal }
                        }
                        Err(error) => spadina_core::ClientResponse::AssetCreationFailed { id, error },
                      }
                      .as_wsm(),
                    )
                  }));
                }
              }
            } else {
              if let Err(e) = connection
                .send(spadina_core::ClientResponse::<String>::AssetCreationFailed { id, error: spadina_core::AssetError::PermissionError }.as_wsm())
                .await
              {
                eprintln!("Failed to send to player {}: {}", &player_name, e);
              }
            }
          }
          Message::External(spadina_core::ClientRequest::AssetPull { principal }) => match directory.upgrade() {
            None => {
              if let Err(e) = connection.send(spadina_core::ClientResponse::AssetUnavailable { principal }.as_wsm()).await {
                eprintln!("Failed to send to player {}: {}", &player_name, e);
              }
            }
            Some(directory) => {
              side_tasks.push(tokio::spawn(async move {
                match directory.asset_manager().pull(&principal).await {
                  Ok(asset) => InternalClientRequest::Message(spadina_core::ClientResponse::Asset { principal, asset }.as_wsm()),
                  Err(e) => {
                    eprintln!("Asset {} cannot be pulled by player: {}", &principal, e);
                    InternalClientRequest::Message(spadina_core::ClientResponse::AssetUnavailable { principal }.as_wsm())
                  }
                }
              }));
            }
          },
          Message::External(spadina_core::ClientRequest::AnnouncementAdd { id, announcement }) => {
            let now = chrono::Utc::now();
            let result = if announcement.when.expires() < now {
              spadina_core::UpdateResult::NotAllowed
            } else if is_superuser || authnz.check_admin("announcement_add", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
              let spadina_core::communication::Announcement { title, body, when, realm, public } = announcement;
              authnz.announcements.write(|announcements| {
                announcements.push(spadina_core::communication::Announcement {
                  title: std::sync::Arc::from(title),
                  body: std::sync::Arc::from(body),
                  when,
                  realm: realm.map(|r| r.convert_str()),
                  public,
                });
                announcements.retain(|a| a.when.expires() > now);
              })
            } else {
              spadina_core::UpdateResult::NotAllowed
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AnnouncementUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AnnouncementClear { id }) => {
            let result =
              if is_superuser || authnz.check_admin("announcement_clear", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                authnz.announcements.write(|announcements| announcements.clear())
              } else {
                spadina_core::UpdateResult::NotAllowed
              };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AnnouncementUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AnnouncementList) => {
            if let Err(e) = connection.send(spadina_core::ClientResponse::Announcements { announcements: authnz.announcements.read() }.as_wsm()).await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AvatarGet) => {
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AvatarCurrent { avatar: current_avatar.read() }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::AvatarSet { id, avatar }) => {
            let success = current_avatar.write(|a| *a = avatar) == spadina_core::UpdateResult::Success;
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::AvatarUpdate { id, success }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::BookmarkAdd { id, bookmark }) => {
            let success = match bookmark.localize(&authnz.server_name) {
              Some(bookmark) => match database.bookmark_add(player_info.db_id, &bookmark) {
                Err(e) => {
                  eprintln!("Failed to write asset to database for {}: {}", &player_name, e);
                  false
                }
                Ok(_) => true,
              },
              None => false,
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::BookmarkUpdate { id, success }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::BookmarkRemove { id, bookmark }) => {
            let success = match bookmark.localize(&authnz.server_name) {
              Some(bookmark) => match database.bookmark_rm(player_info.db_id, &bookmark) {
                Err(e) => {
                  eprintln!("Failed to delete asset to database for {}: {}", &player_name, e);
                  false
                }
                Ok(_) => true,
              },
              None => false,
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::BookmarkUpdate { id, success }.as_wsm()).await {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::BookmarksList) => match database.bookmark_get(player_info.db_id, |b| Some(b)) {
            Err(e) => {
              eprintln!("Failed to get bookmarks for {}: {}", &player_name, e)
            }
            Ok(bookmarks) => {
              if let Err(e) = connection.send(spadina_core::ClientResponse::Bookmarks { bookmarks }.as_wsm()).await {
                eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
                break;
              }
            }
          },
          Message::External(spadina_core::ClientRequest::CalendarIdentifier) => {
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::Calendar { id: player_info.calendar_id.clone() }.as_wsm()).await {
              eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::CalendarReset { id, player }) => {
            let result = match player.filter(|player| player.as_str() != player_name.as_ref()) {
              None => match database.calendar_id(player_info.db_id) {
                Ok(id) => {
                  player_info.calendar_id = id;
                  spadina_core::UpdateResult::Success
                }
                Err(e) => {
                  eprintln!("Failed to reset calendar link for {}: {}", &player_name, e);
                  spadina_core::UpdateResult::InternalError
                }
              },
              Some(player) => {
                if is_superuser || authnz.check_admin("calendar_reset", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
                  match database.calendar_reset(&player) {
                    Ok(()) => spadina_core::UpdateResult::Success,
                    Err(e) => {
                      eprintln!("Failed to reset calendar link for {}: {}", &player, e);
                      spadina_core::UpdateResult::InternalError
                    }
                  }
                } else {
                  spadina_core::UpdateResult::NotAllowed
                }
              }
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::CalendarUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::CalendarRealmAdd { id, realm }) => {
            let success = match database.calendar_add(player_info.db_id, &realm) {
              Ok(_) => true,
              Err(e) => {
                eprintln!("Failed to add subscription for {} to {}: {}", &player_name, &realm, e);
                false
              }
            };

            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::CalendarRealmChange { id, success }.as_wsm()).await {
              eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::CalendarRealmClear { id }) => {
            let success = match database.calendar_rm_all(player_info.db_id) {
              Ok(_) => true,
              Err(e) => {
                eprintln!("Failed to remove all subscriptions for {}: {}", &player_name, e);
                false
              }
            };

            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::CalendarRealmChange { id, success }.as_wsm()).await {
              eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::CalendarRealmList) => match database.calendar_list(player_info.db_id, &authnz.server_name) {
            Err(e) => {
              eprintln!("Failed to fetch calendar subscriptions for {}: {}", &player_name, e);
            }
            Ok(mut realms) => {
              if realms.is_empty() {
                if let Err(e) = connection.send(spadina_core::ClientResponse::CalendarRealmList { realms }.as_wsm()).await {
                  eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
                  break;
                }
              } else {
                match directory.upgrade() {
                  Some(directory) => side_tasks.push(tokio::spawn(async move {
                    for realm in &mut realms {
                      realm.asset.upgrade_in_place();
                      realm.owner.upgrade_in_place();
                      realm.activity = directory
                        .realm_activity(spadina_core::realm::LocalRealmTarget {
                          asset: realm.asset.clone().to_arc(),
                          owner: realm.owner.clone().to_arc(),
                        })
                        .await;
                    }
                    InternalClientRequest::Message(spadina_core::ClientResponse::CalendarRealmList { realms }.as_wsm())
                  })),
                  None => {
                    if let Err(e) = connection.send(spadina_core::ClientResponse::CalendarRealmList { realms }.as_wsm()).await {
                      eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
                      break;
                    }
                  }
                }
              }
            }
          },
          Message::External(spadina_core::ClientRequest::CalendarRealmRemove { id, realm }) => {
            let success = match database.calendar_rm(player_info.db_id, &realm) {
              Ok(_) => true,
              Err(e) => {
                eprintln!("Failed to remove subscription for {} to {}: {}", &player_name, &realm, e);
                false
              }
            };

            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::CalendarRealmChange { id, success }.as_wsm()).await {
              eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::ConsensualEmoteRequest { emote, player }) => {
            current_location.consensual_emote_request(emote, player).await
          }
          Message::External(spadina_core::ClientRequest::ConsensualEmoteResponse { id, ok }) => {
            current_location.consensual_emote_response(id, ok).await
          }
          Message::External(spadina_core::ClientRequest::DirectMessageGet { player, from, to }) => {
            let player = player.localize(&authnz.server_name);
            let messages = match &player {
              spadina_core::player::PlayerIdentifier::Local(name) => database.direct_message_get(player_info.db_id, name, &from, &to),
              spadina_core::player::PlayerIdentifier::Remote { player: player_name, server: peer_server } => {
                database.remote_direct_message_get(player_info.db_id, player_name, peer_server)
              }
            };
            match messages {
              Ok(messages) => {
                if let Err(e) = connection.send(spadina_core::ClientResponse::DirectMessages { player, messages }.as_wsm()).await {
                  eprintln!("Failed to send mesage to player {}: {}", &player_name, e);
                  break;
                }
              }
              Err(e) => eprintln!("Failed to fetch messages between {} and {}: {}", &player_name, &player, e),
            }
          }
          Message::External(spadina_core::ClientRequest::DirectMessageSend { id, recipient, body }) => {
            let status = if player_info.debuted || is_superuser {
              match recipient.localize(&authnz.server_name) {
                spadina_core::player::PlayerIdentifier::Local(recipient) => {
                  let recipient = std::sync::Arc::from(recipient);
                  match directory.upgrade() {
                    Some(directory) => match directory.players.get(&recipient) {
                      Some(recipient_player) => {
                        let output =
                          recipient_player.send_dm(spadina_core::player::PlayerIdentifier::Local(player_name.clone()), body.convert_str()).await;
                        side_tasks.push(tokio::spawn(async move {
                          InternalClientRequest::Message(
                            spadina_core::ClientResponse::<String>::DirectMessageReceipt {
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
                          if recipient_info.message_acl.check(&spadina_core::player::PlayerIdentifier::Local(&player_name), &authnz.server_name)
                            == spadina_core::access::SimpleAccess::Allow
                          {
                            match database.direct_message_write(&player_name, &recipient, &body) {
                              Ok(ts) => Some(spadina_core::communication::DirectMessageStatus::Delivered(ts)),
                              Err(e) => {
                                eprintln!("Failed to deliver direct message from {} to {}: {}", &player_name, &recipient, e);
                                Some(spadina_core::communication::DirectMessageStatus::InternalError)
                              }
                            }
                          } else {
                            Some(spadina_core::communication::DirectMessageStatus::Forbidden)
                          }
                        }
                        Ok(None) => Some(spadina_core::communication::DirectMessageStatus::UnknownRecipient),
                        Err(e) => {
                          eprintln!("Failed to deliver direct message from {} to {}: {}", &player_name, &recipient, e);
                          Some(spadina_core::communication::DirectMessageStatus::InternalError)
                        }
                      },
                    },
                    None => Some(spadina_core::communication::DirectMessageStatus::Forbidden),
                  }
                }
                spadina_core::player::PlayerIdentifier::Remote { server, player } => match directory.upgrade() {
                  None => Some(spadina_core::communication::DirectMessageStatus::InternalError),
                  Some(directory) => {
                    let player_name = player_name.clone();
                    let server = std::sync::Arc::from(server);
                    let player = std::sync::Arc::from(player);

                    match directory.peer(&server, |peer| peer.send_dm(player_name, player_info.db_id, player, body).boxed()).await {
                      None => Some(spadina_core::communication::DirectMessageStatus::UnknownRecipient),

                      Some(status) => {
                        side_tasks.push(tokio::spawn(async move {
                          InternalClientRequest::Message(
                            spadina_core::ClientResponse::<String>::DirectMessageReceipt {
                              id,
                              status: status.await.unwrap_or(spadina_core::communication::DirectMessageStatus::InternalError),
                            }
                            .as_wsm(),
                          )
                        }));
                        None
                      }
                    }
                  }
                },
              }
            } else {
              Some(spadina_core::communication::DirectMessageStatus::Forbidden)
            };

            if let Some(status) = status {
              if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::DirectMessageReceipt { id, status }.as_wsm()).await {
                eprintln!("Failed to send to player {}: {}", &player_name, e);
                break;
              }
            }
          }
          Message::External(spadina_core::ClientRequest::DirectMessageStats) => {
            match database.direct_message_stats(player_info.db_id) {
              Ok((stats, last_login)) => {
                if let Err(e) = connection.send(spadina_core::ClientResponse::DirectMessageStats { stats, last_login }.as_wsm()).await {
                  eprintln!("Failed to send to player {}: {}", &player_name, e);
                  break;
                }
              }
              Err(e) => eprintln!("Failed to fetch messages stats for {}: {}", &player_name, e),
            };
          }
          Message::External(spadina_core::ClientRequest::FollowRequest { player }) => current_location.follow_request(player.convert_str()).await,
          Message::External(spadina_core::ClientRequest::FollowResponse { id, ok }) => current_location.follow_response(id, ok).await,
          Message::External(spadina_core::ClientRequest::InRealm { request }) => {
            current_location.send_realm(request.convert_str()).await;
          }
          Message::External(spadina_core::ClientRequest::Invite { id }) => {
            let result = if is_superuser || authnz.check_admin("invite", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
              authnz.authentication.invite(&authnz.server_name, &database).await
            } else {
              Err(spadina_core::communication::InvitationError::NotAllowed)
            };
            if let Err(e) = connection
              .send(
                match result {
                  Ok(url) => spadina_core::ClientResponse::InviteSuccess { id, url },
                  Err(error) => spadina_core::ClientResponse::InviteFailure { id, error },
                }
                .as_wsm(),
              )
              .await
            {
              eprintln!("Failed to send to player {}: {}", &player_name, e);
            }
          }
          Message::External(spadina_core::ClientRequest::LocationChange { location }) => {
            let (new_location, location) = match location {
              spadina_core::location::LocationRequest::Realm(realm) => {
                location::Location::new_realm(
                  &directory,
                  capabilities.clone(),
                  is_superuser,
                  player_name.clone(),
                  &current_avatar,
                  realm,
                  player_info.debuted || is_superuser,
                  &authnz.server_name,
                )
                .await
              }
              spadina_core::location::LocationRequest::Guest(host) => {
                if player_info.debuted || is_superuser {
                  location::Location::new_guest(&directory, capabilities.clone(), player_name.clone(), &current_avatar, host, &authnz.server_name)
                    .await
                } else {
                  (location::Location::NoWhere, spadina_core::location::LocationResponse::PermissionDenied)
                }
              }
              spadina_core::location::LocationRequest::Host { capabilities, rules, default } => {
                match spadina_core::capabilities::all_supported(capabilities) {
                  Err(capability) => {
                    (location::Location::NoWhere, spadina_core::location::LocationResponse::MissingCapabilities { capabilities: vec![capability] })
                  }
                  Ok(capabilities) => {
                    if is_superuser
                      || player_info.debuted
                        && authnz
                          .creating
                          .read("hosting", |acl| {
                            acl.check(&spadina_core::player::PlayerIdentifier::Local(&player_name), &authnz.server_name)
                              == spadina_core::access::SimpleAccess::Allow
                          })
                          .await
                    {
                      location::Location::new_hosting(
                        player_name.clone(),
                        authnz.server_name.clone(),
                        capabilities,
                        crate::access::AccessSetting { rules, default },
                        &directory,
                      )
                    } else {
                      (location::Location::NoWhere, spadina_core::location::LocationResponse::PermissionDenied)
                    }
                  }
                }
              }
              spadina_core::location::LocationRequest::NoWhere => (location::Location::NoWhere, spadina_core::location::LocationResponse::NoWhere),
            };
            current_location = new_location;
            if let Err(e) = connection.send(spadina_core::ClientResponse::LocationChange { location }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::LocationMessagesGet { from, to }) => current_location.message_get(from, to).await,
          Message::External(spadina_core::ClientRequest::LocationMessageSend { body }) => current_location.message_send(body.convert_str()).await,
          Message::External(spadina_core::ClientRequest::PlayerCheck { player }) => {
            let result = match player.localize(&authnz.server_name) {
              spadina_core::player::PlayerIdentifier::Local(name) => {
                if name.as_str() == player_name.as_ref() {
                  Some(
                    spadina_core::ClientResponse::PlayerState {
                      player: spadina_core::player::PlayerIdentifier::Local(player_name.clone()),
                      state: current_location.get_state(),
                    }
                    .as_wsm(),
                  )
                } else {
                  match directory.upgrade() {
                    None => Some(
                      spadina_core::ClientResponse::PlayerState {
                        player: spadina_core::player::PlayerIdentifier::Local(name),
                        state: spadina_core::player::PlayerLocationState::Unknown,
                      }
                      .as_wsm(),
                    ),
                    Some(directory) => {
                      let name = std::sync::Arc::from(name);
                      match directory.players.get(&name) {
                        Some(other) => {
                          let (sender, receiver) = tokio::sync::oneshot::channel();
                          match other
                            .output
                            .send(InternalClientRequest::CheckOnline(
                              spadina_core::player::PlayerIdentifier::Local(player_name.clone().into()),
                              sender,
                            ))
                            .await
                          {
                            Ok(()) => {
                              side_tasks.push(tokio::spawn(async move {
                                InternalClientRequest::Message(
                                  spadina_core::ClientResponse::PlayerState {
                                    player: spadina_core::player::PlayerIdentifier::Local(name),
                                    state: match receiver.await {
                                      Ok(v) => v,
                                      Err(_) => spadina_core::player::PlayerLocationState::Unknown,
                                    },
                                  }
                                  .as_wsm(),
                                )
                              }));
                              None
                            }
                            Err(_) => Some(
                              spadina_core::ClientResponse::PlayerState {
                                player: spadina_core::player::PlayerIdentifier::Local(name),
                                state: spadina_core::player::PlayerLocationState::Unknown,
                              }
                              .as_wsm(),
                            ),
                          }
                        }
                        None => {
                          let state = match database.player_load(&name, false) {
                            Ok(None) => spadina_core::player::PlayerLocationState::Invalid,
                            Ok(Some(info)) => {
                              match info.location_acl.check(&spadina_core::player::PlayerIdentifier::Local(&player_name), &authnz.server_name) {
                                spadina_core::access::LocationAccess::Location | spadina_core::access::LocationAccess::OnlineOnly => {
                                  spadina_core::player::PlayerLocationState::Offline
                                }
                                spadina_core::access::LocationAccess::Deny => spadina_core::player::PlayerLocationState::Unknown,
                              }
                            }
                            Err(e) => {
                              eprintln!("Failed to load player information for {} when requested by {}: {}", &name, &player_name, e);
                              spadina_core::player::PlayerLocationState::Unknown
                            }
                          };
                          Some(
                            spadina_core::ClientResponse::PlayerState { player: spadina_core::player::PlayerIdentifier::Local(name), state }.as_wsm(),
                          )
                        }
                      }
                    }
                  }
                }
              }
              spadina_core::player::PlayerIdentifier::Remote { server, player } => {
                let player = std::sync::Arc::from(player);
                let server = std::sync::Arc::from(server);
                match directory.upgrade() {
                  None => Some(
                    spadina_core::ClientResponse::PlayerState {
                      player: spadina_core::player::PlayerIdentifier::Remote { server, player },
                      state: spadina_core::player::PlayerLocationState::Unknown,
                    }
                    .as_wsm(),
                  ),
                  Some(directory) => {
                    match directory.peer(&server, |peer| peer.check_online_status(player_name.clone(), player.clone()).boxed()).await {
                      Some(receiver) => {
                        let player = player.clone();
                        side_tasks.push(tokio::spawn(async move {
                          let state = match receiver.await {
                            Ok(v) => v,
                            Err(_) => spadina_core::player::PlayerLocationState::Unknown,
                          };
                          InternalClientRequest::Message(
                            spadina_core::ClientResponse::PlayerState {
                              player: spadina_core::player::PlayerIdentifier::Remote { server: server.as_ref(), player: player.as_ref() },
                              state: state.as_ref(),
                            }
                            .as_wsm(),
                          )
                        }));
                        None
                      }
                      None => Some(
                        spadina_core::ClientResponse::PlayerState {
                          player: spadina_core::player::PlayerIdentifier::Remote { server: server.as_ref(), player: player.as_ref() },
                          state: spadina_core::player::PlayerLocationState::Invalid,
                        }
                        .as_wsm(),
                      ),
                    }
                  }
                }
              }
            };
            if let Some(message) = result {
              if let Err(e) = connection.send(message).await {
                eprintln!("Failed to send to player {}: {}", &player_name, e);
                break;
              }
            }
          }
          Message::External(spadina_core::ClientRequest::PlayerReset { id, player, reset }) => {
            let result = if is_superuser || authnz.check_admin("player_reset", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
              match database.player_reset(&player, reset) {
                Err(e) => {
                  eprintln!("Failed to delete player {}: {}", &player, e);
                  spadina_core::UpdateResult::InternalError
                }
                Ok(_) => spadina_core::UpdateResult::Success,
              }
            } else {
              spadina_core::UpdateResult::NotAllowed
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PlayerReset { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::PublicKeyAdd { id, der }) => {
            let result = match openssl::pkey::PKey::public_key_from_der(&der) {
              Err(_) => spadina_core::UpdateResult::NotAllowed,
              Ok(_) => match database.public_key_add(player_info.db_id, &der) {
                Ok(_) => spadina_core::UpdateResult::Success,
                Err(e) => {
                  eprintln!("Failed to add public key: {}", e);
                  spadina_core::UpdateResult::InternalError
                }
              },
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PublicKeyUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::PublicKeyDelete { id, name }) => {
            let result = match database.public_key_rm(player_info.db_id, &name) {
              Err(e) => {
                eprintln!("Failed to delete public key: {}", e);
                spadina_core::UpdateResult::InternalError
              }
              Ok(_) => spadina_core::UpdateResult::Success,
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PublicKeyUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::PublicKeyDeleteAll { id }) => {
            let result = match database.public_key_rm_all(player_info.db_id) {
              Err(e) => {
                eprintln!("Failed to delete all public keys: {}", e);
                spadina_core::UpdateResult::InternalError
              }
              Ok(_) => spadina_core::UpdateResult::Success,
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PublicKeyUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::PublicKeyList) => match database.public_key_list(player_info.db_id) {
            Ok(keys) => {
              if let Err(e) = connection.send(spadina_core::ClientResponse::PublicKeys { keys }.as_wsm()).await {
                eprintln!("Failed to send data to player {}: {}", &player_name, e);
                break;
              }
            }
            Err(e) => {
              eprintln!("Failed to get public keys: {}", e);
            }
          },
          Message::External(spadina_core::ClientRequest::NoOperation) => (),
          Message::External(spadina_core::ClientRequest::Quit) => {
            break;
          }

          Message::External(spadina_core::ClientRequest::RealmsList { source }) => {
            let results = match source {
              spadina_core::realm::RealmSource::Personal => Some((
                spadina_core::realm::RealmSource::Personal,
                database.realm_list(&authnz.server_name, true, crate::database::realm_scope::RealmListScope::<String>::Owner(player_info.db_id)),
                true,
              )),
              spadina_core::realm::RealmSource::LocalServer => Some((
                spadina_core::realm::RealmSource::LocalServer,
                database.realm_list(&authnz.server_name, false, crate::database::realm_scope::RealmListScope::<String>::InDirectory),
                true,
              )),
              spadina_core::realm::RealmSource::RemoteServer(peer_name) => match spadina_core::net::parse_server_name(&peer_name) {
                Some(peer_name) => {
                  if peer_name.as_str() == authnz.server_name.as_ref() {
                    Some((
                      spadina_core::realm::RealmSource::RemoteServer(peer_name),
                      database.realm_list(&authnz.server_name, false, crate::database::realm_scope::RealmListScope::<String>::InDirectory),
                      true,
                    ))
                  } else {
                    match directory.upgrade() {
                      Some(directory) => {
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        directory
                          .peer(&peer_name, |peer| {
                            peer
                              .fetch_realms(crate::peer::message::PeerRealmSource::InDirectory, crate::peer::AvailableRealmSink::OneShot(tx))
                              .boxed()
                          })
                          .await;
                        side_tasks.push(tokio::spawn(async move {
                          let realms = match rx.await {
                            Ok(v) => v,
                            Err(_) => Vec::new(),
                          };

                          InternalClientRequest::Message(
                            spadina_core::ClientResponse::RealmsAvailable {
                              display: spadina_core::realm::RealmSource::RemoteServer(std::sync::Arc::from(peer_name)),
                              realms,
                            }
                            .as_wsm(),
                          )
                        }));
                        None
                      }
                      None => Some((spadina_core::realm::RealmSource::RemoteServer(peer_name), Vec::new(), true)),
                    }
                  }
                }
                None => Some((spadina_core::realm::RealmSource::RemoteServer(peer_name), vec![], false)),
              },
              spadina_core::realm::RealmSource::Bookmarks => {
                let mut peer_realms: std::collections::BTreeMap<String, Vec<spadina_core::realm::LocalRealmTarget<String>>> = Default::default();
                let mut realms = Vec::new();
                match database.bookmark_get::<_, Vec<_>>(player_info.db_id, |v| match v {
                  spadina_core::communication::Bookmark::Realm(link) => match link {
                    spadina_core::realm::RealmTarget::LocalRealm { asset, owner } => {
                      Some((None, spadina_core::realm::LocalRealmTarget { asset, owner }))
                    }
                    spadina_core::realm::RealmTarget::RemoteRealm { server, asset, owner } => {
                      Some((Some(server), spadina_core::realm::LocalRealmTarget { asset, owner }))
                    }
                    _ => None,
                  },
                  _ => None,
                }) {
                  Err(diesel::result::Error::NotFound) => (),
                  Err(e) => {
                    eprintln!("Failed to fetch realm bookmarks: {}", e);
                  }
                  Ok(targets) => {
                    for (server, realm) in targets {
                      match server {
                        None => realms.push(realm),
                        Some(server) => peer_realms.entry(server).or_default().push(realm),
                      }
                    }
                  }
                }
                let mut realms = database.realm_list(
                  &authnz.server_name,
                  false,
                  crate::database::realm_scope::RealmListScope::Any(
                    realms
                      .into_iter()
                      .map(|spadina_core::realm::LocalRealmTarget { asset, owner }| {
                        crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::NamedAsset { owner, asset })
                      })
                      .collect(),
                  ),
                );

                if let Some(directory) = directory.upgrade() {
                  let (tx, rx) = tokio::sync::watch::channel(Vec::new());
                  let tx = std::sync::Arc::new(crate::prometheus_locks::mutex::PrometheusLabelledMutex::new_with_labels(
                    &crate::metrics::BOOKMARKS_SENDER,
                    tx,
                    (),
                  ));
                  bookmark_realms = Some(rx);
                  if !realms.is_empty() {
                    let tx = tx.clone();
                    let directory = directory.clone();
                    tokio::spawn(async move {
                      for realm in &mut realms {
                        realm.activity = directory
                          .realm_activity(spadina_core::realm::LocalRealmTarget { asset: realm.asset.clone(), owner: realm.owner.clone() })
                          .await;
                      }
                      tx.lock("bookmarks_add_activity").await.send_modify(|r| r.extend(realms));
                    });
                  }
                  for (peer_name, realms) in peer_realms {
                    let tx = tx.clone();
                    directory
                      .peer(&peer_name, |peer| {
                        peer
                          .fetch_realms(
                            crate::peer::message::PeerRealmSource::Specific { realms: realms.into_iter().map(|realm| realm.convert_str()).collect() },
                            crate::peer::AvailableRealmSink::Watch(tx),
                          )
                          .boxed()
                      })
                      .await;
                  }
                  None
                } else {
                  Some((spadina_core::realm::RealmSource::Bookmarks, realms, true))
                }
              }
              spadina_core::realm::RealmSource::Manual(target) => match target.localize(&authnz.server_name) {
                spadina_core::realm::RealmTarget::Home => Some((
                  spadina_core::realm::RealmSource::Manual(spadina_core::realm::RealmTarget::Home),
                  database.realm_list(
                    &authnz.server_name,
                    true,
                    crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::<String>::Train {
                      owner: player_info.db_id,
                      train: 0,
                    }),
                  ),
                  true,
                )),
                spadina_core::realm::RealmTarget::LocalRealm { asset, owner } => {
                  let realms = database.realm_list(
                    &authnz.server_name,
                    true,
                    crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::NamedAsset {
                      owner: owner.as_str(),
                      asset: asset.as_str(),
                    }),
                  );
                  Some((spadina_core::realm::RealmSource::Manual(spadina_core::realm::RealmTarget::LocalRealm { owner, asset }), realms, true))
                }
                spadina_core::realm::RealmTarget::PersonalRealm { asset } => {
                  let realms = database.realm_list(
                    &authnz.server_name,
                    true,
                    crate::database::realm_scope::RealmListScope::Single(crate::database::realm_scope::RealmScope::Asset {
                      owner: player_info.db_id,
                      asset: &asset,
                    }),
                  );
                  Some((spadina_core::realm::RealmSource::Manual(spadina_core::realm::RealmTarget::PersonalRealm { asset }), realms, true))
                }
                spadina_core::realm::RealmTarget::RemoteRealm { server, owner, asset } => match directory.upgrade() {
                  Some(directory) => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let asset: std::sync::Arc<str> = asset.into();
                    let owner: std::sync::Arc<str> = owner.into();
                    let realm = spadina_core::realm::LocalRealmTarget { asset: asset.clone(), owner: owner.clone() };
                    directory
                      .peer(&server, |peer| {
                        peer
                          .fetch_realms(
                            crate::peer::message::PeerRealmSource::Specific { realms: vec![realm] },
                            crate::peer::AvailableRealmSink::OneShot(tx),
                          )
                          .boxed()
                      })
                      .await;
                    side_tasks.push(tokio::spawn(async move {
                      let realms = match rx.await {
                        Ok(v) => v,
                        Err(_) => Vec::new(),
                      };

                      InternalClientRequest::Message(
                        spadina_core::ClientResponse::RealmsAvailable {
                          display: spadina_core::realm::RealmSource::Manual(spadina_core::realm::RealmTarget::RemoteRealm {
                            server: std::sync::Arc::from(server),
                            owner,
                            asset,
                          }),
                          realms,
                        }
                        .as_wsm(),
                      )
                    }));
                    None
                  }
                  None => Some((
                    spadina_core::realm::RealmSource::Manual(spadina_core::realm::RealmTarget::RemoteRealm { server, owner, asset }),
                    Vec::new(),
                    false,
                  )),
                },
              },
            };
            if let Some((display, mut realms, update_activity)) = results {
              let upgrade = if update_activity { directory.upgrade() } else { None };

              if let Some(directory) = upgrade {
                side_tasks.push(tokio::spawn(async move {
                  for realm in &mut realms {
                    realm.activity = directory
                      .realm_activity(spadina_core::realm::LocalRealmTarget { asset: realm.asset.clone(), owner: realm.owner.clone() })
                      .await;
                  }
                  InternalClientRequest::Message(spadina_core::ClientResponse::RealmsAvailable { display: display.convert_str(), realms }.as_wsm())
                }));
              } else {
                if let Err(e) =
                  connection.send(spadina_core::ClientResponse::RealmsAvailable { display: display.convert_str(), realms }.as_wsm()).await
                {
                  eprintln!("Failed to send data to player {}: {}", &player_name, e);
                  break;
                }
              }
            }
          }
          Message::External(spadina_core::ClientRequest::PeerBanClear { id, bans }) => {
            let bans: Vec<_> = bans.into_iter().filter_map(|ban| ban.clean()).collect();
            let result = if is_superuser || authnz.check_admin("peer_block_clear", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await
            {
              authnz
                .banned_peers
                .write("client_clear_ban", |banned| {
                  for ban in bans {
                    banned.remove(&ban);
                  }
                })
                .await
            } else {
              spadina_core::UpdateResult::NotAllowed
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PeersBannedUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::PeerBanList) => {
            if let Err(e) = connection
              .send(
                spadina_core::ClientResponse::PeersBanned { bans: authnz.banned_peers.read("client_list_ban", |bans| bans.clone()).await }.as_wsm(),
              )
              .await
            {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::RealmDelete { id, asset, owner }) => {
            if let Some(directory) = directory.upgrade() {
              let (sender, reciever) = tokio::sync::oneshot::channel();
              if let Err(()) = directory
                .launch(crate::destination::LaunchRequest::Delete(
                  spadina_core::realm::LocalRealmTarget {
                    asset: std::sync::Arc::from(asset),
                    owner: owner.map(std::sync::Arc::from).unwrap_or_else(|| player_name.clone()),
                  },
                  if is_superuser { None } else { Some(spadina_core::player::PlayerIdentifier::Local(player_name.clone())) },
                  sender,
                ))
                .await
              {
                eprintln!("Failed to delete realm for player {}", &player_name);
              }
              side_tasks.push(tokio::spawn(async move {
                InternalClientRequest::Message(
                  spadina_core::ClientResponse::<String>::RealmDelete {
                    id,
                    result: match reciever.await {
                      Ok(value) => value,
                      Err(_) => spadina_core::UpdateResult::InternalError,
                    },
                  }
                  .as_wsm(),
                )
              }))
            }
          }
          Message::External(spadina_core::ClientRequest::PeerBanSet { id, bans }) => {
            let bans: std::collections::HashSet<_> = bans.into_iter().filter_map(|ban| ban.clean()).collect();
            let result = if is_superuser || authnz.check_admin("peer_block_clear", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await
            {
              if let Some(directory) = directory.upgrade() {
                directory.apply_peer_bans(&bans);
              }
              authnz.banned_peers.write("client_set_ban", |banned| banned.extend(bans)).await
            } else {
              spadina_core::UpdateResult::NotAllowed
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::<String>::PeersBannedUpdate { id, result }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::Peers) => {
            let peers = match directory.upgrade() {
              Some(directory) => directory.peers(),
              None => Vec::new(),
            };
            if let Err(e) = connection.send(spadina_core::ClientResponse::Peers { peers }.as_wsm()).await {
              eprintln!("Failed to send data to player {}: {}", &player_name, e);
              break;
            }
          }
          Message::External(spadina_core::ClientRequest::TrainAdd { id, asset, allow_first }) => {
            if is_superuser || authnz.check_admin("peer_block_clear", &spadina_core::player::PlayerIdentifier::Local(&player_name)).await {
              if let Some(directory) = directory.upgrade() {
                let database = database.clone();
                side_tasks.push(tokio::spawn(async move {
                  InternalClientRequest::Message(
                    spadina_core::ClientResponse::<String>::TrainAdd {
                      id,
                      result: match directory.asset_manager().pull(&asset).await {
                        Ok(realm_asset) => match spadina_core::asset::AssetAnyRealm::<String>::load(realm_asset, directory.asset_manager()).await {
                          Ok((realm, _)) => {
                            let propagation_rules = match realm {
                              spadina_core::asset::AssetAnyRealm::Simple(r) => r.propagation_rules,
                            };
                            if propagation_rules.iter().any(|rule| match rule.propagation_match {
                              spadina_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext => true,
                              _ => false,
                            }) {
                              match database.train_add(&asset, allow_first) {
                                Err(e) => {
                                  eprintln!("Failed to add train asset {}: {}", &asset, e);
                                  spadina_core::TrainAddResult::InternalError
                                }
                                Ok(_) => {
                                  if let Err(_) = directory.launch(crate::destination::LaunchRequest::ClearCache).await {
                                    eprintln!("Failed to flush cache when adding new train car");
                                  }
                                  spadina_core::TrainAddResult::Success
                                }
                              }
                            } else {
                              spadina_core::TrainAddResult::NotTrain
                            }
                          }
                          Err(e) => spadina_core::TrainAddResult::Asset(e),
                        },
                        Err(_) => spadina_core::TrainAddResult::NotFound,
                      },
                    }
                    .as_wsm(),
                  )
                }));
              }
            } else {
              if let Err(e) = connection
                .send(spadina_core::ClientResponse::<String>::TrainAdd { id, result: spadina_core::TrainAddResult::NotAllowed }.as_wsm())
                .await
              {
                eprintln!("Failed to send data to player {}: {}", &player_name, e);
                break;
              }
            }
          }
          Message::External(spadina_core::ClientRequest::ToHost { request }) => current_location.send_guest_request(request.convert_str()).await,
          Message::External(spadina_core::ClientRequest::FromHost { request }) => current_location.send_host_command(request.convert_str()).await,
        }
        std::mem::drop(connection.send(spadina_core::ClientResponse::<String>::Disconnect.as_wsm()).await);
      }
      internal_input.close();
      if let Some(directory) = directory.upgrade() {
        directory.players.remove_if(&player_name, |_, old_client| old_client.output.is_closed());
      }
    });
    result
  }
  pub async fn kill(&self) {
    std::mem::drop(self.output.clone().send(InternalClientRequest::Quit).await);
  }
  pub async fn check_online_status(
    &self,
    player: crate::destination::SharedPlayerId,
  ) -> tokio::sync::oneshot::Receiver<spadina_core::player::PlayerLocationState<std::sync::Arc<str>>> {
    let (output, input) = tokio::sync::oneshot::channel();
    if let Err(tokio::sync::mpsc::error::SendError(InternalClientRequest::CheckOnline(player, output))) =
      self.output.send(InternalClientRequest::CheckOnline(player, output)).await
    {
      if output.send(spadina_core::player::PlayerLocationState::Unknown).is_err() {
        eprintln!("Failed to fail sending player online stat for {}", player);
      }
    }
    input
  }
  pub async fn send_dm(
    &self,
    sender: crate::destination::SharedPlayerId,
    message: spadina_core::communication::MessageBody<crate::shstr::ShStr>,
  ) -> tokio::sync::oneshot::Receiver<spadina_core::communication::DirectMessageStatus> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Err(tokio::sync::mpsc::error::SendError(InternalClientRequest::DirectMessage(_, _, tx))) =
      self.output.send(InternalClientRequest::DirectMessage(sender, message, tx)).await
    {
      std::mem::drop(tx.send(spadina_core::communication::DirectMessageStatus::InternalError));
    }
    rx
  }
}
impl crate::http::websocket::WebSocketClient for Client {
  type Claim = crate::http::jwt::PlayerClaim<String>;

  fn accept(
    directory: &std::sync::Arc<crate::destination::Directory>,
    claim: Self::Claim,
    capabilities: std::collections::BTreeSet<&'static str>,
    socket: tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    let player_name: std::sync::Arc<str> = std::sync::Arc::from(claim.name);
    let (authnz, database) = directory.clone_auth_and_db();
    if let Some(old_client) = directory.players.insert(
      player_name.clone(),
      crate::client::Client::new(
        player_name,
        authnz,
        database,
        false,
        std::sync::Arc::new(capabilities),
        std::sync::Arc::downgrade(directory),
        socket,
      ),
    ) {
      async move { old_client.kill().await }.boxed()
    } else {
      async {}.boxed()
    }
  }
}
