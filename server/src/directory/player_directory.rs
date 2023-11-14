use crate::client::Client;
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use crate::directory::location_endpoint::LocationEndpoint;
use crate::directory::Directory;
use crate::join_request::JoinRequest;
use crate::player_location_update::PlayerLocationUpdate;
use crate::socket_entity;
use chrono::{DateTime, Utc};
use spadina_core::access::OnlineAccess;
use spadina_core::communication::{DirectMessageStatus, MessageBody};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::Activity;
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::player::{OnlineState, PlayerIdentifier};
use spadina_core::shared_ref::SharedRef;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::WebSocketStream;

pub enum PlayerRequest {
  Check(PlayerIdentifier<SharedRef<str>>, oneshot::Sender<OnlineState<SharedRef<str>>>),
  Connect(WebSocketStream<MixedConnection>),
  DirectMessage(PlayerIdentifier<SharedRef<str>>, MessageBody<String>, DateTime<Utc>),
}
pub enum PlayerDirectoryRequest {
  Activity(SharedRef<str>, oneshot::Sender<Activity>),
  Check(PlayerIdentifier<SharedRef<str>>, SharedRef<str>, oneshot::Sender<OnlineState<SharedRef<str>>>),
  Connect(Arc<str>, WebSocketStream<MixedConnection>),
  DirectMessage {
    recipient: SharedRef<str>,
    sender: PlayerIdentifier<SharedRef<str>>,
    body: MessageBody<String>,
    status: watch::Sender<DirectMessageStatus>,
  },
  Host(Arc<str>, LocationEndpoint),
  Join(SharedRef<str>, JoinRequest),
}
pub type PlayerDirectory = mpsc::Sender<PlayerDirectoryRequest>;

pub fn start(database: Database, directory: Directory, mut rx: mpsc::Receiver<PlayerDirectoryRequest>) {
  tokio::spawn(async move {
    let mut players = BTreeMap::<Arc<str>, mpsc::Sender<PlayerRequest>>::new();
    let mut hosting = BTreeMap::<Arc<str>, LocationEndpoint>::new();
    loop {
      players.retain(|_, connection| !connection.is_closed());
      hosting.retain(|_, endpoint| !endpoint.is_closed());
      match rx.recv().await {
        None => break,
        Some(PlayerDirectoryRequest::Activity(host, output)) => {
          let _ = output.send(hosting.get(host.as_ref()).map(|endpoint| endpoint.activity()).unwrap_or(Activity::Deserted));
        }
        Some(PlayerDirectoryRequest::Check(requester, recipient, output)) => match players.get_mut(recipient.as_ref()) {
          Some(handle) => {
            let _ = handle.send(PlayerRequest::Check(requester, output)).await;
          }
          None => {
            let _ =
              output.send(match database.player_acl(PlayerReference::Name(recipient.as_ref()), crate::database::schema::player::dsl::online_acl) {
                Ok(Some(acl)) => {
                  if acl.check(&requester, &directory.access_management.server_name) == OnlineAccess::Deny {
                    OnlineState::Unknown
                  } else {
                    OnlineState::Offline
                  }
                }
                Ok(None) => OnlineState::Invalid,
                Err(e) => {
                  eprintln!("Failed to get online ACL for {}: {}", &recipient, e);
                  OnlineState::Unknown
                }
              });
          }
        },
        Some(PlayerDirectoryRequest::Connect(player, connection)) => {
          socket_entity::send::<Client>(player, PlayerRequest::Connect(connection), &database, &directory, &mut players).await;
        }
        Some(PlayerDirectoryRequest::DirectMessage { recipient, sender, body, status }) => {
          let result = match &sender {
            PlayerIdentifier::Local(sender) => database.direct_message_write(sender.as_ref(), recipient.as_ref(), &body),
            PlayerIdentifier::Remote { player, server } => {
              database.remote_direct_message_write(PlayerReference::Name(recipient.as_ref()), player.as_ref(), server.as_ref(), &body, None)
            }
          };
          let (update, forward) = match result {
            Ok(timestamp) => (DirectMessageStatus::Delivered(timestamp), Some(PlayerRequest::DirectMessage(sender, body, timestamp))),
            Err(diesel::result::Error::NotFound) => (DirectMessageStatus::UnknownRecipient, None),
            Err(e) => {
              eprintln!("Failed to send direct message to {}: {}", recipient.as_ref(), e);
              (DirectMessageStatus::InternalError, None)
            }
          };
          let _ = status.send(update);
          if let Some(request) = forward {
            if let Some(handle) = players.get_mut(recipient.as_ref()) {
              let _ = handle.send(request).await;
            }
          }
        }
        Some(PlayerDirectoryRequest::Host(player, endpoint)) => {
          hosting.insert(player, endpoint);
        }
        Some(PlayerDirectoryRequest::Join(player, request)) => match hosting.get_mut(player.as_ref()) {
          None => {
            let _ = request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::ResolutionError));
          }
          Some(endpoint) => {
            if let Err(request) = endpoint.join(request) {
              let _ = request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::ResolutionError));
            }
          }
        },
      }
    }
  });
}
