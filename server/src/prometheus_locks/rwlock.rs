use std::ops::{Deref, DerefMut};
pub struct PrometheusRwLock<T> {
  lock: tokio::sync::RwLock<T>,
  wait: prometheus::HistogramVec,
  hold: prometheus::HistogramVec,
}

pub struct PrometheusReadGuard<'a, T> {
  guard: tokio::sync::RwLockReadGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}

pub struct PrometheusWriteGuard<'a, T> {
  guard: tokio::sync::RwLockWriteGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}
impl<T: Sized> PrometheusRwLock<T> {
  pub fn new(name: &str, purpose: &str, t: T) -> prometheus::Result<Self> {
    Ok(Self {
      lock: tokio::sync::RwLock::new(t),
      wait: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_waiting", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["mode"],
      )?,
      hold: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_holding", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["location", "mode"],
      )?,
    })
  }

  pub async fn read(&self, location: &str) -> PrometheusReadGuard<'_, T> {
    let wait = self.wait.with_label_values(&["read"]).start_timer();
    let guard = self.lock.read().await;
    wait.observe_duration();
    let hold = self.hold.with_label_values(&[location, "read"]).start_timer();
    PrometheusReadGuard { hold, guard }
  }
  pub async fn write(&self, location: &str) -> PrometheusWriteGuard<'_, T> {
    let wait = self.wait.with_label_values(&["write"]).start_timer();
    let guard = self.lock.write().await;
    wait.observe_duration();
    let hold = self.hold.with_label_values(&[location, "write"]).start_timer();
    PrometheusWriteGuard { hold, guard }
  }
}
impl<T: Sized> Deref for PrometheusReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    self.guard.deref()
  }
}

impl<T: Sized> Deref for PrometheusWriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    self.guard.deref()
  }
}

impl<T: Sized> DerefMut for PrometheusWriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut T {
    self.guard.deref_mut()
  }
}
