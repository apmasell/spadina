use std::fmt::Write;

pub enum ShStr {
  Single(String),
  Shared(std::sync::Arc<str>),
}
impl ShStr {
  pub fn as_str<'a>(&'a self) -> &'a str {
    match self {
      ShStr::Single(s) => s.as_str(),
      ShStr::Shared(s) => s.as_ref(),
    }
  }
  pub fn upgrade(self) -> Self {
    match self {
      ShStr::Single(s) => ShStr::Single(s.into()),
      ShStr::Shared(s) => ShStr::Shared(s),
    }
  }
  pub fn upgrade_in_place(&mut self) {
    replace_with::replace_with_or_abort(self, |v| v.upgrade())
  }
  pub fn to_arc(self) -> std::sync::Arc<str> {
    self.into()
  }
}
impl From<String> for ShStr {
  fn from(value: String) -> Self {
    ShStr::Single(value)
  }
}
impl From<std::sync::Arc<str>> for ShStr {
  fn from(value: std::sync::Arc<str>) -> Self {
    Self::Shared(value)
  }
}
impl From<&std::sync::Arc<str>> for ShStr {
  fn from(value: &std::sync::Arc<str>) -> Self {
    Self::Shared(value.clone())
  }
}
impl From<ShStr> for String {
  fn from(value: ShStr) -> Self {
    match value {
      ShStr::Single(s) => s,
      ShStr::Shared(s) => s.to_string(),
    }
  }
}
impl From<ShStr> for std::sync::Arc<str> {
  fn from(value: ShStr) -> Self {
    match value {
      ShStr::Single(s) => std::sync::Arc::from(s),
      ShStr::Shared(s) => s,
    }
  }
}
impl AsRef<str> for ShStr {
  fn as_ref(&self) -> &str {
    match self {
      ShStr::Single(s) => s.as_str(),
      ShStr::Shared(s) => s.as_ref(),
    }
  }
}
impl std::borrow::Borrow<str> for ShStr {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}
impl Clone for ShStr {
  fn clone(&self) -> Self {
    match self {
      Self::Single(s) => Self::Single(s.clone()),
      Self::Shared(s) => Self::Shared(s.clone()),
    }
  }
}
impl serde::Serialize for ShStr {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(self.as_str())
  }
}
impl<'de> serde::Deserialize<'de> for ShStr {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    Ok(ShStr::Single(String::deserialize(deserializer)?))
  }
}
impl std::cmp::Eq for ShStr {}
impl std::cmp::Ord for ShStr {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.as_str().cmp(other.as_str())
  }
}
impl std::cmp::PartialEq for ShStr {
  fn eq(&self, other: &Self) -> bool {
    self.as_str() == other.as_str()
  }
}
impl std::cmp::PartialOrd for ShStr {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}
impl std::hash::Hash for ShStr {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_str().hash(state);
  }
}
impl std::fmt::Debug for ShStr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}
impl std::fmt::Display for ShStr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}
impl prometheus_client::encoding::EncodeLabelValue for ShStr {
  fn encode(&self, encoder: &mut prometheus_client::encoding::LabelValueEncoder) -> Result<(), std::fmt::Error> {
    encoder.write_str(self.as_str())
  }
}
