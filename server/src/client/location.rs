use crate::client::hosting;
use crate::client::hosting::HostInput;
use crate::directory::Directory;
use crate::join_request::JoinRequest;
use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use spadina_core::access::{AccessSetting, Privilege};
use spadina_core::avatar::Avatar;
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::target::UnresolvedTarget;
use spadina_core::location::DescriptorKind;
use spadina_core::net::server::hosting::HostCommand;
use spadina_core::net::server::ClientResponse;
use spadina_core::player::OnlineState;
use spadina_core::reference_converter::AsReference;
use spadina_core::shared_ref::SharedRef;
use std::sync::Arc;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub enum Location {
  NoWhere,
  Location { location: OnlineState<Arc<str>>, tx: mpsc::Sender<PlayerEvent>, rx: mpsc::Receiver<PlayerLocationUpdate> },
  Hosting { tx: mpsc::Sender<HostInput>, rx: mpsc::Receiver<Message> },
}

impl Location {
  pub async fn start_hosting(
    &mut self,
    owner_name: Arc<str>,
    descriptor: DescriptorKind<Arc<str>>,
    avatar: Avatar,
    acl: AccessSetting<String, Privilege>,
    directory: &Directory,
  ) -> LocationChangeResponse<&'static str> {
    let (location, result) = {
      let (endpoint, rx, tx) = hosting::start_hosting(descriptor, owner_name.clone(), &directory.access_management, avatar, acl);
      if directory.register_host(owner_name, endpoint).await.is_err() {
        (Location::NoWhere, LocationChangeResponse::InternalError)
      } else {
        (Location::Hosting { tx, rx }, LocationChangeResponse::Resolving)
      }
    };
    *self = location;
    result
  }

  pub fn start_join(&mut self, is_superuser: bool, player: Arc<str>, avatar: Avatar) -> (LocationChangeResponse<&'static str>, JoinRequest) {
    let (realm_input, realm_output) = mpsc::channel(100);
    let (player_input, player_output) = mpsc::channel(100);
    *self = Location::Location { tx: realm_input, rx: player_output, location: OnlineState::InTransit };
    (
      LocationChangeResponse::Resolving,
      JoinRequest {
        avatar: avatar.clone(),
        is_superuser,
        name: spadina_core::player::PlayerIdentifier::Local(player),
        tx: player_input,
        rx: realm_output,
      },
    )
  }
  pub fn get_state(&self) -> OnlineState<Arc<str>> {
    match self {
      Location::NoWhere => OnlineState::Online,
      Location::Location { location, .. } => location.clone(),
      Location::Hosting { .. } => OnlineState::Hosting,
    }
  }
  pub async fn send(&mut self, request: PlayerEvent) -> Result<(), Message> {
    let err = match self {
      Location::Location { tx, .. } => tx.send(request).await.is_err(),
      Location::NoWhere => true,
      Location::Hosting { tx, .. } => tx.send(HostInput::Request(request)).await.is_err(),
    };
    if err {
      *self = Location::NoWhere;
      Err(ClientResponse::LocationChange::<String, Vec<u8>> { location: LocationChangeResponse::NoWhere }.into())
    } else {
      Ok(())
    }
  }

  pub async fn send_host_command(&mut self, request: HostCommand<String, Vec<u8>>) -> () {
    let reset = if let Location::Hosting { tx, .. } = self { tx.send(HostInput::Command(request)).await.is_err() } else { false };
    if reset {
      *self = Location::NoWhere;
    }
  }
}
pub enum LocationEvent {
  IdleTimeout,
  Message(Message),
  Redirect(UnresolvedTarget<SharedRef<str>>),
}
impl futures::Stream for Location {
  type Item = LocationEvent;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
    let location = self.get_mut();
    let (result, reset) = match location {
      Location::NoWhere => (Poll::Pending, false),
      Location::Location { rx, location, .. } => match rx.poll_recv(cx) {
        Poll::Pending => (Poll::Pending, false),
        Poll::Ready(None) => (Poll::Ready(Some(LocationEvent::Redirect(UnresolvedTarget::NoWhere))), true),
        Poll::Ready(Some(PlayerLocationUpdate::Move(target))) => (Poll::Ready(Some(LocationEvent::Redirect(target))), true),
        Poll::Ready(Some(PlayerLocationUpdate::ResolveUpdate(new_location))) => {
          let reset = new_location.is_released();
          let message = ClientResponse::<_, &[u8]>::LocationChange { location: new_location.reference(AsReference::<str>::default()) }.into();
          *location = new_location.into_location_state();
          (Poll::Ready(Some(LocationEvent::Message(message))), reset)
        }
        Poll::Ready(Some(PlayerLocationUpdate::ResponseSingle(response))) => {
          (Poll::Ready(Some(LocationEvent::Message(ClientResponse::InLocation { response }.into()))), true)
        }
        Poll::Ready(Some(PlayerLocationUpdate::ResponseShared(response))) => {
          (Poll::Ready(Some(LocationEvent::Message(ClientResponse::InLocation { response }.into()))), true)
        }
      },
      Location::Hosting { rx, .. } => match rx.poll_recv(cx) {
        Poll::Pending => (Poll::Pending, false),
        Poll::Ready(None) => (Poll::Ready(Some(LocationEvent::Redirect(UnresolvedTarget::NoWhere))), true),
        Poll::Ready(Some(message)) => (Poll::Ready(Some(LocationEvent::Message(message))), true),
      },
    };
    if reset {
      *location = Location::NoWhere;
    }
    result
  }
}
