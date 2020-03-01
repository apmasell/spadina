use futures::SinkExt;

use crate::{prometheus_locks, OutgoingConnection, OutstandingId, PlayerKey};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct PeerClaim {
  pub(crate) exp: usize,
  pub(crate) name: String,
}

pub(crate) enum PeerConnection {
  Online(OutgoingConnection<PeerMessage, hyper::upgrade::Upgraded>),
  Dead(chrono::DateTime<chrono::Utc>, Vec<PeerMessage>),
  Offline,
}

pub struct PeerState {
  pub(crate) connection: prometheus_locks::labelled_mutex::PrometheusLabelledMutex<'static, PeerConnection>,
  pub(crate) interested_in_list: tokio::sync::Mutex<std::collections::HashSet<PlayerKey>>,
  pub(crate) interested_in_ids: tokio::sync::Mutex<std::collections::HashMap<i32, std::sync::Arc<tokio::sync::Mutex<OutstandingId>>>>,
  pub(crate) name: String,
}

/// Messages exchanged between servers; all though there is a client/server relationship implied by Web Sockets, the connection is peer-to-peer, therefore, there is no distinction between requests and responses in this structure
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PeerMessage {
  /// Request assets from this server if they are available
  AssetsPull { assets: Vec<String> },
  /// Send assets requested by the other server
  AssetsPush { assets: std::collections::HashMap<String, puzzleverse_core::asset::Asset> },
  /// Change the avatar of a player
  AvatarSet { player: String, avatar: puzzleverse_core::avatar::Avatar },
  /// Transfer direct messages
  DirectMessage(Vec<PeerDirectMessage>),
  /// Check the online status of a player
  OnlineStatusRequest { requester: String, target: String },
  /// Send the online status of a player
  OnlineStatusResponse { requester: String, target: String, state: puzzleverse_core::PlayerLocationState },
  /// Indicate that a realm change has occurred; if the realm change was not successful, the peer server has relinquished control of the player
  RealmChanged { player: String, change: puzzleverse_core::RealmChange },
  /// Process a realm-related request for a player that has been handed off to this server
  RealmRequest { player: String, request: puzzleverse_core::RealmRequest },
  /// Receive a realm-related response for a player that has been handed off to this server
  RealmResponse { player: String, response: puzzleverse_core::RealmResponse },
  /// List realms that are in the public directory for this server
  RealmsList,
  /// List realms that exist for the given realm identifiers
  RealmsListIds(i32, Vec<String>),
  /// The realms that are available in the public directory on this server
  RealmsAvailable(Vec<puzzleverse_core::Realm>),
  /// The realms that are available when queried by identifier
  RealmsAvailableIds(i32, Vec<puzzleverse_core::Realm>),
  /// For a visitor, indicate what assets the client will require
  VisitorCheckAssets { player: String, assets: Vec<String> },
  /// Releases control of a player to the originating server
  VisitorRelease(String, ReleaseTarget),
  /// Send player to a realm on the destination server
  ///
  /// This transfers control of that player to the peer server until the originating server yanks them back or the destination server send them back
  VisitorSend { player: String, realm: String, avatar: puzzleverse_core::avatar::Avatar },
  /// Forces a player to be removed from a peer server by the originating server
  VisitorYank(String),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerDirectMessage {
  pub sender: String,
  pub recipient: String,
  pub timestamp: chrono::DateTime<chrono::Utc>,
  pub body: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerConnectRequest {
  pub(crate) token: String,
  pub(crate) server: String,
}

impl PeerConnection {
  pub async fn send(&mut self, message: PeerMessage) -> () {
    match self {
      PeerConnection::Online(_) => {}
      PeerConnection::Dead(time, queued) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(5) {
          queued.push(message);
        } else {
          *self = PeerConnection::Offline;
        }
      }
      PeerConnection::Offline => eprintln!("Ignoring message to offline server"),
    }
  }
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum ReleaseTarget {
  Transit,
  Home,
  Realm(String, String),
}

pub async fn process_server_message(server: &std::sync::Arc<crate::Server>, server_name: &str, peer_id: &crate::PeerKey, req: PeerMessage) {
  async fn resolve_player_from_peer(
    server: &std::sync::Arc<crate::Server>,
    peer_id: &crate::PeerKey,
    player: &str,
    server_name: &str,
    avatar: Option<puzzleverse_core::avatar::Avatar>,
  ) -> PlayerKey {
    match server.players.write("resolve_player_from_peer").await.entry(format!("{}@{}", &player, server_name)) {
      std::collections::hash_map::Entry::Occupied(o) => {
        if let Some(avatar) = avatar {
          if let Some(state) = server.player_states.read("resolve_player_from_peer").await.get(o.get().clone()) {
            *state.avatar.write("resolve_player_from_peer").await = avatar;
          }
        }
        o.get().clone()
      }
      std::collections::hash_map::Entry::Vacant(v) => {
        let principal = format!("{}@{}", &player, server_name);
        let player_key = server.player_states.write("resolve_player_from_peer").await.insert(crate::player_state::PlayerState {
          avatar: crate::PLAYER_AVATAR_LOCK.create(principal.clone(), avatar.unwrap_or_default()),
          principal,
          name: player.to_string(),
          debuted: true.into(),
          server: Some(server_name.into()),
          mutable: tokio::sync::Mutex::new(crate::player_state::MutablePlayerState {
            goal: crate::player_state::Goal::Undecided,
            connection: crate::player_state::PlayerConnection::FromPeer(player.to_string(), peer_id.clone()),
          }),
          message_acl: std::sync::Arc::new(tokio::sync::Mutex::new((puzzleverse_core::AccessDefault::Allow, vec![]))),
          online_acl: std::sync::Arc::new(tokio::sync::Mutex::new((puzzleverse_core::AccessDefault::Allow, vec![]))),
          location_acl: std::sync::Arc::new(tokio::sync::Mutex::new((puzzleverse_core::AccessDefault::Allow, vec![]))),
          new_realm_access_acl: std::sync::Arc::new(tokio::sync::Mutex::new((puzzleverse_core::AccessDefault::Allow, vec![]))),
          new_realm_admin_acl: std::sync::Arc::new(tokio::sync::Mutex::new((puzzleverse_core::AccessDefault::Allow, vec![]))),
        });
        v.insert(player_key.clone());
        player_key
      }
    }
  }
  match req {
    PeerMessage::AssetsPull { assets } => {
      let mut output = std::collections::HashMap::new();
      for asset in assets {
        if let Ok(value) = server.asset_store.pull(&asset).await {
          output.insert(asset, value);
        }
      }
      if output.len() > 0 {
        if let Some(peer_state) = server.peer_states.read("asset_pull").await.get(peer_id.clone()) {
          peer_state.connection.lock("asset_pull").await.send(PeerMessage::AssetsPush { assets: output }).await;
        }
      }
    }
    PeerMessage::AssetsPush { assets } => {
      server.receive_asset(server_name, assets).await;
    }
    PeerMessage::AvatarSet { avatar, player } => {
      if let Some(key) = server.players.write("avatar_set_peer").await.get(&format!("{}@{}", &player, server_name)) {
        if let Some(state) = server.player_states.read("avatar_set_peer").await.get(key.clone()) {
          *state.avatar.write("avatar_set_peer").await = avatar;
        }
      }
    }
    PeerMessage::DirectMessage(messages) => match server.database.remote_direct_messages_receive(&server_name, messages) {
      Err(e) => eprintln!("Failed to get users to write messages from {}: {}", &server_name, e),

      Ok(written) => {
        let mut output = std::collections::HashMap::new();
        for (mut sender, recipient, body, timestamp) in written {
          sender.push('@');
          sender.push_str(&server_name);
          (match (match output.entry(recipient) {
            std::collections::hash_map::Entry::Vacant(e) => e.insert(std::collections::HashMap::new()),
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
          })
          .entry(sender)
          {
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Vec::new()),
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
          })
          .push(puzzleverse_core::DirectMessage { inbound: true, body, timestamp });
        }
        for (recipient, data) in output.drain() {
          if let Some(player_id) = server.players.read("peer_direct_message").await.get(&recipient) {
            if let Some(player_state) = server.player_states.read("peer_direct_message").await.get(player_id.clone()) {
              let mut mutable_player_state = player_state.mutable.lock().await;
              for (sender, messages) in data {
                mutable_player_state.connection.send_local(puzzleverse_core::ClientResponse::DirectMessages { player: sender, messages }).await;
              }
            }
          }
        }
      }
    },
    PeerMessage::OnlineStatusRequest { requester, target } => {
      let state = server.check_player_state(&target, &requester, Some(server_name)).await;
      if let Some(peer_state) = server.peer_states.read("peer_online_status_request").await.get(peer_id.clone()) {
        peer_state.connection.lock("online_status_request").await.send(PeerMessage::OnlineStatusResponse { requester, target, state }).await;
      }
    }
    PeerMessage::OnlineStatusResponse { requester, mut target, state } => {
      if let Some(player_id) = server.players.read("peer_online_status_response").await.get(&requester) {
        if let Some(player_state) = server.player_states.read("peer_online_status_response").await.get(player_id.clone()) {
          target.push('@');
          target.push_str(&server_name);
          player_state.mutable.lock().await.connection.send_local(puzzleverse_core::ClientResponse::PlayerState { player: target, state }).await;
        }
      }
    }
    PeerMessage::RealmChanged { player, change } => {
      let missing_capabilities: Vec<_> = match &change {
        puzzleverse_core::RealmChange::Denied => vec![],
        puzzleverse_core::RealmChange::Success { capabilities, .. } => {
          capabilities.iter().filter(|c| !puzzleverse_core::CAPABILITIES.contains(&c.as_str())).collect()
        }
      };
      if missing_capabilities.is_empty() {
        server
          .send_response_to_player_visiting_peer(
            peer_id,
            &server_name,
            &player,
            match &change {
              puzzleverse_core::RealmChange::Success { .. } => None,
              puzzleverse_core::RealmChange::Denied => Some(crate::player_state::Goal::Undecided),
            },
            puzzleverse_core::ClientResponse::RealmChanged(change),
          )
          .await;
      } else {
        server
          .send_response_to_player_visiting_peer(
            peer_id,
            &server_name,
            &player,
            Some(crate::player_state::Goal::Undecided),
            puzzleverse_core::ClientResponse::RealmChanged(puzzleverse_core::RealmChange::Denied),
          )
          .await;
        if let Some(peer_state) = server.peer_states.read("realm_changed").await.get(peer_id.clone()) {
          peer_state.connection.lock("realm_changed").await.send(PeerMessage::VisitorYank(player)).await;
        }
      }
    }
    PeerMessage::RealmRequest { player, request } => {
      let player_id = resolve_player_from_peer(&server, &peer_id, &player, &server_name, None).await;
      if server.process_realm_request(&player, &player_id, Some(&server_name), false, request).await {
        let player_states = server.player_states.read("peer_realm_request").await;
        let mut mutable_player_state = player_states.get(player_id).unwrap().mutable.lock().await;
        mutable_player_state.goal = crate::player_state::Goal::Undecided;
        mutable_player_state.connection.release_player(server).await;
      }
    }
    PeerMessage::RealmResponse { player, response } => {
      server.send_response_to_player_visiting_peer(peer_id, &server_name, &player, None, puzzleverse_core::ClientResponse::InRealm(response)).await
    }
    PeerMessage::RealmsAvailable(realms) => {
      if let Some(peer_state) = server.peer_states.read("realms_available").await.get(peer_id.clone()) {
        for player in peer_state.interested_in_list.lock().await.drain() {
          if let Some(player_state) = server.player_states.read("peer_realms_available").await.get(player) {
            player_state
              .mutable
              .lock()
              .await
              .connection
              .send_local(puzzleverse_core::ClientResponse::RealmsAvailable {
                display: puzzleverse_core::RealmSource::RemoteServer(server_name.to_string()),
                realms: realms.clone(),
              })
              .await;
          }
        }
      }
    }
    PeerMessage::RealmsAvailableIds(id, realms) => {
      if let Some(peer_state) = server.peer_states.read("realms_available_ids").await.get(peer_id.clone()) {
        if let Some(outstanding) = peer_state.interested_in_ids.lock().await.remove(&id) {
          let mut outstanding = outstanding.lock().await;
          let (player, response) = match &mut *outstanding {
            OutstandingId::Bookmarks { player, results } => {
              results.insert(peer_id.clone(), realms);
              (
                player.clone(),
                puzzleverse_core::ClientResponse::RealmsAvailable {
                  display: puzzleverse_core::RealmSource::Bookmarks,
                  realms: results.values().flatten().cloned().collect(),
                },
              )
            }
            OutstandingId::Manual { player, server, realm } => (
              player.clone(),
              puzzleverse_core::ClientResponse::RealmsAvailable {
                display: puzzleverse_core::RealmSource::Manual(puzzleverse_core::RealmTarget::RemoteRealm {
                  server: server.clone(),
                  realm: realm.clone(),
                }),
                realms,
              },
            ),
          };
          if let Some(player_state) = server.player_states.read("peer_realms_available_ids").await.get(player) {
            player_state.mutable.lock().await.connection.send_local(response).await;
          }
        }
      }
    }
    PeerMessage::RealmsList => {
      if let Some(peer_state) = server.peer_states.read("realm_list").await.get(peer_id.clone()) {
        let mut realms = server.database.realm_list(&server.name, crate::database::RealmListScope::InDirectory);
        server.populate_activity(&mut realms).await;
        peer_state.connection.lock("realms_list").await.send(PeerMessage::RealmsAvailable(realms)).await;
      }
    }
    PeerMessage::RealmsListIds(id, ids) => {
      if let Some(peer_state) = server.peer_states.read("realm_list_ids").await.get(peer_id.clone()) {
        let mut realms = server.database.realm_list(
          &server.name,
          crate::database::RealmListScope::ByPrincipal { ids: ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice() },
        );
        server.populate_activity(&mut realms).await;
        peer_state.connection.lock("realms_list_ids").await.send(PeerMessage::RealmsAvailableIds(id, realms)).await;
      }
    }
    PeerMessage::VisitorCheckAssets { player, assets } => {
      let mut missing_assets = Vec::new();
      for a in assets.iter() {
        if server.asset_store.missing(a).await {
          missing_assets.push(a.clone());
        }
      }
      if missing_assets.len() > 0 {
        if let Some(peer_state) = server.peer_states.read("visitor_check_assets").await.get(peer_id.clone()) {
          peer_state.connection.lock("visitor_check_assets").await.send(PeerMessage::AssetsPull { assets: missing_assets }).await;
        }
      }
      server
        .send_response_to_player_visiting_peer(peer_id, &server_name, &player, None, puzzleverse_core::ClientResponse::CheckAssets { asset: assets })
        .await
    }

    PeerMessage::VisitorRelease(player, target) => {
      if let Some(player_id) = server.players.read("visitor_release").await.get(&player) {
        if let Some(playerstate) = server.player_states.read("visitor_release").await.get(*player_id) {
          let mut playerstate_mutable = playerstate.mutable.lock().await;
          match playerstate_mutable.goal {
            crate::player_state::Goal::OnPeer(peer_state, _) => {
              if peer_id == &peer_state {
                let mut set_dead = None;
                if let crate::player_state::PlayerConnection::Local(db_id, connection, _) = &mut playerstate_mutable.connection {
                  if let Err(e) = connection.send(puzzleverse_core::ClientResponse::InTransit).await {
                    eprintln!("Failed to send to player {}: {}", &playerstate.principal, e);
                    set_dead = Some(*db_id);
                  }
                  let (new_goal, change) = server
                    .resolve_realm(
                      &player_id,
                      &player,
                      &*playerstate.avatar.read("visitor_release").await,
                      Some(server_name.to_string()),
                      *db_id,
                      target,
                    )
                    .await;
                  playerstate_mutable.goal = new_goal;
                  if let Some(change) = change {
                    playerstate_mutable.connection.send_change(server, change).await;
                  }
                }
                if let Some(db_id) = set_dead {
                  playerstate_mutable.connection =
                    crate::player_state::PlayerConnection::LocalDead(db_id, chrono::Utc::now(), vec![puzzleverse_core::ClientResponse::InTransit]);
                }
              }
            }
            _ => (),
          }
        }
      }
    }
    PeerMessage::VisitorSend { player, realm, avatar } => {
      let allowed = {
        let access_acl = server.access_acl.lock().await;
        access_acl.0.check(access_acl.1.iter(), &player, &Some(server_name), &server.name)
      };
      if allowed {
        let player_id = resolve_player_from_peer(&server, &peer_id, &player, &server_name, Some(avatar)).await;
        if let Err(e) =
          server.move_queue.lock("visitor_send").await.send(crate::RealmMove::ToExistingRealm { player: player_id, realm, server: None }).await
        {
          eprintln!("Failed to move player {} to new realm from server {}: {}", &player, &server_name, e);
        }
      } else {
        if let Some(peer_state) = server.peer_states.read("visitor_send").await.get(peer_id.clone()) {
          peer_state.connection.lock("visitor_send").await.send(PeerMessage::VisitorRelease(player, ReleaseTarget::Transit)).await;
        }
      }
    }
    PeerMessage::VisitorYank(player) => {
      if let Some(player_id) = server.players.read("visitor_yank").await.get(&format!("{}@{}", &player, server_name)) {
        if let Some(mut playerstate) = match server.player_states.read("visitor_yank").await.get(*player_id) {
          Some(m) => Some(m.mutable.lock().await),
          None => None,
        } {
          match playerstate.goal {
            crate::player_state::Goal::InRealm(realm, _) => {
              if let Some(realm_state) = server.realm_states.read("visitor_yank").await.get(realm) {
                let mut puzzle_state = realm_state.puzzle_state.lock("visitor_yank").await;
                let mut links = std::collections::hash_map::HashMap::new();
                links.insert(player_id.clone(), puzzleverse_core::asset::rules::RealmLink::Home);
                puzzle_state.yank(player_id, &mut links);
                server.move_players_from_realm(&realm_state.owner, None, links).await;
              }
            }
            _ => (),
          }
          playerstate.goal = crate::player_state::Goal::OnPeer(peer_id.clone(), None);
        }
      }
    }
  }
}
