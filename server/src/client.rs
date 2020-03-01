use futures::SinkExt;

pub(crate) async fn process_client_request(
  server: &std::sync::Arc<crate::Server>,
  player_name: &str,
  player: &crate::PlayerKey,
  db_id: i32,
  superuser: bool,
  request: puzzleverse_core::ClientRequest,
) -> bool {
  match server.player_states.read("process_client_request").await.get(player.clone()) {
    Some(player_state) => {
      let mut mutable_player_state = player_state.mutable.lock().await;
      match request {
        puzzleverse_core::ClientRequest::AccountLockChange { id, name, locked } => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::AccountLockChange {
              id,
              success: if (superuser || {
                let acl = &server.admin_acl.lock().await;
                (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
              }) && !(locked && &name == player_name)
              {
                server.authentication.lock(&name, locked).await
              } else {
                false
              },
              name,
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AccountLockStatus { name } => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::AccountLockStatus {
              status: if superuser || {
                let acl = &server.admin_acl.lock().await;
                (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
              } {
                server.authentication.is_locked(&name).await
              } else {
                puzzleverse_core::AccountLockState::NotAllowed
              },
              name,
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AccessGet { target } => {
          let acls = match &target {
            puzzleverse_core::AccessTarget::AccessServer => &server.access_acl,
            puzzleverse_core::AccessTarget::AdminServer => &server.admin_acl,
            puzzleverse_core::AccessTarget::CheckOnline => &player_state.online_acl,
            puzzleverse_core::AccessTarget::DirectMessagesServer => &server.message_acl,
            puzzleverse_core::AccessTarget::DirectMessagesUser => &player_state.message_acl,
            puzzleverse_core::AccessTarget::NewRealmDefaultAccess => &player_state.new_realm_access_acl,
            puzzleverse_core::AccessTarget::NewRealmDefaultAdmin => &player_state.new_realm_admin_acl,
            puzzleverse_core::AccessTarget::ViewLocation => &player_state.location_acl,
          }
          .lock()
          .await;
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::CurrentAccess { target, acls: acls.1.clone(), default: acls.0.clone() })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AccessSet { id, target, acls, default } => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::AccessChange {
              id,
              response: if match &target {
                puzzleverse_core::AccessTarget::DirectMessagesUser => true,
                puzzleverse_core::AccessTarget::CheckOnline => true,
                puzzleverse_core::AccessTarget::NewRealmDefaultAccess => true,
                puzzleverse_core::AccessTarget::NewRealmDefaultAdmin => true,
                puzzleverse_core::AccessTarget::ViewLocation => true,
                _ => false,
              } || superuser
                || {
                  let acl = &server.admin_acl.lock().await;
                  (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
                } {
                let update_result = server.database.acl_write(player_name, &target, &default, &acls);
                match update_result {
                  Ok(_) => {
                    *(match &target {
                      puzzleverse_core::AccessTarget::AccessServer => &server.access_acl,
                      puzzleverse_core::AccessTarget::AdminServer => &server.admin_acl,
                      puzzleverse_core::AccessTarget::DirectMessagesUser => &player_state.message_acl,
                      puzzleverse_core::AccessTarget::DirectMessagesServer => &server.message_acl,
                      puzzleverse_core::AccessTarget::CheckOnline => &player_state.online_acl,
                      puzzleverse_core::AccessTarget::NewRealmDefaultAccess => &player_state.new_realm_access_acl,
                      puzzleverse_core::AccessTarget::NewRealmDefaultAdmin => &player_state.new_realm_admin_acl,
                      puzzleverse_core::AccessTarget::ViewLocation => &player_state.location_acl,
                    }
                    .lock()
                    .await) = (default, acls);
                    puzzleverse_core::AccessChangeResponse::Changed
                  }
                  Err(e) => {
                    eprintln!("Failed to update ACL {:?}: {}", &target, e);
                    puzzleverse_core::AccessChangeResponse::InternalError
                  }
                }
              } else {
                puzzleverse_core::AccessChangeResponse::Denied
              },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AssetCreate { id, asset_type, name, tags, licence, data } => {
          use puzzleverse_core::asset::AssetKind;
          let result = match asset_type.as_str() {
            puzzleverse_core::asset::SimpleRealmDescription::KIND => {
              puzzleverse_core::asset::verify_submission::<puzzleverse_core::asset::SimpleRealmDescription<String, String, String>, _>(
                &server.asset_store,
                &data,
              )
              .await
            }
            puzzleverse_core::asset::PuzzleCustom::KIND => {
              puzzleverse_core::asset::verify_submission::<puzzleverse_core::asset::PuzzleCustom<String, String>, _>(&server.asset_store, &data).await
            }
            puzzleverse_core::asset::SimpleSprayModel::KIND => {
              puzzleverse_core::asset::verify_submission::<puzzleverse_core::asset::SimpleSprayModel, _>(&server.asset_store, &data).await
            }
            _ => Err(puzzleverse_core::AssetError::UnknownKind),
          };
          mutable_player_state
            .connection
            .send_local(match result {
              Ok(details) => {
                let asset = puzzleverse_core::asset::Asset {
                  asset_type,
                  author: format!("{}@{}", player_name, &server.name),
                  capabilities: details.capabilities,
                  children: details.children,
                  data,
                  licence,
                  name,
                  tags,
                  created: chrono::Utc::now(),
                };
                let principal = asset.principal_hash();
                server.asset_store.push(&principal, &asset).await;
                if let Err(e) = server.database.bookmark_add(db_id, &puzzleverse_core::BookmarkType::Asset, &principal) {
                  eprint!("Failed to bookmark newly created asset {} for {}: {}", &principal, player_name, e);
                }
                puzzleverse_core::ClientResponse::AssetCreationSucceeded { id, hash: principal }
              }
              Err(error) => puzzleverse_core::ClientResponse::AssetCreationFailed { id, error },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AssetPull { id } => {
          let mut retry = true;
          while retry {
            retry = false;
            match server.asset_store.pull(&id).await {
              Ok(asset) => {
                mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::Asset(id.clone(), asset)).await;
              }
              Err(puzzleverse_core::asset_store::LoadError::Unknown) => {
                retry = server.find_asset(&id, crate::AssetPullAction::PushToPlayer(player.clone())).await;
              }
              Err(puzzleverse_core::asset_store::LoadError::InternalError) => {
                eprintln!("Asset {} cannot be loaded", &id);
                mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::AssetUnavailable(id.clone())).await;
              }
              Err(puzzleverse_core::asset_store::LoadError::Corrupt) => {
                eprintln!("Asset {} is corrupt", &id);
                mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::AssetUnavailable(id.clone())).await;
              }
            };
          }
          false
        }
        puzzleverse_core::ClientRequest::AnnouncementAdd(announcement) => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            let mut announcements = server.announcements.write("clear").await;
            announcements.push(announcement);
            let now = chrono::Utc::now();
            announcements.retain(|a| a.expires > now);
            server.database.announcements(&announcements);
          }
          false
        }
        puzzleverse_core::ClientRequest::AnnnouncementClear => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            let mut announcements = server.announcements.write("clear").await;
            announcements.clear();
            server.database.announcements(&announcements);
            tokio::spawn(server.clone().update_announcements());
          }
          false
        }
        puzzleverse_core::ClientRequest::AvatarGet => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::AvatarCurrent(player_state.avatar.read("avatar_get").await.clone()))
            .await;
          false
        }
        puzzleverse_core::ClientRequest::AvatarSet(avatar) => {
          let mut player_avatar = player_state.avatar.write("avatar_set").await;
          *player_avatar = avatar;
          if let Err(e) = server.database.player_avatar(db_id, &*player_avatar) {
            eprintln!("Failed to update avatar for {}: {}", player_name, e);
          }
          if let crate::player_state::Goal::OnPeer(peer_key, _) = &mutable_player_state.goal {
            if let Some(peer_state) = server.peer_states.read("avatar_set").await.get(peer_key.clone()) {
              peer_state
                .connection
                .lock("avatar_set")
                .await
                .send(crate::peer::PeerMessage::AvatarSet { player: player_name.to_string(), avatar: player_avatar.clone() })
                .await;
            }
          }
          false
        }
        puzzleverse_core::ClientRequest::BookmarkAdd(bookmark_type, asset) => {
          if let Err(e) = server.database.bookmark_add(db_id, &bookmark_type, &asset) {
            eprintln!("Failed to write asset to database for {}: {}", player_name, e)
          }
          false
        }
        puzzleverse_core::ClientRequest::BookmarkRemove(bookmark_type, asset) => {
          if let Err(e) = server.database.bookmark_rm(db_id, &bookmark_type, &asset) {
            eprintln!("Failed to delete asset to database for {}: {}", player_name, e)
          }

          false
        }
        puzzleverse_core::ClientRequest::BookmarksGet(bookmark_type) => {
          match server.database.bookmark_get(db_id, &bookmark_type) {
            Err(e) => {
              eprintln!("Failed to delete asset to database for {}: {}", player_name, e)
            }
            Ok(assets) => mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::Bookmarks(bookmark_type, assets)).await,
          }
          false
        }
        puzzleverse_core::ClientRequest::Capabilities => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::Capabilities {
              server_capabilities: puzzleverse_core::CAPABILITIES.iter().map(|s| s.to_string()).collect(),
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::DirectMessageGet { player, from, to } => {
          match puzzleverse_core::PlayerIdentifier::new(&player, Some(&server.name)) {
            puzzleverse_core::PlayerIdentifier::Local(name) => match server.database.direct_message_get(db_id, &player, &from, &to) {
              Ok(messages) => {
                mutable_player_state
                  .connection
                  .send_local(puzzleverse_core::ClientResponse::DirectMessages {
                    player,
                    messages: messages
                      .into_iter()
                      .map(|(body, timestamp, inbound)| puzzleverse_core::DirectMessage { body, inbound, timestamp })
                      .collect(),
                  })
                  .await
              }
              Err(e) => eprintln!("Failed to fetch messages between {} and {}: {}", &player_name, &name, e),
            },
            puzzleverse_core::PlayerIdentifier::Remote { player, server: peer_server } => {
              match server.database.remote_direct_message_get(db_id, &player, &peer_server) {
                Ok(messages) => {
                  mutable_player_state
                    .connection
                    .send_local(puzzleverse_core::ClientResponse::DirectMessages {
                      player,
                      messages: messages
                        .into_iter()
                        .map(|(body, timestamp, inbound)| puzzleverse_core::DirectMessage { body, inbound, timestamp })
                        .collect(),
                    })
                    .await
                }
                Err(e) => eprintln!("Failed to fetch messages between {} and {}@{}: {}", &player_name, &player, &peer_server, e),
              }
            }
            puzzleverse_core::PlayerIdentifier::Bad => (),
          }
          false
        }
        puzzleverse_core::ClientRequest::DirectMessageSend { id, recipient, body } => {
          let timestamp = chrono::Utc::now();
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::DirectMessageReceipt {
              id,
              status: if player_state.debuted.load(std::sync::atomic::Ordering::Relaxed) {
                match puzzleverse_core::PlayerIdentifier::new(&recipient, Some(&server.name)) {
                  puzzleverse_core::PlayerIdentifier::Local(name) => server.send_direct_message(player_name, &name, body).await,
                  puzzleverse_core::PlayerIdentifier::Remote { server: peer_name, player } => {
                    let was_sent = match server.peers.read("send_direct_message").await.get(&peer_name) {
                      Some(peer_key) => match server.peer_states.read("send_direct_message").await.get(peer_key.clone()) {
                        None => false,
                        Some(state) => {
                          let mut locked_state = state.connection.lock("direct_message").await;
                          match &mut *locked_state {
                            crate::peer::PeerConnection::Online(connection) => match connection
                              .send(crate::peer::PeerMessage::DirectMessage(vec![crate::peer::PeerDirectMessage {
                                sender: player_name.to_string(),
                                recipient: player.clone(),
                                timestamp,
                                body: body.clone(),
                              }]))
                              .await
                            {
                              Ok(_) => true,
                              Err(e) => {
                                eprintln!("Failed to send direct message to {}: {}", &peer_name, e);
                                false
                              }
                            },
                            crate::peer::PeerConnection::Dead(_, _) => false,
                            crate::peer::PeerConnection::Offline => false,
                          }
                        }
                      },
                      None => false,
                    };
                    if !was_sent {
                      server.attempt_peer_server_connection(&peer_name).await;
                    }

                    match server.database.remote_direct_message_write(db_id, &player, &peer_name, &body, &timestamp, if was_sent { "o" } else { "O" })
                    {
                      Ok(_) => {
                        if was_sent {
                          puzzleverse_core::DirectMessageStatus::Delivered
                        } else {
                          puzzleverse_core::DirectMessageStatus::Queued
                        }
                      }
                      Err(e) => {
                        eprintln!("Failed to write remote direct message to {} to database: {}", &peer_name, e);
                        puzzleverse_core::DirectMessageStatus::InternalError
                      }
                    }
                  }
                  puzzleverse_core::PlayerIdentifier::Bad => puzzleverse_core::DirectMessageStatus::UnknownRecipient,
                }
              } else {
                puzzleverse_core::DirectMessageStatus::Forbidden
              },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::DirectMessageStats => {
          match server.database.direct_message_stats(db_id) {
            Ok((stats, last_login)) => {
              mutable_player_state
                .connection
                .send_local(puzzleverse_core::ClientResponse::DirectMessageStats { stats: stats.into_iter().collect(), last_login })
                .await
            }
            Err(e) => eprintln!("Failed to fetch messages stats for {}: {}", &player_name, e),
          };
          false
        }
        puzzleverse_core::ClientRequest::InRealm(realm_request) => {
          if server.process_realm_request(&player_name, &player, None, superuser, realm_request).await {
            mutable_player_state.goal = crate::player_state::Goal::Undecided;
            mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::InTransit).await;
          }
          false
        }
        puzzleverse_core::ClientRequest::Invite { id } => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::Invite {
              id,
              url: if superuser || {
                let acl = &server.admin_acl.lock().await;
                (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
              } {
                server.authentication.invite(&server.name).await
              } else {
                None
              },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::PlayerCheck(target_player) => {
          let (state, peer_server) = match puzzleverse_core::PlayerIdentifier::new(&target_player, Some(&server.name)) {
            puzzleverse_core::PlayerIdentifier::Local(name) => (Some(server.check_player_state(&name, player_name, None).await), None),
            puzzleverse_core::PlayerIdentifier::Remote { server: peer_server, player: target } => {
              match server.peers.read("player_check").await.get(&peer_server) {
                None => (Some(puzzleverse_core::PlayerLocationState::ServerDown), Some(peer_server)),
                Some(peer_id) => match server.peer_states.read("player_check").await.get(peer_id.clone()) {
                  None => (Some(puzzleverse_core::PlayerLocationState::ServerDown), Some(peer_server)),
                  Some(peer_state) => {
                    peer_state
                      .connection
                      .lock("player_check")
                      .await
                      .send(crate::peer::PeerMessage::OnlineStatusRequest { requester: player_name.to_string(), target })
                      .await;
                    (None, None)
                  }
                },
              }
            }
            puzzleverse_core::PlayerIdentifier::Bad => (Some(puzzleverse_core::PlayerLocationState::Invalid), None),
          };
          if let Some(state) = state {
            mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::PlayerState { state, player: target_player }).await;
          }
          if let Some(peer_server) = peer_server {
            server.attempt_peer_server_connection(&peer_server).await;
          }
          false
        }
        puzzleverse_core::ClientRequest::PlayerDelete(target_player) => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            let should_disconnect = &target_player == player_name;
            let server = server.clone();
            tokio::spawn(async move {
              let mut players = server.players.write("player_delete").await;
              let mut player_states = server.player_states.write("player_delete").await;
              let mut realms = server.realms.lock("player_delete").await;
              let mut realm_states = server.realm_states.write("player_delete").await;
              let queue = server.move_queue.lock("player_delete").await;
              let dead_player_id = players.remove(&target_player);
              if let Some(player_id) = dead_player_id {
                player_states.remove(player_id);
              }
              let mut dead_realms = Vec::new();
              for (realm_key, realm_state) in realm_states.iter_mut() {
                if &realm_state.owner == &target_player {
                  let puzzle_state = realm_state.puzzle_state.lock("player_delete").await;
                  for (player_id, _) in puzzle_state.active_players.iter() {
                    if Some(player_id) != dead_player_id.as_ref() {
                      if let Err(e) = queue.send(crate::RealmMove::ToHome(player_id.clone())).await {
                        eprintln!("Failed to send player home: {}", e);
                      }
                    }
                  }
                  dead_realms.push(realm_key.clone());
                  realms.remove(&realm_state.id);
                }
              }
              for realm_key in dead_realms.into_iter() {
                realm_states.remove(realm_key);
              }
              if let Err(e) = server.database.player_delete(db_id) {
                eprintln!("Failed to remove player {}: {}", &target_player, e);
              }
            });
            should_disconnect
          } else {
            false
          }
        }

        puzzleverse_core::ClientRequest::PublicKeyAdd { name, der } => {
          if let Ok(_) = openssl::pkey::PKey::public_key_from_der(&der) {
            if let Err(e) = server.database.public_key_add(db_id, &name, &der) {
              eprintln!("Failed to add public key: {}", e);
            }
          }
          false
        }
        puzzleverse_core::ClientRequest::PublicKeyDelete { name } => {
          if let Err(e) = server.database.public_key_rm(db_id, &name) {
            eprintln!("Failed to delete public key: {}", e);
          }
          false
        }
        puzzleverse_core::ClientRequest::PublicKeyDeleteAll => {
          if let Err(e) = server.database.public_key_rm_all(db_id) {
            eprintln!("Failed to delete all public keys: {}", e);
          }
          false
        }
        puzzleverse_core::ClientRequest::PublicKeyList => {
          match server.database.public_key_list(db_id) {
            Ok(keys) => {
              mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::PublicKeys(keys)).await;
            }
            Err(e) => {
              eprintln!("Failed to get public keys: {}", e);
            }
          }

          false
        }
        puzzleverse_core::ClientRequest::Quit => true,
        puzzleverse_core::ClientRequest::RealmChange { realm } => {
          if let Err(e) = server
            .move_queue
            .lock("request_realm_change")
            .await
            .send(if player_state.debuted.load(std::sync::atomic::Ordering::Relaxed) {
              match realm {
                puzzleverse_core::RealmTarget::Home => crate::RealmMove::ToTrain { player: player.clone(), owner: player_name.into(), train: 0 },
                puzzleverse_core::RealmTarget::LocalRealm(name) => {
                  crate::RealmMove::ToExistingRealm { player: player.clone(), realm: name, server: None }
                }
                puzzleverse_core::RealmTarget::PersonalRealm(asset) => match server.database.realm_create(&asset, player_name, None, None, None) {
                  Ok(name) => crate::RealmMove::ToExistingRealm { player: player.clone(), realm: name, server: None },
                  Err(e) => {
                    eprintln!("Failed to create realm {} for {}: {}", &asset, player_name, e);
                    crate::RealmMove::ToHome(player.clone())
                  }
                },
                puzzleverse_core::RealmTarget::RemoteRealm { realm, server: mut server_name } => {
                  server_name.make_ascii_lowercase();
                  crate::RealmMove::ToExistingRealm {
                    player: player.clone(),
                    realm,
                    server: if &server_name == &server.name { None } else { Some(server_name) },
                  }
                }
              }
            } else {
              match realm {
                puzzleverse_core::RealmTarget::LocalRealm(name) => match server.database.realm_count(&name, db_id) {
                  Err(e) => {
                    eprintln!("Failed to determine if non-debuted player {} can access {}: {}", player_name, &name, e);
                    crate::RealmMove::ToHome(player.clone())
                  }
                  Ok(1) => crate::RealmMove::ToExistingRealm { player: player.clone(), realm: name, server: None },
                  _ => crate::RealmMove::ToHome(player.clone()),
                },
                _ => crate::RealmMove::ToHome(player.clone()),
              }
            })
            .await
          {
            eprintln!("Failed to queue realm change for {}: {}", &player_name, e);
          }
          false
        }
        puzzleverse_core::ClientRequest::RealmCreate { id, name, asset, seed } => {
          let result = server.database.realm_create(&asset, player_name, Some(name), seed, None);
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::RealmCreation {
              id,
              status: match result {
                Ok(id) => puzzleverse_core::RealmCreationStatus::Created(id),
                Err(diesel::result::Error::NotFound) => puzzleverse_core::RealmCreationStatus::Duplicate,
                Err(e) => {
                  eprintln!("Failed to create realm {} for {}: {}", &asset, player_name, e);
                  puzzleverse_core::RealmCreationStatus::InternalError
                }
              },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::RealmDelete { id, target } => {
          let mut realms = server.realms.lock("realm_delete").await;
          let should_delete = if let Some(crate::RealmKind::Loaded(realm_id)) = realms.get(&target) {
            let mut realm_states = server.realm_states.write("realm_delete").await;
            let should_delete = if let Some(realm_state) = realm_states.get(realm_id.clone()) {
              if &realm_state.owner == player_name {
                let puzzle_state = realm_state.puzzle_state.lock("realm_delete").await;
                let links: std::collections::HashMap<_, _> =
                  puzzle_state.active_players.keys().into_iter().map(|p| (p.clone(), puzzleverse_core::asset::rules::RealmLink::Home)).collect();
                server.move_players_from_realm(&realm_state.owner, None, links).await;
                true
              } else {
                false
              }
            } else {
              false
            };
            if should_delete {
              realm_states.remove(realm_id.clone());
            }
            should_delete
          } else {
            false
          };
          if should_delete {
            realms.remove(&target);
          }
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::RealmDeletion {
              id,
              ok: match server.database.realm_delete(&target, db_id) {
                Err(e) => {
                  eprintln!("Failed to delete realm {}: {}", &target, e);
                  false
                }
                Ok(c) => c > 0,
              },
            })
            .await;
          false
        }
        puzzleverse_core::ClientRequest::RealmsList(source) => {
          let results = match source {
            puzzleverse_core::RealmSource::Personal => Some((
              puzzleverse_core::RealmSource::Personal,
              server.database.realm_list(&server.name, crate::database::RealmListScope::Owner(db_id)),
              true,
            )),
            puzzleverse_core::RealmSource::LocalServer => Some((
              puzzleverse_core::RealmSource::LocalServer,
              server.database.realm_list(&server.name, crate::database::RealmListScope::InDirectory),
              true,
            )),
            puzzleverse_core::RealmSource::RemoteServer(peer_name) => match puzzleverse_core::parse_server_name(&peer_name) {
              Some(peer_name) => {
                if &peer_name == &server.name {
                  Some((
                    puzzleverse_core::RealmSource::RemoteServer(peer_name),
                    server.database.realm_list(&server.name, crate::database::RealmListScope::InDirectory),
                    true,
                  ))
                } else {
                  let p = player.clone();
                  server
                    .perform_on_peer_server(peer_name, "realms_list", move |_, peer| {
                      Box::pin(async move {
                        peer.interested_in_list.lock().await.insert(p);
                        peer.connection.lock("list_realms").await.send(crate::peer::PeerMessage::RealmsList).await
                      })
                    })
                    .await;
                  None
                }
              }
              None => Some((puzzleverse_core::RealmSource::RemoteServer(peer_name), vec![], false)),
            },
            puzzleverse_core::RealmSource::Bookmarks => {
              let mut bookmarks: std::collections::BTreeMap<String, Vec<String>> = {
                match server.database.bookmark_get(db_id, &puzzleverse_core::BookmarkType::Realm) {
                  Err(diesel::result::Error::NotFound) => Default::default(),
                  Err(e) => {
                    eprintln!("Failed to fetch realm bookmarks: {}", e);
                    Default::default()
                  }
                  Ok(urls) => {
                    let mut realms = std::collections::BTreeMap::new();
                    for (server, realm) in urls.into_iter().flat_map(|url| {
                      url
                        .parse::<puzzleverse_core::RealmTarget>()
                        .ok()
                        .map(|target| match target {
                          puzzleverse_core::RealmTarget::LocalRealm(id) => Some(("".to_string(), id)),
                          puzzleverse_core::RealmTarget::RemoteRealm { server: peer_server, realm } => {
                            if &peer_server == &server.name {
                              Some(("".to_string(), realm))
                            } else {
                              Some((peer_server, realm))
                            }
                          }
                          _ => None,
                        })
                        .flatten()
                        .into_iter()
                    }) {
                      match realms.entry(server) {
                        std::collections::btree_map::Entry::Vacant(v) => {
                          v.insert(vec![realm]);
                        }
                        std::collections::btree_map::Entry::Occupied(mut o) => {
                          o.get_mut().push(realm);
                        }
                      }
                    }
                    realms
                  }
                }
              };
              let local_bookmarks: std::collections::HashMap<_, _> = bookmarks
                .remove("")
                .map(|ids| {
                  (
                    crate::PeerKey::default(),
                    server.database.realm_list(
                      &server.name,
                      crate::database::RealmListScope::ByPrincipal { ids: ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice() },
                    ),
                  )
                })
                .into_iter()
                .collect();
              let response = std::sync::Arc::new(tokio::sync::Mutex::new(crate::OutstandingId::Bookmarks {
                player: player.clone(),
                results: local_bookmarks.clone(),
              }));

              for (peer_name, ids) in bookmarks.into_iter() {
                let sequence_number = server.id_sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let response = response.clone();
                server
                  .perform_on_peer_server(peer_name, "check_peer_bookmarks", move |_, peer| {
                    Box::pin(async move {
                      peer.interested_in_ids.lock().await.insert(sequence_number, response);
                      peer.connection.lock("check_peer_bookmarks").await.send(crate::peer::PeerMessage::RealmsListIds(sequence_number, ids)).await
                    })
                  })
                  .await;
              }
              Some((puzzleverse_core::RealmSource::Bookmarks, local_bookmarks.values().flat_map(|v| v.iter()).cloned().collect(), true))
            }
            puzzleverse_core::RealmSource::Manual(target) => match target {
              puzzleverse_core::RealmTarget::Home => Some((
                puzzleverse_core::RealmSource::Manual(puzzleverse_core::RealmTarget::Home),
                server.database.realm_list(&server.name, crate::database::RealmListScope::Train { owner: db_id, train: 1 }),
                true,
              )),
              puzzleverse_core::RealmTarget::LocalRealm(id) => {
                let realms = server.database.realm_list(&server.name, crate::database::RealmListScope::ByPrincipal { ids: &[&id] });
                Some((puzzleverse_core::RealmSource::Manual(puzzleverse_core::RealmTarget::LocalRealm(id)), realms, true))
              }
              puzzleverse_core::RealmTarget::PersonalRealm(asset) => {
                let realms = server.database.realm_list(&server.name, crate::database::RealmListScope::ByAsset { owner: db_id, asset: &asset });
                Some((puzzleverse_core::RealmSource::Manual(puzzleverse_core::RealmTarget::PersonalRealm(asset)), realms, true))
              }
              puzzleverse_core::RealmTarget::RemoteRealm { server: peer_server, realm } => {
                let sequence_number = server.id_sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let response = std::sync::Arc::new(tokio::sync::Mutex::new(crate::OutstandingId::Manual {
                  player: player.clone(),
                  server: peer_server.clone(),
                  realm: realm.clone(),
                }));
                server
                  .perform_on_peer_server(peer_server, "bookmarks_peer_realm", move |_, peer| {
                    Box::pin(async move {
                      peer.interested_in_ids.lock().await.insert(sequence_number, response);
                      peer
                        .connection
                        .lock("bookmarks_peer_realm")
                        .await
                        .send(crate::peer::PeerMessage::RealmsListIds(sequence_number, vec![realm]))
                        .await
                    })
                  })
                  .await;
                None
              }
            },
          };
          if let Some((display, mut realms, update_activity)) = results {
            if update_activity {
              server.populate_activity(&mut realms).await;
            }
            mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::RealmsAvailable { display, realms }).await
          }
          false
        }
        puzzleverse_core::ClientRequest::ServersClearBanned(servers) => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            let mut banned = server.banned_peers.write("client_clear_ban").await;
            for server in &servers {
              banned.remove(server);
            }
            if let Err(e) = server.database.banned_peers_remove(&servers) {
              println!("Failed to update database peer bans: {}", e);
            }
          }
          false
        }
        puzzleverse_core::ClientRequest::ServersListBanned => {
          mutable_player_state
            .connection
            .send_local(puzzleverse_core::ClientResponse::ServersBanned(server.banned_peers.read("client_list").await.iter().cloned().collect()))
            .await;
          false
        }
        puzzleverse_core::ClientRequest::ServersSetBanned(servers) => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            let mut banned = server.banned_peers.write("client_clear_ban").await;
            let servers: Vec<_> = servers.into_iter().flat_map(|s| puzzleverse_core::parse_server_name(&s).into_iter()).collect();
            if !servers.is_empty() {
              if let Err(e) = server.database.banned_peers_add(&servers) {
                println!("Failed to update database peer bans: {}", e);
              }
              let mut peers = server.peers.write("add_ban").await;
              let mut peer_states = server.peer_states.write("add_ban").await;
              for server in &servers {
                if let Some(key) = peers.remove(server) {
                  peer_states.remove(key);
                }
              }
              for server in &servers {
                banned.insert(server.clone());
              }
              let server = server.clone();
              tokio::spawn(async move {
                let mut players = server.players.write("add_ban").await;
                let mut player_states = server.player_states.write("add_ban").await;
                let mut dead_players = std::collections::HashSet::new();
                let mut realm_updates = Vec::new();

                for (key, state) in player_states.iter() {
                  if state.server.as_ref().map(|s| servers.iter().any(|ss| ss == s)).unwrap_or(false) {
                    players.remove(&state.principal);
                    if let crate::player_state::Goal::InRealm(realm, _) = state.mutable.lock().await.goal {
                      if let Some(realm_state) = server.realm_states.read("add_ban").await.get(realm) {
                        let mut links = std::collections::hash_map::HashMap::new();
                        realm_state.puzzle_state.lock("add_ban").await.yank(&key, &mut links);
                        realm_updates.push((links, realm_state.owner.clone()));
                      }
                    }
                    dead_players.insert(key);
                  }
                }
                player_states.retain(|key, _| !dead_players.contains(&key));
                for (mut links, owner) in realm_updates {
                  links.retain(|key, _| !dead_players.contains(&key));
                  server.move_players_from_realm(&owner, None, links).await;
                }
              });
            }
          }
          false
        }
        puzzleverse_core::ClientRequest::Servers => {
          let mut active_peers = Vec::new();
          let peer_states = server.peer_states.read("list_servers").await;
          for (peer_name, peer_key) in server.peers.read("list_servers").await.iter() {
            if let Some(peer) = peer_states.get(peer_key.clone()) {
              if let crate::peer::PeerConnection::Online(_) = &*peer.connection.lock("list_online_servers").await {
                active_peers.push(peer_name.clone());
              }
            }
          }
          mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::Servers(active_peers)).await;
          false
        }
        puzzleverse_core::ClientRequest::TrainAdd(asset, allowed_first) => {
          if superuser || {
            let acl = &server.admin_acl.lock().await;
            (*acl).0.check::<&str, _, _>(acl.1.iter(), player_name, &None, &server.name)
          } {
            server.add_train(&asset, allowed_first).await;
          }
          false
        }
      }
    }
    None => true,
  }
}
