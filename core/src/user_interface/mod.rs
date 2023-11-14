pub trait Renderer: 'static + Send + Sync {
  type Id: 'static + Copy + Send + Sync + serde::de::DeserializeOwned + serde::Serialize;
}
pub enum Action {
  Perform(serde_json::Value),
  Dialog(Dialog),
  Menu(Vec<(String, Action)>),
}
pub enum Interaction {
  Click(Action),
  Drag(Action, u32),
}

pub struct WorldElement<T: Renderer> {
  pub id: T::Id,
  pub data: T,
  pub click: Option<Action>,
  pub drag_source: Option<u32>,
  pub drag_recipient: (),
}

pub struct Dialog {
  pub controls: Vec<Widget>,
}
pub enum Widget {
  Bool { property: String, label: String },
  Id { property: String },
}
pub enum Build<Extra: serde::Serialize + 'static> {
  Bool { property: String, value: bool },
  Extra { property: String, value: Extra },
  Id { property: String },
}
