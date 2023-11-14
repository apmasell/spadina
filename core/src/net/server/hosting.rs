use crate::avatar::Avatar;
use crate::location::target::UnresolvedTarget;
use crate::player::PlayerIdentifier;
use crate::reference_converter::{Converter, Referencer};
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HostCommand<S: AsRef<str>, B> {
  Broadcast { response: B },
  Drop { player: PlayerIdentifier<S> },
  Move { player: PlayerIdentifier<S>, target: UnresolvedTarget<S> },
  Quit,
  RequestError { player: PlayerIdentifier<S>, request_id: i32 },
  Response { player: PlayerIdentifier<S>, response: B },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HostEvent<S: AsRef<str>, B> {
  PlayerEntered {
    avatar: Avatar,
    player: PlayerIdentifier<S>,
    is_admin: bool,
  },
  PlayerLeft {
    player: PlayerIdentifier<S>,
  },
  PlayerRequest {
    /// A request from a guest player received to this player, who is host
    player: PlayerIdentifier<S>,
    is_admin: bool,
    request_id: i32,
    request: B,
  },
}

impl<S: AsRef<str>, B> HostCommand<S, B> {
  pub fn reference<'a, R: Referencer<S> + Referencer<B>>(
    &'a self,
    reference: R,
  ) -> HostCommand<<R as Referencer<S>>::Output<'a>, <R as Referencer<B>>::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      HostCommand::Broadcast { response } => HostCommand::Broadcast { response: reference.convert(response) },
      HostCommand::Drop { player } => HostCommand::Drop { player: player.reference(reference) },
      HostCommand::Move { player, target } => HostCommand::Move { player: player.reference(reference), target: target.reference(reference) },
      HostCommand::Quit => HostCommand::Quit,
      HostCommand::RequestError { player, request_id } => HostCommand::RequestError { player: player.reference(reference), request_id: *request_id },
      HostCommand::Response { player, response } => {
        HostCommand::Response { player: player.reference(reference), response: reference.convert(response) }
      }
    }
  }
  pub fn convert<C: Converter<S> + Converter<B>>(self, converter: C) -> HostCommand<<C as Converter<S>>::Output, <C as Converter<B>>::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      HostCommand::Broadcast { response } => HostCommand::Broadcast { response: converter.convert(response) },
      HostCommand::Drop { player } => HostCommand::Drop { player: player.convert(converter) },
      HostCommand::Move { player, target } => HostCommand::Move { player: player.convert(converter), target: target.convert(converter) },
      HostCommand::Quit => HostCommand::Quit,
      HostCommand::RequestError { request_id, player } => HostCommand::RequestError { request_id, player: player.convert(converter) },
      HostCommand::Response { player, response } => {
        HostCommand::Response { player: player.convert(converter), response: converter.convert(response) }
      }
    }
  }
}
impl<S: AsRef<str>, B> HostEvent<S, B> {
  pub fn reference<'a, R: Referencer<S> + Referencer<B>>(
    &'a self,
    reference: R,
  ) -> HostEvent<<R as Referencer<S>>::Output<'a>, <R as Referencer<B>>::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      HostEvent::PlayerEntered { player, is_admin, avatar } => {
        HostEvent::PlayerEntered { player: player.reference(reference), is_admin: *is_admin, avatar: avatar.clone() }
      }
      HostEvent::PlayerLeft { player } => HostEvent::PlayerLeft { player: player.reference(reference) },
      HostEvent::PlayerRequest { request_id, player, is_admin, request } => HostEvent::PlayerRequest {
        request_id: *request_id,
        player: player.reference(reference),
        is_admin: *is_admin,
        request: reference.convert(request),
      },
    }
  }
  pub fn convert<C: Converter<S> + Converter<B>>(self, converter: C) -> HostEvent<<C as Converter<S>>::Output, <C as Converter<B>>::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      HostEvent::PlayerEntered { player, is_admin, avatar } => HostEvent::PlayerEntered { player: player.convert(converter), is_admin, avatar },
      HostEvent::PlayerLeft { player } => HostEvent::PlayerLeft { player: player.convert(converter) },
      HostEvent::PlayerRequest { request_id, player, is_admin, request } => {
        HostEvent::PlayerRequest { request_id, player: player.convert(converter), is_admin, request: converter.convert(request) }
      }
    }
  }
}
