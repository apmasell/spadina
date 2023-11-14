use crate::controller::{
  Controller, ControllerInput, ControllerOutput, ControllerTemplate, LoadError, MessagePackDeserializer, MessagePackSerializer,
};
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::error::Error;
use std::time::Duration;

pub struct SerdeController<C: Controller>(pub C);
pub struct SerdeControllerTemplate<CT: ControllerTemplate>(pub CT);

impl<C: Controller> Controller for SerdeController<C>
where
  C::Input: DeserializeOwned,
  C::Output: Serialize,
{
  type Input = Vec<u8>;
  type Output = Vec<u8>;

  fn capabilities(&self) -> &BTreeSet<&'static str> {
    self.0.capabilities()
  }

  fn next_timer(&self) -> Option<Duration> {
    self.0.next_timer()
  }

  fn process(&mut self, input: ControllerInput<Self::Input, &str>) -> Vec<ControllerOutput<Self::Output>> {
    match input {
      ControllerInput::Add { player, player_id, player_kind } => self.0.process(ControllerInput::Add { player, player_id, player_kind }),
      ControllerInput::Input { request_id, player, player_id, player_kind, request } => match rmp_serde::from_slice(&request) {
        Err(_) => return Vec::new(),
        Ok(request) => self.0.process(ControllerInput::Input { request_id, player, player_id, player_kind, request }),
      },
      ControllerInput::Remove { player, player_id, player_kind } => self.0.process(ControllerInput::Remove { player, player_id, player_kind }),
      ControllerInput::Timer => self.0.process(ControllerInput::Timer),
    }
    .into_iter()
    .flat_map(|output| match output {
      ControllerOutput::Broadcast { response } => match rmp_serde::to_vec_named(&response) {
        Ok(response) => Some(ControllerOutput::Broadcast { response }),
        Err(e) => {
          eprintln!("Failed to serialize controller message: {}", e);
          None
        }
      },
      ControllerOutput::Move { player, target } => Some(ControllerOutput::Move { player, target }),
      ControllerOutput::Quit => Some(ControllerOutput::Quit),
      ControllerOutput::Response { player, response } => match rmp_serde::to_vec_named(&response) {
        Ok(response) => Some(ControllerOutput::Response { player, response }),
        Err(e) => {
          eprintln!("Failed to serialize controller message: {}", e);
          None
        }
      },
    })
    .collect()
  }

  fn serialize_message_pack(
    &self,
    serializer: MessagePackSerializer,
  ) -> Result<<MessagePackSerializer as Serializer>::Ok, <MessagePackSerializer as Serializer>::Error> {
    self.0.serialize_message_pack(serializer)
  }

  fn to_json(&self) -> Result<Value, serde_json::Error> {
    self.0.to_json()
  }
}
impl<CT: ControllerTemplate> ControllerTemplate for SerdeControllerTemplate<CT>
where
  CT::Error: Error,
  CT::Controller: 'static,
  <CT::Controller as Controller>::Output: Serialize,
  <CT::Controller as Controller>::Input: DeserializeOwned,
{
  type Error = CT::Error;
  type Controller = SerdeController<CT::Controller>;

  fn blank(&self) -> Self::Controller {
    SerdeController(self.0.blank())
  }

  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<serde_json::Error, Self::Error>> {
    match self.0.load_json(value) {
      Ok(c) => Ok(SerdeController(c)),
      Err(LoadError::Deserialization(e)) => Err(LoadError::Deserialization(e)),
      Err(LoadError::DeserializationMismatch) => Err(LoadError::DeserializationMismatch),
      Err(LoadError::State(s)) => Err(LoadError::State(s)),
    }
  }

  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>> {
    match self.0.load_message_pack(de) {
      Ok(c) => Ok(SerdeController(c)),
      Err(LoadError::Deserialization(e)) => Err(LoadError::Deserialization(e)),
      Err(LoadError::DeserializationMismatch) => Err(LoadError::DeserializationMismatch),
      Err(LoadError::State(s)) => Err(LoadError::State(s)),
    }
  }
  fn name(&self, owner: &str) -> Cow<'static, str> {
    self.0.name(owner)
  }
}
