use futures::SinkExt;

/// Represents a logged in player in the game
///
/// This object tracts the current state information for any player. If a player's connection is cut, this object can persist in memory to allow them to recover their state once they log in again.
pub(crate) struct PlayerState {
  pub(crate) debuted: std::sync::atomic::AtomicBool,
  pub(crate) location_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) message_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) mutable: tokio::sync::Mutex<MutablePlayerState>,
  pub(crate) avatar: crate::prometheus_locks::labelled_rwlock::PrometheusLabelledRwLock<'static, puzzleverse_core::avatar::Avatar>,
  pub(crate) name: String,
  pub(crate) new_realm_access_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) new_realm_admin_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) online_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) principal: String,
  pub(crate) server: Option<String>,
}
pub(crate) struct MutablePlayerState {
  pub(crate) goal: Goal,
  pub(crate) connection: PlayerConnection,
}

#[derive(PartialEq)]
pub(crate) enum Goal {
  Undecided,
  OnPeer(crate::PeerKey, Option<String>),
  InRealm(crate::RealmKey, RealmGoal),
  ResolvingLink(u64),
  WaitingAssetTransfer(crate::RealmKey),
}
#[derive(PartialEq)]
pub(crate) enum RealmGoal {
  Idle,
  ConsensualEmote {
    emote: String,
    epoch: i32,
    initiator: crate::PlayerKey,
    initiator_position: puzzleverse_core::Point,
    recipient_position: puzzleverse_core::Point,
  },
  Follow {
    epoch: i32,
    initiator: crate::PlayerKey,
  },
}

pub(crate) enum PlayerConnection {
  Offline,
  Local(
    i32,
    crate::OutgoingConnection<puzzleverse_core::ClientResponse, puzzleverse_core::net::IncomingConnection>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
  ),
  LocalDead(i32, chrono::DateTime<chrono::Utc>, Vec<puzzleverse_core::ClientResponse>),
  FromPeer(String, crate::PeerKey),
}

impl PlayerConnection {
  pub(crate) async fn release_player(&mut self, server: &crate::Server) {
    match self {
      PlayerConnection::Offline => (),
      PlayerConnection::Local(_, connection, _) => {
        if let Err(e) = connection.send(puzzleverse_core::ClientResponse::InTransit).await {
          eprintln!("Failed to send to client: {}", e);
          *self = PlayerConnection::Offline;
        }
      }
      PlayerConnection::FromPeer(name, peer) => match server.peer_states.read("release_player").await.get(peer.clone()) {
        None => eprintln!("Player on dead server got realm response"),
        Some(state) => {
          state
            .connection
            .lock("send_release")
            .await
            .send(crate::peer::PeerMessage::VisitorRelease(name.clone(), crate::peer::ReleaseTarget::Transit))
            .await;
        }
      },
      PlayerConnection::LocalDead(_, _, _) => {
        *self = PlayerConnection::Offline;
      }
    }
  }

