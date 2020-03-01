use crate::peer::message::PeerMessage;
use futures::SinkExt;
pub const PATH_START: &str = "/api/server/start/v1";
pub const PATH_FINISH: &str = "/api/server/finish/v1";
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerClaim<S: AsRef<str>> {
  pub exp: usize,
  pub name: S,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PeerHttpRequestBody<S: AsRef<str>> {
  pub token: S,
  pub server: S,
}

#[pin_project::pin_project(project = PeerConnectionProjection)]
pub enum PeerConnection {
  Online(#[pin] tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>, std::sync::Arc<str>),

  Dead(chrono::DateTime<chrono::Utc>, Vec<tokio_tungstenite::tungstenite::Message>, std::sync::Arc<str>),
  Offline(std::sync::Arc<str>),
}
impl PeerConnection {
  pub async fn establish(
    &mut self,
    mut connection: tokio_tungstenite::WebSocketStream<spadina_core::net::IncomingConnection>,
    name: std::sync::Arc<str>,
  ) {
    if let PeerConnection::Dead(_, queued, name) = self {
      for message in queued.drain(..) {
        if let Err(e) = connection.send(message).await {
          eprintln!("Failed to send during reconnection for {}: {}", name, e);
        }
      }
    }
    *self = PeerConnection::Online(connection, name);
  }
  pub async fn send(&mut self, message: tokio_tungstenite::tungstenite::Message) -> () {
    match self {
      PeerConnection::Online(tx, name) => {
        if let Err(e) = tx.send(message).await {
          eprintln!("Failed to send to peer {}: {}", name, e);
          crate::metrics::BAD_PEER_SEND.get_or_create(&crate::metrics::PeerLabel { peer: crate::shstr::ShStr::Shared(name.clone()) }).inc();
        }
      }
      PeerConnection::Dead(time, queued, name) => {
        if chrono::Utc::now() - *time < chrono::Duration::minutes(5) {
          queued.push(message);
        } else {
          *self = PeerConnection::Offline(name.clone());
        }
      }
      PeerConnection::Offline(name) => eprintln!("Ignoring message to offline peer {}", name),
    }
  }
}
impl futures::Stream for PeerConnection {
  type Item = PeerMessage<crate::shstr::ShStr>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    match self.project() {
      PeerConnectionProjection::Online(c, name) => match c.poll_next(cx) {
        std::task::Poll::Ready(Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(bytes)))) => match rmp_serde::from_slice(&bytes) {
          Ok(value) => std::task::Poll::Ready(Some(value)),
          Err(e) => {
            eprintln!("Bad message from peer {}: {}", name, e);
            crate::metrics::BAD_PEER_REQUESTS.get_or_create(&crate::metrics::PeerLabel { peer: crate::shstr::ShStr::Shared(name.clone()) }).inc();
            std::task::Poll::Pending
          }
        },
        std::task::Poll::Ready(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(str)))) => match serde_json::from_str(&str) {
          Ok(value) => std::task::Poll::Ready(Some(value)),
          Err(e) => {
            eprintln!("Bad message from peer {}: {}", name, e);
            crate::metrics::BAD_PEER_REQUESTS.get_or_create(&crate::metrics::PeerLabel { peer: crate::shstr::ShStr::Shared(name.clone()) }).inc();
            std::task::Poll::Pending
          }
        },
        std::task::Poll::Ready(Some(Err(e))) => {
          eprintln!("Error from peer {} socket: {}", name, e);
          crate::metrics::BAD_PEER_REQUESTS.get_or_create(&crate::metrics::PeerLabel { peer: crate::shstr::ShStr::Shared(name.clone()) }).inc();
          std::task::Poll::Ready(None)
        }
        std::task::Poll::Ready(_) => std::task::Poll::Ready(None),
        std::task::Poll::Pending => std::task::Poll::Pending,
      },
      PeerConnectionProjection::Dead(_, _, _) => std::task::Poll::Pending,
      PeerConnectionProjection::Offline(_) => std::task::Poll::Pending,
    }
  }
}
