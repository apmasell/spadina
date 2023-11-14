use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::Histogram;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

pub struct ExecutionTimeMonitor<'a, F, L> {
  inner: F,
  duration: Duration,
  labels: L,
  family: &'a Family<L, Histogram>,
}

impl<'a, F, L> ExecutionTimeMonitor<'a, F, L> {
  pub fn new(inner: F, labels: L, family: &'a Family<L, Histogram>) -> Self {
    Self { inner, labels, family, duration: Default::default() }
  }
}

impl<'a, F: Future + Unpin, L: EncodeLabelSet + Clone + Hash + Eq + Unpin> Future for ExecutionTimeMonitor<'a, F, L> {
  type Output = F::Output;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let Self { inner, duration, labels, family } = self.get_mut();
    let start = SystemTime::now();
    let result = Pin::new(inner).poll(cx);
    if let Some(elapsed) = SystemTime::now().duration_since(start).ok() {
      *duration += elapsed;
    }
    match result {
      Poll::Pending => Poll::Pending,
      Poll::Ready(value) => {
        family.get_or_create(labels).observe(duration.as_secs_f64());
        Poll::Ready(value)
      }
    }
  }
}
