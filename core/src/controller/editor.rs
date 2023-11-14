use crate::controller::{
  Controller, ControllerInput, ControllerOutput, ControllerTemplate, LoadError, MessagePackDeserializer, MessagePackSerializer,
};
use serde::Serializer;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;

pub struct Editor;
#[derive(Debug)]
pub enum EditorError {}
impl ControllerTemplate for Editor {
  type Error = EditorError;
  type Controller = EditorController;

  fn blank(&self) -> Self::Controller {
    EditorController
  }

  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<serde_json::Error, Self::Error>> {
    todo!()
  }

  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>> {
    todo!()
  }

  fn name(&self, _owner: &str) -> Cow<'static, str> {
    Cow::Borrowed("New Editor")
  }
}
pub struct EditorController;
impl Controller for EditorController {
  type Input = ();
  type Output = ();

  fn capabilities(&self) -> &BTreeSet<&'static str> {
    todo!()
  }

  fn next_timer(&self) -> Option<Duration> {
    todo!()
  }

  fn process(&mut self, input: ControllerInput<Self::Input, &str>) -> Vec<ControllerOutput<Self::Output>> {
    todo!()
  }

  fn serialize_message_pack(
    &self,
    serializer: MessagePackSerializer,
  ) -> Result<<MessagePackSerializer as Serializer>::Ok, <MessagePackSerializer as Serializer>::Error> {
    todo!()
  }

  fn to_json(&self) -> Result<Value, serde_json::Error> {
    todo!()
  }
}

impl Display for EditorError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    todo!()
  }
}
impl Error for EditorError {}
