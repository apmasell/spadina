use futures::StreamExt;
use spadina_core::net::ToWebMessage;
pub(crate) enum PlayerProxy {
  Guest {
    player: std::sync::Arc<str>,
    realm: tokio::sync::mpsc::Receiver<crate::destination::DestinationRequest<spadina_core::self_hosted::GuestRequest<crate::shstr::ShStr>>>,
    avatar: tokio_stream::wrappers::WatchStream<spadina_core::avatar::Avatar>,
    local_server: std::sync::Arc<str>,
  },
  Realm {
    player: std::sync::Arc<str>,
    realm: tokio::sync::mpsc::Receiver<crate::destination::DestinationRequest<spadina_core::realm::RealmRequest<crate::shstr::ShStr>>>,
    avatar: tokio_stream::wrappers::WatchStream<spadina_core::avatar::Avatar>,
    local_server: std::sync::Arc<str>,
  },
}
pub(crate) struct GuestStream {
  pub player: std::sync::Arc<str>,
  pub rx: tokio::sync::mpsc::Receiver<crate::destination::DestinationResponse<spadina_core::self_hosted::GuestResponse<crate::shstr::ShStr>>>,
}
pub(crate) struct RealmStream {
  pub player: std::sync::Arc<str>,
  pub rx: tokio::sync::mpsc::Receiver<crate::destination::DestinationResponse<spadina_core::realm::RealmResponse<crate::shstr::ShStr>>>,
}

