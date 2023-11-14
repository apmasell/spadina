use spadina_core::location::directory::Activity;
use std::fmt;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct AtomicActivity(Arc<AtomicU16>);

impl AtomicActivity {
  pub fn clear(&self) {
    self.0.store(0, Ordering::Relaxed)
  }
  pub fn get(&self) -> Activity {
    match self.0.load(Ordering::Relaxed) {
      0 => Activity::Deserted,
      1..=19 => Activity::Quiet,
      20..=99 => Activity::Popular,
      100..=499 => Activity::Busy,
      _ => Activity::Crowded,
    }
  }
  pub fn update(&self, player_count: usize) {
    let player_count = player_count.try_into().unwrap_or(u16::MAX);
    let _ = self.0.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| Some(old.saturating_add(player_count) / 2));
  }
}
impl fmt::Debug for AtomicActivity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("AtomicActivity").field(&self.get()).finish()
  }
}
