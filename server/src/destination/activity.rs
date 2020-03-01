#[derive(Clone, Default)]
pub struct AtomicActivity(std::sync::Arc<std::sync::atomic::AtomicU16>);

impl AtomicActivity {
  pub fn get(&self) -> spadina_core::realm::RealmActivity {
    match self.0.load(std::sync::atomic::Ordering::Relaxed) {
      0 => spadina_core::realm::RealmActivity::Deserted,
      1..=19 => spadina_core::realm::RealmActivity::Quiet,
      20..=99 => spadina_core::realm::RealmActivity::Popular,
      100..=499 => spadina_core::realm::RealmActivity::Busy,
      _ => spadina_core::realm::RealmActivity::Crowded,
    }
  }
  pub fn update(&self, player_count: usize) {
    let player_count = player_count.try_into().unwrap_or(std::u16::MAX);
    let _ = self
      .0
      .fetch_update(std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed, |old| Some(old.saturating_add(player_count) / 2));
  }
}
impl std::fmt::Debug for AtomicActivity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("AtomicActivity").field(&self.get()).finish()
  }
}
