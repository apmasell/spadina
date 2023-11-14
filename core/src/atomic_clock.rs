use chrono::{DateTime, Utc};
use serde::Serializer;

pub struct AtomicClock(std::sync::atomic::AtomicI64);

impl AtomicClock {
  pub fn now() -> Self {
    AtomicClock(Utc::now().timestamp().into())
  }
  pub fn read(&self) -> DateTime<Utc> {
    DateTime::from_timestamp(self.0.load(std::sync::atomic::Ordering::Relaxed), 0).expect("Failed to create timestamp from current value")
  }
  pub fn reset(&self) {
    self.0.store(Utc::now().timestamp(), std::sync::atomic::Ordering::Relaxed)
  }
}

impl From<DateTime<Utc>> for AtomicClock {
  fn from(value: DateTime<Utc>) -> Self {
    AtomicClock(value.timestamp().into())
  }
}
impl<'de> serde::Deserialize<'de> for AtomicClock {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    DateTime::<Utc>::deserialize(deserializer).map(|t| t.into())
  }
}

impl serde::Serialize for AtomicClock {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.read().serialize(serializer)
  }
}
