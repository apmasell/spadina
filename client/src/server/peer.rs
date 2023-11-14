use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::Deref;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Peer(String);

impl From<String> for Peer {
  fn from(value: String) -> Self {
    Peer(value)
  }
}
impl Serialize for Peer {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(&self.0)
  }
}

impl<'de> Deserialize<'de> for Peer {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    Ok(Peer(String::deserialize(deserializer)?))
  }
}
impl Deref for Peer {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.0.as_str()
  }
}
