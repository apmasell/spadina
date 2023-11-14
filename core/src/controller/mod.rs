pub mod boxed_template;
pub mod editor;
pub mod puzzle;
pub mod serde_controller;

use crate::location::target::UnresolvedTarget;
use crate::player::PlayerIdentifier;
use serde::Serializer;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

pub type MessagePackSerializer<'a> = &'a mut rmp_serde::Serializer<&'a mut Vec<u8>>;
pub type MessagePackDeserializer<'a> = &'a mut rmp_serde::Deserializer<&'a mut dyn Read>;

pub enum LoadError<E, S> {
  State(S),
  Deserialization(E),
  DeserializationMismatch,
}

pub trait Controller: Send + 'static {
  type Input: Send + 'static;
  type Output: Send + 'static;
  fn capabilities(&self) -> &BTreeSet<&'static str>;
  fn next_timer(&self) -> Option<Duration>;
  fn process(&mut self, input: ControllerInput<Self::Input, &str>) -> Vec<ControllerOutput<Self::Output>>;
  fn serialize_message_pack(
    &self,
    serializer: MessagePackSerializer,
  ) -> Result<<MessagePackSerializer as Serializer>::Ok, <MessagePackSerializer as Serializer>::Error>;
  fn to_json(&self) -> Result<Value, serde_json::Error>;
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ControllerInput<Request, S: AsRef<str>> {
  Add { player: PlayerIdentifier<S>, player_id: u32, player_kind: PlayerKind },
  Input { request_id: i32, player: PlayerIdentifier<S>, player_id: u32, player_kind: PlayerKind, request: Request },
  Remove { player: PlayerIdentifier<S>, player_id: u32, player_kind: PlayerKind },
  Timer,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ControllerOutput<Response> {
  Broadcast { response: Response },
  Move { player: u32, target: UnresolvedTarget<Arc<str>> },
  Quit,
  Response { player: u32, response: Response },
}

pub trait ControllerTemplate: Send + Sync + 'static {
  type Error: Error + 'static;
  type Controller: Controller;
  fn blank(&self) -> Self::Controller;
  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<serde_json::Error, Self::Error>>;
  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>>;
  fn name(&self, owner: &str) -> Cow<'static, str>;
}

pub type GenericControllerTemplate =
  Arc<dyn ControllerTemplate<Error = GenericError, Controller = Box<dyn Controller<Input = Vec<u8>, Output = Vec<u8>> + Send + Sync>> + Send + Sync>;
pub struct GenericError(pub Box<dyn Error + Send + Sync>);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlayerKind {
  Regular,
  Admin,
  Owner,
}
impl<E: Error, P: Error + Send + Sync + 'static> LoadError<E, P> {
  pub fn boxed(self) -> LoadError<E, GenericError> {
    match self {
      LoadError::State(s) => LoadError::State(GenericError(Box::new(s))),
      LoadError::Deserialization(e) => LoadError::Deserialization(e),
      LoadError::DeserializationMismatch => LoadError::DeserializationMismatch,
    }
  }
}

impl<P: Error> From<rmp_serde::decode::Error> for LoadError<rmp_serde::decode::Error, P> {
  fn from(value: rmp_serde::decode::Error) -> Self {
    LoadError::Deserialization(value)
  }
}

impl<E: Error, P: Error> Debug for LoadError<E, P> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      LoadError::State(p) => Debug::fmt(p, f),
      LoadError::Deserialization(d) => Debug::fmt(d, f),
      LoadError::DeserializationMismatch => f.write_str("Deserialized data does not match template"),
    }
  }
}

impl<E: Error, P: Error> Display for LoadError<E, P> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      LoadError::State(p) => Display::fmt(p, f),
      LoadError::Deserialization(d) => Display::fmt(d, f),
      LoadError::DeserializationMismatch => f.write_str("Deserialized data does not match template"),
    }
  }
}

impl<E: Error, P: Error> Error for LoadError<E, P> {}

impl<C: Controller + ?Sized> Controller for Box<C> {
  type Input = C::Input;
  type Output = C::Output;

  fn capabilities(&self) -> &BTreeSet<&'static str> {
    self.as_ref().capabilities()
  }

  fn next_timer(&self) -> Option<Duration> {
    self.as_ref().next_timer()
  }

  fn process(&mut self, input: ControllerInput<Self::Input, &str>) -> Vec<ControllerOutput<Self::Output>> {
    self.as_mut().process(input)
  }

  fn serialize_message_pack(
    &self,
    serializer: MessagePackSerializer,
  ) -> Result<<MessagePackSerializer as Serializer>::Ok, <MessagePackSerializer as Serializer>::Error> {
    self.as_ref().serialize_message_pack(serializer)
  }

  fn to_json(&self) -> Result<Value, serde_json::Error> {
    self.as_ref().to_json()
  }
}
impl<CT: ControllerTemplate + ?Sized> ControllerTemplate for Arc<CT> {
  type Error = CT::Error;
  type Controller = CT::Controller;

  fn blank(&self) -> Self::Controller {
    self.as_ref().blank()
  }

  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<serde_json::Error, Self::Error>> {
    self.as_ref().load_json(value)
  }

  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>> {
    self.as_ref().load_message_pack(de)
  }

  fn name(&self, owner: &str) -> Cow<'static, str> {
    self.as_ref().name(owner)
  }
}

impl Debug for GenericError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Debug::fmt(&self.0, f)
  }
}

impl Display for GenericError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Display::fmt(&self.0, f)
  }
}

impl Error for GenericError {}
