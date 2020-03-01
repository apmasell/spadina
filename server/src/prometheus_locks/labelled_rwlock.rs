use std::ops::{Deref, DerefMut};
pub struct PrometheusLabelled {
  wait: prometheus::HistogramVec,
  hold: prometheus::HistogramVec,
}
pub struct PrometheusLabelledRwLock<'a, T> {
  label: String,
  lock: tokio::sync::RwLock<T>,
  owner: &'a PrometheusLabelled,
}

pub struct PrometheusLabelledReadGuard<'a, T> {
  guard: tokio::sync::RwLockReadGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}

pub struct PrometheusLabelledWriteGuard<'a, T> {
  guard: tokio::sync::RwLockWriteGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}
impl PrometheusLabelled {
  pub fn new(name: &str, purpose: &str, label_name: &str) -> prometheus::Result<Self> {
    Ok(Self {
      wait: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_waiting", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["mode", label_name],
      )?,
      hold: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_holding", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["location", "mode", label_name],
      )?,
    })
  }
  pub fn create<T>(&self, label: impl Into<String>, t: T) -> PrometheusLabelledRwLock<'_, T> {
    PrometheusLabelledRwLock { lock: tokio::sync::RwLock::new(t), label: label.into(), owner: self }
  }
}
impl<'a, T: Sized> PrometheusLabelledRwLock<'a, T> {
  pub async fn read(&self, location: &str) -> PrometheusLabelledReadGuard<'_, T> {
    let wait = self.owner.wait.with_label_values(&["read", &self.label]).start_timer();
    let guard = self.lock.read().await;
    wait.observe_duration();
    let hold = self.owner.hold.with_label_values(&[location, "read", &self.label]).start_timer();
    PrometheusLabelledReadGuard { hold, guard }
  }
  pub async fn write(&self, location: &str) -> PrometheusLabelledWriteGuard<'_, T> {
    let wait = self.owner.wait.with_label_values(&["write", &self.label]).start_timer();
    let guard = self.lock.write().await;
    wait.observe_duration();
    let hold = self.owner.hold.with_label_values(&[location, "write", &self.label]).start_timer();
    PrometheusLabelledWriteGuard { hold, guard }
  }
}
impl<T: Sized> Deref for PrometheusLabelledReadGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    self.guard.deref()
  }
}

impl<T: Sized> Deref for PrometheusLabelledWriteGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    self.guard.deref()
  }
}

impl<T: Sized> DerefMut for PrometheusLabelledWriteGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut T {
    self.guard.deref_mut()
  }
}