impl futures::Stream for PlayerProxy {
  type Item = super::PeerRequest;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    match self.get_mut() {
      PlayerProxy::Guest { player, realm, avatar, local_server } => match realm.poll_recv(cx) {
        std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
        std::task::Poll::Ready(Some(request)) => std::task::Poll::Ready(Some(super::PeerRequest::Message(match request {
          crate::destination::DestinationRequest::Request(request) => {
            crate::peer::message::PeerMessage::GuestRequest { player: crate::shstr::ShStr::Shared(player.clone()), request }.as_wsm()
          }
          crate::destination::DestinationRequest::ConsensualEmoteRequest { emote, player: recipient } => {
            crate::peer::message::PeerMessage::ConsensualEmoteRequestInitiate {
              player: player.as_ref(),
              emote: emote.as_ref(),
              recipient: recipient.as_ref().globalize(local_server.as_ref()),
            }
            .as_wsm()
          }
          crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok } => {
            crate::peer::message::PeerMessage::ConsensualEmoteResponse { player: player.as_ref(), id, ok }.as_wsm()
          }
          crate::destination::DestinationRequest::FollowResponse(id, ok) => {
            crate::peer::message::PeerMessage::FollowResponse { player: player.as_ref(), id, ok }.as_wsm()
          }
          crate::destination::DestinationRequest::FollowRequest(target) => crate::peer::message::PeerMessage::FollowRequestInitiate {
            player: player.as_ref(),
            target: target.as_ref().globalize(local_server.as_ref()),
          }
          .as_wsm(),
          crate::destination::DestinationRequest::Messages { from, to } => {
            crate::peer::message::PeerMessage::LocationMessagesGet { player: &player, from, to }.as_wsm()
          }
          crate::destination::DestinationRequest::SendMessage(body) => {
            crate::peer::message::PeerMessage::LocationMessageSend { player: crate::shstr::ShStr::Shared(player.clone()), body }.as_wsm()
          }
        }))),
        std::task::Poll::Pending => match avatar.poll_next_unpin(cx) {
          std::task::Poll::Pending | std::task::Poll::Ready(None) => std::task::Poll::Pending,
          std::task::Poll::Ready(Some(avatar)) => std::task::Poll::Ready(Some(super::PeerRequest::Message(
            crate::peer::message::PeerMessage::AvatarSet { player: player.as_ref(), avatar }.as_wsm(),
          ))),
        },
      },
      PlayerProxy::Realm { player, realm, avatar, local_server } => match realm.poll_recv(cx) {
        std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
        std::task::Poll::Ready(Some(request)) => std::task::Poll::Ready(Some(super::PeerRequest::Message(match request {
          crate::destination::DestinationRequest::Request(request) => {
            crate::peer::message::PeerMessage::RealmRequest { player: crate::shstr::ShStr::Shared(player.clone()), request }.as_wsm()
          }
          crate::destination::DestinationRequest::ConsensualEmoteRequest { emote, player: recipient } => {
            crate::peer::message::PeerMessage::ConsensualEmoteRequestInitiate {
              player: player.as_ref(),
              emote: emote.as_ref(),
              recipient: recipient.as_ref().globalize(local_server.as_ref()),
            }
            .as_wsm()
          }
          crate::destination::DestinationRequest::ConsensualEmoteResponse { id, ok } => {
            crate::peer::message::PeerMessage::ConsensualEmoteResponse { player: player.as_ref(), id, ok }.as_wsm()
          }
          crate::destination::DestinationRequest::FollowResponse(id, ok) => {
            crate::peer::message::PeerMessage::FollowResponse { player: player.as_ref(), id, ok }.as_wsm()
          }
          crate::destination::DestinationRequest::FollowRequest(target) => crate::peer::message::PeerMessage::FollowRequestInitiate {
            player: player.as_ref(),
            target: target.as_ref().globalize(local_server.as_ref()),
          }
          .as_wsm(),
          crate::destination::DestinationRequest::Messages { from, to } => {
            crate::peer::message::PeerMessage::LocationMessagesGet { player: &player, from, to }.as_wsm()
          }
          crate::destination::DestinationRequest::SendMessage(body) => {
            crate::peer::message::PeerMessage::LocationMessageSend { player: crate::shstr::ShStr::Shared(player.clone()), body }.as_wsm()
          }
        }))),
        std::task::Poll::Pending => match avatar.poll_next_unpin(cx) {
          std::task::Poll::Pending | std::task::Poll::Ready(None) => std::task::Poll::Pending,
          std::task::Poll::Ready(Some(avatar)) => std::task::Poll::Ready(Some(super::PeerRequest::Message(
            crate::peer::message::PeerMessage::AvatarSet { player: player.as_ref(), avatar }.as_wsm(),
          ))),
        },
      },
    }
  }
}
impl futures::Stream for RealmStream {
  type Item = super::PeerRequest;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    let realm_stream = self.get_mut();
    match realm_stream.rx.poll_recv(cx) {
      std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
      std::task::Poll::Ready(Some(response)) => std::task::Poll::Ready(Some(match response {
        crate::destination::DestinationResponse::ConsensualEmoteRequest(sender, id, emote) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::ConsensualEmoteRequestFromLocation {
            id,
            player: realm_stream.player.as_ref(),
            emote: emote.as_ref(),
            sender: sender.as_ref(),
          }
          .as_wsm(),
        ),
        crate::destination::DestinationResponse::FollowRequest(source, id) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::FollowRequestFromLocation { id, player: realm_stream.player.as_ref(), source: source.as_ref() }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Location(response) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationChange { player: crate::shstr::ShStr::Shared(realm_stream.player.clone()), response }.as_wsm(),
        ),
        crate::destination::DestinationResponse::MessagePosted(message) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationMessagePosted { player: realm_stream.player.clone(), message }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Messages { from, to, messages } => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationMessages { player: realm_stream.player.to_string(), from, to, messages }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Move(target) => {
          super::PeerRequest::Message(crate::peer::message::PeerMessage::VisitorRelease { player: realm_stream.player.clone(), target }.as_wsm())
        }
        crate::destination::DestinationResponse::MoveTrain(owner, train) => {
          super::PeerRequest::SendPlayerTrain { player: realm_stream.player.clone(), owner, train }
        }
        crate::destination::DestinationResponse::Response(response) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::RealmResponse { player: crate::shstr::ShStr::Shared(realm_stream.player.clone()), response }.as_wsm(),
        ),
      })),
      std::task::Poll::Pending => std::task::Poll::Pending,
    }
  }
}
impl futures::Stream for GuestStream {
  type Item = super::PeerRequest;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    let realm_stream = self.get_mut();
    match realm_stream.rx.poll_recv(cx) {
      std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
      std::task::Poll::Ready(Some(response)) => std::task::Poll::Ready(Some(match response {
        crate::destination::DestinationResponse::ConsensualEmoteRequest(sender, id, emote) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::ConsensualEmoteRequestFromLocation {
            id,
            player: realm_stream.player.as_ref(),
            emote: emote.as_ref(),
            sender: sender.as_ref(),
          }
          .as_wsm(),
        ),
        crate::destination::DestinationResponse::FollowRequest(source, id) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::FollowRequestFromLocation { id, player: realm_stream.player.as_ref(), source: source.as_ref() }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Location(response) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationChange { player: crate::shstr::ShStr::Shared(realm_stream.player.clone()), response }.as_wsm(),
        ),
        crate::destination::DestinationResponse::MessagePosted(message) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationMessagePosted { player: realm_stream.player.clone(), message }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Messages { from, to, messages } => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::LocationMessages { player: realm_stream.player.to_string(), from, to, messages }.as_wsm(),
        ),
        crate::destination::DestinationResponse::Move(target) => {
          super::PeerRequest::Message(crate::peer::message::PeerMessage::VisitorRelease { player: realm_stream.player.clone(), target }.as_wsm())
        }
        crate::destination::DestinationResponse::MoveTrain(owner, train) => {
          super::PeerRequest::SendPlayerTrain { player: realm_stream.player.clone(), owner, train }
        }
        crate::destination::DestinationResponse::Response(response) => super::PeerRequest::Message(
          crate::peer::message::PeerMessage::GuestResponse { player: crate::shstr::ShStr::Shared(realm_stream.player.clone()), response }.as_wsm(),
        ),
      })),
      std::task::Poll::Pending => std::task::Poll::Pending,
    }
  }
}
