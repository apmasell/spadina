use crate::socket_entity::ConnectionState;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use spadina_core::net::mixed_connection::MixedConnection;
use std::mem::swap;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::WebSocketStream;

pub enum EntityConnection {
  Online(WebSocketStream<MixedConnection>),
  Dead(DateTime<Utc>, Vec<Message>),
  Offline,
}
impl EntityConnection {
  pub async fn establish(&mut self, connection: WebSocketStream<MixedConnection>) -> Result<(), Error> {
    let mut swapped = EntityConnection::Online(connection);
    swap(self, &mut swapped);
    if let EntityConnection::Dead(_, queued) = swapped {
      for message in queued {
        self.send(message).await?;
      }
    }
    Ok(())
  }
  pub fn connection_state(&self) -> ConnectionState {
    if let EntityConnection::Online(socket) = self {
      match socket.get_ref() {
        MixedConnection::Upgraded(_) => ConnectionState::ConnectedWeb,
        MixedConnection::Unix(_) => ConnectionState::ConnectedUnix,
      }
    } else {
      ConnectionState::Disconnected
    }
  }
  pub async fn send(&mut self, message: Message) -> Result<(), Error> {
    match self {
      EntityConnection::Online(tx) => {
        tx.send(message).await?;
        Ok(())
      }
      EntityConnection::Dead(time, queued) => {
        if Utc::now() - *time < chrono::Duration::minutes(5) {
          queued.push(message);
          Ok(())
        } else {
          *self = EntityConnection::Offline;
          Err(Error::ConnectionClosed)
        }
      }
      EntityConnection::Offline => Err(Error::ConnectionClosed),
    }
  }
}
impl futures::Stream for EntityConnection {
  type Item = Result<Message, Error>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    match self.get_mut() {
      EntityConnection::Online(c) => c.poll_next_unpin(cx),
      EntityConnection::Dead(..) => std::task::Poll::Pending,
      EntityConnection::Offline => std::task::Poll::Pending,
    }
  }
}