  pub(crate) async fn send(&mut self, server: &crate::Server, player_key: &crate::PlayerKey, response: puzzleverse_core::RealmResponse) {
    match self {
      PlayerConnection::Offline => {
        if let Err(e) = server.move_queue.lock("player_send").await.send(crate::RealmMove::ToHome(player_key.clone())).await {
          eprintln!("Failed to move offline player out of realm: {}", e);
        }
      }
      PlayerConnection::Local(db_id, connection, _) => {
        if let Err(e) = connection.send(puzzleverse_core::ClientResponse::InRealm(response.clone())).await {
          eprintln!("Write to player failed: {}", &e);
          match e {
            crate::PacketEncodingError::Tungstenite(
              tokio_tungstenite::tungstenite::Error::AlreadyClosed
              | tokio_tungstenite::tungstenite::Error::ConnectionClosed
              | tokio_tungstenite::tungstenite::Error::Io(_),
            ) => {
              *self = PlayerConnection::LocalDead(*db_id, chrono::Utc::now(), vec![puzzleverse_core::ClientResponse::InRealm(response)]);
            }
            _ => (),
          }
        }
      }
      PlayerConnection::LocalDead(_, time, queue) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(2) {
          queue.push(puzzleverse_core::ClientResponse::InRealm(response))
        } else {
          *self = PlayerConnection::Offline;
          if let Err(e) = server.move_queue.lock("player_send").await.send(crate::RealmMove::ToHome(player_key.clone())).await {
            eprintln!("Failed to move dead (now offline) player out of realm: {}", e);
          }
        }
      }
      PlayerConnection::FromPeer(name, peer) => match server.peer_states.write("send_to_player").await.get(peer.clone()) {
        None => eprintln!("Player on dead server got realm response"),
        Some(state) => {
          state.connection.lock("send_realm_response").await.send(crate::peer::PeerMessage::RealmResponse { player: name.clone(), response }).await;
        }
      },
    }
  }
  pub(crate) async fn send_assets<I: IntoIterator<Item = String>>(&mut self, server: &crate::Server, assets: I) {
    match self {
      PlayerConnection::Offline => (),
      PlayerConnection::Local(db_id, connection, _) => {
        let asset_list: Vec<_> = assets.into_iter().collect();
        if let Err(e) = connection.send(puzzleverse_core::ClientResponse::CheckAssets { asset: asset_list.clone() }).await {
          eprintln!("Write assets to player failed: {}", &e);
          match e {
            crate::PacketEncodingError::Tungstenite(
              tokio_tungstenite::tungstenite::Error::AlreadyClosed
              | tokio_tungstenite::tungstenite::Error::ConnectionClosed
              | tokio_tungstenite::tungstenite::Error::Io(_),
            ) => {
              *self =
                PlayerConnection::LocalDead(*db_id, chrono::Utc::now(), vec![puzzleverse_core::ClientResponse::CheckAssets { asset: asset_list }]);
            }
            _ => (),
          }
        }
      }
      PlayerConnection::LocalDead(_, time, queue) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(2) {
          queue.push(puzzleverse_core::ClientResponse::CheckAssets { asset: assets.into_iter().collect() })
        } else {
          *self = PlayerConnection::Offline;
        }
      }
      PlayerConnection::FromPeer(name, peer) => match server.peer_states.write("send_assets_to_player").await.get(peer.clone()) {
        None => eprintln!("Player on dead server got assets request"),
        Some(state) => {
          state
            .connection
            .lock("send_assets")
            .await
            .send(crate::peer::PeerMessage::VisitorCheckAssets { player: name.clone(), assets: assets.into_iter().collect() })
            .await;
        }
      },
    }
  }
  pub(crate) async fn send_change(&mut self, server: &crate::Server, change: puzzleverse_core::RealmChange) {
    match self {
      PlayerConnection::Offline => (),
      PlayerConnection::Local(db_id, connection, _) => {
        if let Err(e) = connection.send(puzzleverse_core::ClientResponse::RealmChanged(change.clone())).await {
          eprintln!("Write to player failed: {}", &e);
          match e {
            crate::PacketEncodingError::Tungstenite(
              tokio_tungstenite::tungstenite::Error::AlreadyClosed
              | tokio_tungstenite::tungstenite::Error::ConnectionClosed
              | tokio_tungstenite::tungstenite::Error::Io(_),
            ) => {
              *self = PlayerConnection::LocalDead(*db_id, chrono::Utc::now(), vec![puzzleverse_core::ClientResponse::RealmChanged(change)]);
            }
            _ => (),
          }
        }
      }
      PlayerConnection::LocalDead(_, time, queue) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(2) {
          queue.push(puzzleverse_core::ClientResponse::RealmChanged(change))
        } else {
          *self = PlayerConnection::Offline;
        }
      }
      PlayerConnection::FromPeer(name, peer) => match server.peer_states.write("send_change_to_player").await.get(peer.clone()) {
        None => eprintln!("Player on dead server got realm response"),
        Some(state) => {
          state.connection.lock("send_realm_changed").await.send(crate::peer::PeerMessage::RealmChanged { player: name.clone(), change }).await;
        }
      },
    }
  }
  pub(crate) async fn send_local(&mut self, message: puzzleverse_core::ClientResponse) {
    match self {
      PlayerConnection::Offline => eprintln!("Offline player got local response."),
      PlayerConnection::Local(db_id, connection, _) => {
        if let Err(e) = connection.send(message.clone()).await {
          eprintln!("Failed to write to player: {}", e);
          match e {
            crate::PacketEncodingError::Tungstenite(
              tokio_tungstenite::tungstenite::Error::AlreadyClosed
              | tokio_tungstenite::tungstenite::Error::ConnectionClosed
              | tokio_tungstenite::tungstenite::Error::Io(_),
            ) => {
              *self = PlayerConnection::LocalDead(*db_id, chrono::Utc::now(), vec![message]);
            }
            _ => (),
          }
        }
      }
      PlayerConnection::LocalDead(_, time, queue) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(2) {
          queue.push(message)
        } else {
          *self = PlayerConnection::Offline;
        }
      }
      PlayerConnection::FromPeer(_, _) => eprintln!("Player on peer got local response."),
    }
  }
}
