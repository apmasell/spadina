use crate::gc_map::CollectionStrategy;
use chrono::{DateTime, Utc};

pub struct TimeUse {
  last_update: DateTime<Utc>,
  accumulator: usize,
}

impl Default for TimeUse {
  fn default() -> Self {
    TimeUse { accumulator: 0, last_update: Utc::now() }
  }
}
impl CollectionStrategy for TimeUse {
  type Pressure = usize;

  fn collection_pressure(&self) -> Self::Pressure {
    self.accumulator
  }

  fn notify_used(&mut self, active: usize) {
    let now = Utc::now();
    self.accumulator = (self.accumulator / 2)
      .saturating_add((active as i64).saturating_mul((now - self.last_update).num_seconds()).clamp(0, usize::MAX as i64) as usize);
    self.last_update = now;
  }
}
