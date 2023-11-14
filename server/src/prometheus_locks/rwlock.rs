use prometheus_client::encoding::{EncodeLabelSet, LabelSetEncoder};

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct RwLockStatus<T>(pub(crate) T, pub(crate) bool);

#[allow(dead_code)]
pub struct PrometheusLabelledRwLock<'a, T: super::LabelledValue<N>, N: EncodeLabelSet + 'static> {
  labels: N,
  lock: tokio::sync::RwLock<T>,
  owner: &'a super::PrometheusLabelled<RwLockStatus<N>>,
}

impl<'a, T: super::LabelledValue<N>, N: EncodeLabelSet + Clone + Eq + PartialEq + std::hash::Hash + Sync + Send + std::fmt::Debug + 'static>
  PrometheusLabelledRwLock<'a, T, N>
{
  #[allow(dead_code)]
  pub fn new(owner: &'a super::PrometheusLabelled<RwLockStatus<N>>, item: T) -> PrometheusLabelledRwLock<'a, T, N> {
    PrometheusLabelledRwLock { labels: item.labels(), lock: tokio::sync::RwLock::new(item), owner }
  }
  #[allow(dead_code)]
  pub async fn read(&'a self, location: &'static str) -> super::PrometheusLabelledGuard<'a, RwLockStatus<N>, tokio::sync::RwLockReadGuard<'a, T>> {
    self.owner.acquire(location, RwLockStatus(self.labels.clone(), false), self.lock.read()).await
  }
  #[allow(dead_code)]
  pub async fn write(&'a self, location: &'static str) -> super::PrometheusLabelledGuard<'a, RwLockStatus<N>, tokio::sync::RwLockWriteGuard<'a, T>> {
    self.owner.acquire(location, RwLockStatus(self.labels.clone(), true), self.lock.write()).await
  }
}
impl<T: EncodeLabelSet> EncodeLabelSet for RwLockStatus<T> {
  fn encode(&self, mut encoder: LabelSetEncoder) -> Result<(), std::fmt::Error> {
    let mut label_encoder = encoder.encode_label();
    let mut label_key_encoder = label_encoder.encode_label_key()?;
    prometheus_client::encoding::EncodeLabelKey::encode(&"operation", &mut label_key_encoder)?;

    let mut label_value_encoder = label_key_encoder.encode_label_value()?;
    prometheus_client::encoding::EncodeLabelValue::encode(if self.1 { &"write" } else { &"read" }, &mut label_value_encoder)?;

    label_value_encoder.finish()?;
    self.0.encode(encoder)
  }
}
