use crate::controller::{Controller, ControllerTemplate, LoadError, MessagePackDeserializer};
use serde_json::Value;
use std::borrow::Cow;
use std::error::Error;

pub struct BoxedTemplate<CT: ControllerTemplate>(pub CT);

impl<CT: ControllerTemplate> ControllerTemplate for BoxedTemplate<CT>
where
  CT::Error: Error,
  CT::Controller: 'static,
{
  type Error = CT::Error;
  type Controller = Box<dyn Controller<Input = <CT::Controller as Controller>::Input, Output = <CT::Controller as Controller>::Output>>;

  fn blank(&self) -> Self::Controller {
    Box::new(self.0.blank())
  }

  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<serde_json::Error, Self::Error>> {
    Ok(Box::new(self.0.load_json(value)?))
  }

  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>> {
    Ok(Box::new(self.0.load_message_pack(de)?))
  }
  fn name(&self, owner: &str) -> Cow<'static, str> {
    self.0.name(owner)
  }
}
