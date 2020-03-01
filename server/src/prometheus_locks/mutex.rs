pub struct PrometheusLabelledMutex<'a, T: Send, N: prometheus_client::encoding::EncodeLabelSet> {
  labels: N,
  mutex: tokio::sync::Mutex<T>,
  owner: &'a super::PrometheusLabelled<N>,
}

impl<'a, T: super::LabelledValue<N>, N: prometheus_client::encoding::EncodeLabelSet> PrometheusLabelledMutex<'a, T, N> {
  #[allow(dead_code)]
  pub fn new(owner: &'a super::PrometheusLabelled<N>, item: T) -> PrometheusLabelledMutex<T, N> {
    PrometheusLabelledMutex { owner, labels: item.labels(), mutex: tokio::sync::Mutex::new(item) }
  }
}
impl<
    'a,
    T: Send,
    N: prometheus_client::encoding::EncodeLabelSet + Clone + Eq + PartialEq + std::hash::Hash + Sync + Send + std::fmt::Debug + 'static,
  > PrometheusLabelledMutex<'a, T, N>
{
  pub fn new_with_labels(owner: &'a super::PrometheusLabelled<N>, item: T, labels: N) -> PrometheusLabelledMutex<T, N> {
    PrometheusLabelledMutex { owner, labels, mutex: tokio::sync::Mutex::new(item) }
  }
  pub async fn lock(&'a self, location: &'static str) -> super::PrometheusLabelledGuard<'a, N, tokio::sync::MutexGuard<'a, T>> {
    self.owner.acquire(location, self.labels.clone(), self.mutex.lock()).await
  }
}
impl<'a, T: Send, N: prometheus_client::encoding::EncodeLabelSet + std::fmt::Debug> std::fmt::Debug for PrometheusLabelledMutex<'a, T, N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PrometheusLabelledMutex").field("labels", &self.labels).finish()
  }
}
