use serde::Deserialize;
use serde::Serialize;
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HostCommand<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> {
  Broadcast { response: GuestResponse<S> },
  Move { player: crate::player::PlayerIdentifier<S>, target: Option<crate::realm::RealmTarget<S>> },
  MoveTrain { player: crate::player::PlayerIdentifier<S>, owner: S, train: u16 },
  Quit,
  Response { player: crate::player::PlayerIdentifier<S>, response: GuestResponse<S> },
  SendMessage { body: crate::communication::MessageBody<S> },
  UpdateAccess { default: crate::access::SimpleAccess, rules: Vec<crate::access::AccessControl<crate::access::SimpleAccess>> },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HostEvent<S: AsRef<str>> {
  ConsensualEmote {
    initiator: crate::player::PlayerIdentifier<S>,
    recipient: crate::player::PlayerIdentifier<S>,
    emote: S,
  },
  Follow {
    requester: crate::player::PlayerIdentifier<S>,
    target: crate::player::PlayerIdentifier<S>,
  },
  PlayerEntered {
    player: crate::player::PlayerIdentifier<S>,
  },
  PlayerLeft {
    player: crate::player::PlayerIdentifier<S>,
  },
  PlayerRequest {
    /// A request from a guest player recieved to this player, who is host
    player: crate::player::PlayerIdentifier<S>,
    request: GuestRequest<S>,
  },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GuestRequest<S: AsRef<str>> {
  Realm { request: crate::realm::RealmRequest<S> },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GuestResponse<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> {
  Realm { response: crate::realm::RealmResponse<S> },
}
impl<S: AsRef<str>> GuestRequest<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> GuestRequest<T>
  where
    S: Into<T>,
  {
    match self {
      GuestRequest::Realm { request } => GuestRequest::Realm { request: request.convert_str() },
    }
  }
}

impl<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> GuestResponse<S> {
  pub fn convert_str<T: AsRef<str> + std::cmp::Eq + std::hash::Hash>(self) -> GuestResponse<T>
  where
    S: Into<T>,
  {
    match self {
      GuestResponse::Realm { response } => GuestResponse::Realm { response: response.convert_str() },
    }
  }
}
impl<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> HostCommand<S> {
  pub fn convert_str<T: AsRef<str> + std::cmp::Eq + std::hash::Hash>(self) -> HostCommand<T>
  where
    S: Into<T>,
  {
    match self {
      HostCommand::Broadcast { response } => HostCommand::Broadcast { response: response.convert_str() },
      HostCommand::Move { player, target } => HostCommand::Move { player: player.convert_str(), target: target.map(|t| t.convert_str()) },
      HostCommand::MoveTrain { player, owner, train } => HostCommand::MoveTrain { player: player.convert_str(), owner: owner.into(), train },
      HostCommand::Quit => HostCommand::Quit,
      HostCommand::Response { player, response } => HostCommand::Response { player: player.convert_str(), response: response.convert_str() },
      HostCommand::SendMessage { body } => HostCommand::SendMessage { body: body.convert_str() },
      HostCommand::UpdateAccess { default, rules } => HostCommand::UpdateAccess { default, rules },
    }
  }
}
