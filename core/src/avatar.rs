use serde::Deserialize;
use serde::Serialize;
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Avatar {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  timezone: Option<i8>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Effect {
  Normal,
  Sparkling(crate::asset::Color),
  Tinted(crate::asset::Color),
}

impl Default for Avatar {
  fn default() -> Self {
    Self { timezone: Default::default() }
  }
}
