use std::ops::{Deref, DerefMut};

pub(crate) struct PrometheusLabelled {
  hold: prometheus::HistogramVec,
  wait: prometheus::HistogramVec,
}

pub(crate) struct PrometheusLabelledMutex<'a, T: Sized> {
  label: String,
  mutex: tokio::sync::Mutex<T>,
  owner: &'a PrometheusLabelled,
}

pub struct PrometheusLabelledMutexGuard<'a, T: Sized> {
  guard: tokio::sync::MutexGuard<'a, T>,
  #[allow(dead_code)]
  hold: prometheus::HistogramTimer,
}
impl PrometheusLabelled {
  pub fn new(name: &str, purpose: &str, label_name: &str) -> prometheus::Result<Self> {
    Ok(Self {
      wait: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_waiting", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &[label_name],
      )?,
      hold: prometheus::HistogramVec::new(
        prometheus::HistogramOpts::new(format!("puzzleverse_{}_holding", name), format!("The time spent waiting to acquire a lock for {}.", purpose))
          .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0]),
        &["location", label_name],
      )?,
    })
  }
  pub fn create<T: Sized>(&self, label: impl Into<String>, item: T) -> PrometheusLabelledMutex<T> {
    PrometheusLabelledMutex { owner: self, label: label.into(), mutex: tokio::sync::Mutex::new(item) }
  }
}
impl<'a, T: Sized> PrometheusLabelledMutex<'a, T> {
  pub async fn lock(&self, location: &str) -> PrometheusLabelledMutexGuard<'_, T> {
    let wait = self.owner.wait.with_label_values(&[&self.label]).start_timer();
    let guard = self.mutex.lock().await;
    wait.observe_duration();
    let hold = self.owner.hold.with_label_values(&[location, &self.label]).start_timer();
    PrometheusLabelledMutexGuard { hold, guard }
  }
}

impl<T: Sized> Deref for PrometheusLabelledMutexGuard<'_, T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    self.guard.deref()
  }
}

impl<T: Sized> DerefMut for PrometheusLabelledMutexGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.guard.deref_mut()
  }
}
