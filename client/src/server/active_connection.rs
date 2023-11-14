use crate::server::ServerEvent;
use futures::{SinkExt, Stream, StreamExt};
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::net::server::ClientResponse;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::WebSocketStream;

pub enum ActiveConnection {
  Idle,
  Active(WebSocketStream<MixedConnection>),
}

pub type SendResult<T> = Result<T, Error>;

impl ActiveConnection {
  pub async fn send(&mut self, message: Message) -> SendResult<()> {
    match self {
      ActiveConnection::Active(connection) => connection.send(message).await,
      ActiveConnection::Idle => Err(Error::ConnectionClosed),
    }
  }
}
impl Stream for ActiveConnection {
  type Item = ServerEvent<ClientResponse<String, Vec<u8>>>;
  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match self.get_mut() {
      ActiveConnection::Idle => Poll::Pending,
      ActiveConnection::Active(connection) => match connection.poll_next_unpin(cx) {
        Poll::Pending => Poll::Pending,
        Poll::Ready(None) => Poll::Ready(None),
        Poll::Ready(Some(Ok(Message::Binary(value)))) => match rmp_serde::from_slice(&value) {
          Ok(v) => Poll::Ready(Some(ServerEvent::Result(v))),
          Err(e) => {
            eprintln!("Failed to decode message from server. Mismatched protocols?: {}", e);
            Poll::Ready(Some(ServerEvent::BadMessage))
          }
        },
        Poll::Ready(Some(Ok(_))) => Poll::Ready(Some(ServerEvent::BadMessage)),
        Poll::Ready(Some(Err(e))) => {
          eprintln!("Error in connection: {}", e);
          Poll::Ready(Some(ServerEvent::BadMessage))
        }
      },
    }
  }
}
