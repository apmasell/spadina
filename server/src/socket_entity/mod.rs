use crate::database::Database;
use crate::directory::Directory;
use crate::http_server::jwt::decode_jwt;
use crate::http_server::{websocket, WebServer};
use diesel::QueryResult;
use futures::stream::BoxStream;
use futures::stream::SelectAll;
use futures::Stream;
use futures::StreamExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{body, http, Response, StatusCode};
use serde::de::DeserializeOwned;
use spadina_core::net::mixed_connection::MixedConnection;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::WebSocketStream;

pub mod net;

pub trait SocketEntity: Stream + Sized + Unpin + Send + 'static
where
  Self::Item: Send + 'static,
{
  const DIRECTORY_QUEUE_DEPTH: usize;
  type Claim: DeserializeOwned + Send + 'static;
  type DirectoryRequest: Send + 'static;
  type ExternalRequest: DeserializeOwned + Send + 'static;
  fn establish(claim: Self::Claim, connection: WebSocketStream<MixedConnection>, directory: Directory)
    -> impl Future<Output = Result<(), ()>> + Send;
  fn new(name: Arc<str>, database: &Database) -> QueryResult<Self>;
  fn process(
    &mut self,
    incoming: Incoming<Self>,
    directory: &Directory,
    database: &Database,
    connection_state: ConnectionState,
  ) -> impl Future<Output = Vec<Outgoing<Self>>> + Send;
  fn show_decode_error(&self, error: rmp_serde::decode::Error);
  fn show_socket_error(&self, error: Error);
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ConnectionState {
  Disconnected,
  ConnectedWeb,
  ConnectedUnix,
}

pub enum Incoming<Entity: SocketEntity + ?Sized>
where
  Entity::Item: Send + 'static,
{
  Delayed(Entity::Item),
  Directory(Entity::DirectoryRequest),
  External(Entity::ExternalRequest),
  StateChange,
}

pub enum Outgoing<Entity: SocketEntity + ?Sized>
where
  Entity::Item: Send + 'static,
{
  Connect(WebSocketStream<MixedConnection>),
  Break,
  SideTask(BoxStream<'static, Vec<Outgoing<Entity>>>),
  Send(Message),
}
pub async fn send<Entity: SocketEntity>(
  name: Arc<str>,
  request: Entity::DirectoryRequest,
  database: &Database,
  directory: &Directory,
  active: &mut BTreeMap<Arc<str>, mpsc::Sender<Entity::DirectoryRequest>>,
) where
  Entity::Item: Send + 'static,
{
  match active.entry(name) {
    Entry::Vacant(v) => match start::<Entity>(v.key().clone(), database.clone(), directory.clone()) {
      Ok(sender) => {
        let _ = v.insert(sender).send(request).await;
      }
      Err(e) => {
        eprintln!("Failed to initialize {}: {}", v.key(), e);
      }
    },
    Entry::Occupied(mut o) => {
      let _ = o.get_mut().send(request).await;
    }
  };
}
pub fn start<Entity: SocketEntity>(name: Arc<str>, database: Database, directory: Directory) -> QueryResult<mpsc::Sender<Entity::DirectoryRequest>>
where
  Entity::Item: Send + 'static,
{
  let (tx, rx) = mpsc::channel(Entity::DIRECTORY_QUEUE_DEPTH);
  let entity = Entity::new(name, &database)?;
  tokio::spawn(run(entity, rx, directory, database));
  Ok(tx)
}
async fn run<Entity: SocketEntity>(
  mut entity: Entity,
  mut directory_input: mpsc::Receiver<Entity::DirectoryRequest>,
  directory: Directory,
  database: Database,
) where
  Entity::Item: Send + 'static,
{
  enum Next<Entity: SocketEntity>
  where
    Entity::Item: Send + 'static,
  {
    Directory(Entity::DirectoryRequest),
    Error(Error),
    External(Message),
    Ignore,
    Internal(Entity::Item),
    StateChange,
    TaskResult(Vec<Outgoing<Entity>>),
  }
  impl<Entity: SocketEntity> From<Option<Result<Message, Error>>> for Next<Entity>
  where
    Entity::Item: Send + 'static,
  {
    fn from(value: Option<Result<Message, Error>>) -> Self {
      match value {
        None => Next::StateChange,
        Some(Ok(message)) => Next::External(message),
        Some(Err(e)) => Next::Error(e),
      }
    }
  }

  let mut connection = net::EntityConnection::Dead(chrono::Utc::now(), Vec::new());
  let mut death = directory.access_management.give_me_death();
  let mut side_tasks = SelectAll::new();
  loop {
    let message = tokio::select! {
      biased;
      _ = death.recv() => Next::TaskResult(vec![Outgoing::Break]),
      r = directory_input.recv() => r.map(Next::Directory).unwrap_or(Next::TaskResult(vec![Outgoing::Break])),
      r = side_tasks.next(), if !side_tasks.is_empty() => r.map(Next::TaskResult).unwrap_or(Next::Ignore),
      Some(r) = entity.next() => Next::Internal(r),
      r = connection.next() => Next::from(r),

    };
    let outgoing = match message {
      Next::Directory(request) => entity.process(Incoming::Directory(request), &directory, &database, connection.connection_state()).await,
      Next::Ignore => continue,
      Next::Internal(delayed) => entity.process(Incoming::Delayed(delayed), &directory, &database, connection.connection_state()).await,
      Next::Error(e) => {
        entity.show_socket_error(e);
        continue;
      }
      Next::External(message) => match message {
        Message::Binary(message) => match rmp_serde::from_slice::<Entity::ExternalRequest>(&message) {
          Ok(message) => entity.process(Incoming::External(message), &directory, &database, connection.connection_state()).await,
          Err(e) => {
            entity.show_decode_error(e);
            continue;
          }
        },
        Message::Ping(message) => vec![Outgoing::Send(Message::Pong(message))],
        _ => continue,
      },
      Next::StateChange => entity.process(Incoming::StateChange, &directory, &database, connection.connection_state()).await,
      Next::TaskResult(outgoing) => outgoing,
    };
    for outgoing in outgoing {
      match outgoing {
        Outgoing::Connect(new_connection) => {
          if let Err(e) = connection.establish(new_connection).await {
            entity.show_socket_error(e);
          }
        }
        Outgoing::Break => break,
        Outgoing::SideTask(task) => side_tasks.push(task),
        Outgoing::Send(message) => {
          if let Err(e) = connection.send(message).await {
            entity.show_socket_error(e);
          }
        }
      }
    }
  }
}
pub fn open_websocket<Entity: SocketEntity>(req: hyper::Request<body::Incoming>, web_server: &WebServer) -> Result<Response<Full<Bytes>>, http::Error>
where
  Entity::Item: Send + 'static,
{
  // Check whether they provided a valid Authorization: Bearer header
  let Some(header_value) = req.headers().get(http::header::AUTHORIZATION) else {
    crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
    return Response::builder().status(StatusCode::UNAUTHORIZED).body("No Authorization header".into());
  };
  let header_value = match header_value.to_str() {
    Ok(v) => v,
    Err(e) => {
      crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
      return Response::builder().status(StatusCode::UNAUTHORIZED).body(e.to_string().into());
    }
  };
  if !header_value.starts_with("Bearer ") {
    crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
    return Response::builder().status(StatusCode::UNAUTHORIZED).body("Only bearer authentication is supported".into());
  }
  let claim = match decode_jwt::<Entity::Claim>(&header_value[7..], &web_server.directory.access_management) {
    Ok(claim) => claim,
    Err(e) => return e,
  };

  let is_http_11 = req.version() == http::Version::HTTP_11;
  let is_upgrade = req.headers().get(http::header::CONNECTION).map_or(false, |v| websocket::connection_has(v, "upgrade"));
  let is_websocket_upgrade =
    req.headers().get(http::header::UPGRADE).and_then(|v| v.to_str().ok()).map_or(false, |v| v.eq_ignore_ascii_case("websocket"));
  let is_websocket_version_13 = req.headers().get(http::header::SEC_WEBSOCKET_VERSION).and_then(|v| v.to_str().ok()).map_or(false, |v| v == "13");
  if !is_http_11 || !is_upgrade || !is_websocket_upgrade || !is_websocket_version_13 {
    crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
    return Response::builder()
      .status(StatusCode::UPGRADE_REQUIRED)
      .header(http::header::SEC_WEBSOCKET_VERSION, "13")
      .body("Expected Upgrade to WebSocket version 13".into());
  }
  let Some(websocket_key) = req.headers().get(http::header::SEC_WEBSOCKET_KEY) else {
    crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
    return Response::builder().status(StatusCode::UPGRADE_REQUIRED).body("WebSocket key missing".into());
  };
  let accept = websocket::convert_key(websocket_key.as_bytes());
  let directory = web_server.directory.clone();
  tokio::spawn(async move {
    match hyper::upgrade::on(req).await {
      Err(e) => {
        crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
        eprintln!("Upgrade error: {}", e);
      }
      Ok(upgraded) => {
        if Entity::establish(claim, WebSocketStream::from_raw_socket(upgraded.into(), Role::Server, None).await, directory).await.is_err() {
          crate::metrics::BAD_WEB_REQUEST.get_or_create(&()).inc();
          eprintln!("Failed to establish entity");
        }
      }
    }
  });

  Response::builder()
    .status(StatusCode::SWITCHING_PROTOCOLS)
    .header(http::header::UPGRADE, "websocket")
    .header(http::header::CONNECTION, "upgrade")
    .header(http::header::SEC_WEBSOCKET_ACCEPT, &accept)
    .body(Default::default())
}
