pub mod oneshot_timeout;

use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct TrackingMap<T> {
  next_id: u32,
  active: BTreeMap<u32, T>,
}

pub trait Expires {
  fn end_of_life(&self) -> DateTime<Utc>;
}

impl<T> TrackingMap<T> {
  pub fn add<R>(&mut self, value: T, response: impl FnOnce(u32, &T) -> R) -> R {
    let id = loop {
      let id = self.next_id;
      self.next_id = self.next_id.wrapping_add(1);
      if !self.active.contains_key(&id) {
        break id;
      }
    };
    let result = response(id, &value);
    self.active.insert(id, value);
    result
  }
  pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
    self.active.get_mut(&id)
  }
  pub fn finish(&mut self, id: u32) -> Option<T> {
    self.active.remove(&id)
  }
}
impl<T: Expires> TrackingMap<T> {
  pub fn purge_expired(&mut self) {
    let now = Utc::now();
    self.active.retain(|_, v| v.end_of_life() > now)
  }
}
impl<T> Default for TrackingMap<T> {
  fn default() -> Self {
    TrackingMap { next_id: 0, active: Default::default() }
  }
}
