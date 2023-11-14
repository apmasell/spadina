use crate::database::Database;
use crate::directory::Directory;
use crate::join_request::JoinRequest;
use crate::peer::message::PeerLocationSearch;
use crate::peer::Peer;
use crate::socket_entity;
use chrono::Duration;
use spadina_core::asset::Asset;
use spadina_core::communication::{DirectMessageStatus, MessageBody};
use spadina_core::location::directory::{Activity, DirectoryEntry};
use spadina_core::location::Descriptor;
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::net::parse_server_name;
use spadina_core::player::OnlineState;
use spadina_core::shared_ref::SharedRef;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::WebSocketStream;

pub enum PeerRequest {
  Activity(SharedRef<str>, oneshot::Sender<Activity>),
  Asset(SharedRef<str>, oneshot::Sender<Asset<String, Vec<u8>>>),
  Available { query: PeerLocationSearch<String>, timeout: Duration, output: watch::Sender<Vec<DirectoryEntry<String>>> },
  CheckOnline { requester: Arc<str>, target: SharedRef<str>, output: oneshot::Sender<OnlineState<SharedRef<str>>> },
  Connect(WebSocketStream<MixedConnection>),
  DirectMessage { sender: SharedRef<str>, recipient: SharedRef<str>, body: MessageBody<String>, status: watch::Sender<DirectMessageStatus> },
  Host(String, JoinRequest),
  Location { player: SharedRef<str>, descriptor: Descriptor<SharedRef<str>>, request: JoinRequest },
  RefreshCalendar { player: String },
}

pub enum PeerDirectoryRequest {
  Peers(oneshot::Sender<Vec<Arc<str>>>),
  Request { server: SharedRef<str>, request: PeerRequest },
}
pub type PeerDirectory = mpsc::Sender<PeerDirectoryRequest>;

pub fn start(database: Database, directory: Directory, mut rx: mpsc::Receiver<PeerDirectoryRequest>) {
  tokio::spawn(async move {
    let mut servers = BTreeMap::<Arc<str>, mpsc::Sender<PeerRequest>>::new();
    loop {
      servers.retain(|_, endpoint| !endpoint.is_closed());
      match rx.recv().await {
        None => break,
        Some(PeerDirectoryRequest::Peers(output)) => {
          let _ = output.send(servers.keys().cloned().collect());
        }
        Some(PeerDirectoryRequest::Request { server, request }) => {
          if let Some(server) = parse_server_name(server.as_ref()) {
            socket_entity::send::<Peer>(Arc::from(server), request, &database, &directory, &mut servers).await;
          }
        }
      };
    }
  });
}
