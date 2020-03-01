use std::ops::{Deref, DerefMut};

pub(crate) struct PrometheusMutex<T: Sized> {
  mutex: tokio::sync::Mutex<T>,
  wait: prometheus::Histogram,
  hold: prometheus::HistogramVec,
}

pub struct PrometheusMutexGuard<'a, T: Sized> {
  guard: tokio::sync::MutexGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}
impl<T: Sized> PrometheusMutex<T> {
  pub fn new(name: &str, purpose: &str, t: T) -> prometheus::Result<Self> {
    Ok(Self {
      mutex: tokio::sync::Mutex::new(t),
      wait: prometheus::Histogram::with_opts(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_waiting", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
      )?,
      hold: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_holding", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["location"],
      )?,
    })
  }

  pub async fn lock(&self, location: &str) -> PrometheusMutexGuard<'_, T> {
    let wait = self.wait.start_timer();
    let guard = self.mutex.lock().await;
    wait.observe_duration();
    let hold = self.hold.with_label_values(&[location]).start_timer();
    PrometheusMutexGuard { hold, guard }
  }
}

impl<T: Sized> Deref for PrometheusMutexGuard<'_, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    self.guard.deref()
  }
}

impl<T: Sized> DerefMut for PrometheusMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.guard.deref_mut()
  }
}
