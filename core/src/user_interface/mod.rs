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
  id: T::Id,
  data: T,
  click: Option<Action>,
  drag_source: Option<u32>,
  drag_recipient: (),
}

pub struct Dialog {
  controls: Vec<Widget>,
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
