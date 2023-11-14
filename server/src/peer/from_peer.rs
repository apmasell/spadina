use crate::join_request::JoinRequest;
use crate::peer::message::PeerMessage;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use crate::stream_map::{OutputMapper, StreamsUnorderedMap};
use spadina_core::avatar::Avatar;
use spadina_core::location::target::UnresolvedTarget;
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::{AsReference, ForPacket};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub struct PlayerFromPeer {
  input: mpsc::Receiver<PlayerLocationUpdate>,
  output: mpsc::Sender<PlayerEvent>,
}
impl PlayerFromPeer {
  pub fn create(
    player: Arc<str>,
    server: Arc<str>,
    avatar: Avatar,
    players: &mut StreamsUnorderedMap<BTreeMap<Arc<str>, PlayerFromPeer>>,
  ) -> JoinRequest {
    let (output, rx) = mpsc::channel(100);
    let (tx, input) = mpsc::channel(100);
    players.mutate().insert(player.clone(), PlayerFromPeer { input, output });
    JoinRequest { avatar, is_superuser: false, name: PlayerIdentifier::Remote { player, server }, tx, rx }
  }
  pub async fn send(&self, event: PlayerEvent) -> Result<(), ()> {
    self.output.send(event).await.map_err(|_| ())
  }
}
impl futures::Stream for PlayerFromPeer {
  type Item = PlayerLocationUpdate;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
    let realm_stream = self.get_mut();
    realm_stream.input.poll_recv(cx)
  }
}

impl OutputMapper<Arc<str>> for PlayerFromPeer {
  type Output = Message;

  fn handle(&mut self, player: &Arc<str>, response: Self::Item) -> Option<Self::Output> {
    Some(match response {
      PlayerLocationUpdate::ResolveUpdate(response) => {
        PeerMessage::<_, &[u8]>::LocationChange { player: player.as_ref(), response: response.reference(AsReference::<str>::default()) }.into()
      }
      PlayerLocationUpdate::Move(target) => {
        PeerMessage::<_, &[u8]>::VisitorRelease { player: player.as_ref(), target: target.reference(AsReference::<str>::default()) }.into()
      }
      PlayerLocationUpdate::ResponseSingle(response) => {
        PeerMessage::LocationResponse { player: player.as_ref(), response: response.reference(ForPacket) }.into()
      }
      PlayerLocationUpdate::ResponseShared(response) => {
        PeerMessage::LocationResponse { player: player.as_ref(), response: response.reference(ForPacket) }.into()
      }
    })
  }

  fn end(self, player: &Arc<str>) -> Option<Self::Output> {
    Some(PeerMessage::<_, &[u8]>::VisitorRelease { player, target: UnresolvedTarget::NoWhere }.into())
  }
}
