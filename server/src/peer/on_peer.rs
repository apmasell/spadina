use crate::join_request::JoinRequest;
use crate::peer::message::{PeerMessage, VisitorTarget};
use crate::peer::Peer;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use crate::socket_entity::Outgoing;
use crate::stream_map::{OutputMapper, StreamsUnorderedMap};
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::ForPacket;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct PlayerOnPeer {
  input: mpsc::Receiver<PlayerEvent>,
  output: mpsc::Sender<PlayerLocationUpdate>,
}

impl PlayerOnPeer {
  pub fn create(
    request: JoinRequest,
    target: VisitorTarget<&str>,
    players: &mut StreamsUnorderedMap<BTreeMap<Arc<str>, PlayerOnPeer>>,
  ) -> Vec<Outgoing<Peer>> {
    let PlayerIdentifier::Local(player) = request.name else {
      return vec![];
    };
    let message = Outgoing::Send(PeerMessage::<_, &[u8]>::VisitorSend { player: player.as_ref(), target, avatar: request.avatar }.into());
    players.mutate().insert(player, PlayerOnPeer { input: request.rx, output: request.tx });
    vec![message]
  }
  pub async fn send(&self, update: PlayerLocationUpdate) -> Result<(), ()> {
    self.output.send(update).await.map_err(|_| ())
  }
}

impl futures::Stream for PlayerOnPeer {
  type Item = PlayerEvent;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
    let PlayerOnPeer { input, .. } = self.get_mut();
    input.poll_recv(cx)
  }
}

impl OutputMapper<Arc<str>> for PlayerOnPeer {
  type Output = Message;

  fn handle(&mut self, player: &Arc<str>, request: Self::Item) -> Option<Self::Output> {
    Some(match request {
      PlayerEvent::Avatar(avatar) => PeerMessage::<_, &[u8]>::AvatarSet { player: player.as_ref(), avatar }.into(),
      PlayerEvent::Request(request) => PeerMessage::LocationRequest { player: player.as_ref(), request: request.reference(ForPacket) }.into(),
    })
  }

  fn end(self, player: &Arc<str>) -> Option<Self::Output> {
    Some(PeerMessage::<_, &[u8]>::VisitorYank { player }.into())
  }
}
