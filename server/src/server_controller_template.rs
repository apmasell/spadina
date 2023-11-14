use serde_json::{Error, Value};
use spadina_core::controller::editor::Editor;
use spadina_core::controller::serde_controller::SerdeController;
use spadina_core::controller::{Controller, ControllerTemplate, GenericControllerTemplate, GenericError, LoadError, MessagePackDeserializer};
use spadina_core::location::Application;
use std::borrow::Cow;

pub enum ServerControllerTemplate {
  Asset(GenericControllerTemplate),
  Application(Application),
}

impl ControllerTemplate for ServerControllerTemplate {
  type Error = GenericError;
  type Controller = Box<dyn Controller<Input = Vec<u8>, Output = Vec<u8>> + Send + Sync + 'static>;

  fn blank(&self) -> Self::Controller {
    match self {
      ServerControllerTemplate::Asset(a) => a.blank(),
      ServerControllerTemplate::Application(Application::Editor) => Box::new(SerdeController(Editor.blank())),
    }
  }

  fn load_json(&self, value: Value) -> Result<Self::Controller, LoadError<Error, Self::Error>> {
    match self {
      ServerControllerTemplate::Asset(a) => a.load_json(value),
      ServerControllerTemplate::Application(Application::Editor) => Ok(Box::new(SerdeController(Editor.load_json(value).map_err(|e| e.boxed())?))),
    }
  }

  fn load_message_pack(&self, de: MessagePackDeserializer) -> Result<Self::Controller, LoadError<rmp_serde::decode::Error, Self::Error>> {
    match self {
      ServerControllerTemplate::Asset(a) => a.load_message_pack(de),
      ServerControllerTemplate::Application(Application::Editor) => {
        Ok(Box::new(SerdeController(Editor.load_message_pack(de).map_err(|e| e.boxed())?)))
      }
    }
  }

  fn name(&self, owner: &str) -> Cow<'static, str> {
    match self {
      ServerControllerTemplate::Asset(a) => a.name(owner),
      ServerControllerTemplate::Application(Application::Editor) => Editor.name(owner),
    }
  }
}
