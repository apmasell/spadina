#[macro_use]
pub mod mutex;
pub mod rwlock;

use prometheus_client::encoding::EncodeLabelSet;
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::Histogram;
use tokio::time::Instant;

pub trait LabelledValue<N: EncodeLabelSet>: Sized + Send + Sync {
  fn labels(&self) -> N;
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct LocationLabel<N: EncodeLabelSet>(&'static str, N);
pub trait Acquirable<'a, N> {
  type Guard: 'a;
  fn labels(&self) -> N;
  fn acquire(self) -> std::pin::Pin<Box<dyn Future<Output = Self::Guard> + Send + 'a>>;
}

#[derive(Debug)]
pub struct PrometheusLabelled<N: EncodeLabelSet> {
  hold: Family<LocationLabel<N>, Histogram>,
  wait: Family<LocationLabel<N>, Histogram>,
}
impl<N: EncodeLabelSet> EncodeLabelSet for LocationLabel<N> {
  fn encode(&self, mut encoder: prometheus_client::encoding::LabelSetEncoder) -> Result<(), std::fmt::Error> {
    let mut label_encoder = encoder.encode_label();
    let mut label_key_encoder = label_encoder.encode_label_key()?;
    prometheus_client::encoding::EncodeLabelKey::encode(&"location", &mut label_key_encoder)?;

    let mut label_value_encoder = label_key_encoder.encode_label_value()?;
    prometheus_client::encoding::EncodeLabelValue::encode(&self.0, &mut label_value_encoder)?;

    label_value_encoder.finish()?;
    self.1.encode(encoder)
  }
}
impl<N: EncodeLabelSet + Hash + Eq + Clone> Default for PrometheusLabelled<N> {
  fn default() -> Self {
    fn create_histogram() -> Histogram {
      Histogram::new([0.1, 0.5, 1.0, 5.0, 10.0, 60.0, 300.0, 600.0].into_iter())
    }
    Self { hold: Family::new_with_constructor(create_histogram), wait: Family::new_with_constructor(create_histogram) }
  }
}
impl<N: EncodeLabelSet + Clone + Hash + Eq + Debug + Send + Sync + 'static> PrometheusLabelled<N> {
  pub fn register(&self, registry: &mut prometheus_client::registry::Registry, name: &str, purpose: &str) {
    registry.register(format!("spadina_{}_waiting", name), format!("The time spent waiting to acquire a lock for {}.", purpose), self.wait.clone());
    registry.register(format!("spadina_{}_holding", name), format!("The time spent waiting to acquire a lock for {}.", purpose), self.hold.clone());
  }

  pub async fn acquire<'a, Guard: 'a>(
    &'a self,
    location: &'static str,
    acquisition_labels: N,
    acquire: impl Future<Output = Guard> + Send + 'a,
  ) -> PrometheusLabelledGuard<'a, N, Guard> {
    let labels = LocationLabel(location, acquisition_labels);
    let wait = Instant::now();
    let guard = acquire.await;
    let hold = Instant::now();
    self.wait.get_or_create(&labels).observe((hold - wait).as_secs_f64());

    PrometheusLabelledGuard { hold, labels, owner: self, guard }
  }
}
pub struct PrometheusLabelledGuard<'a, N: EncodeLabelSet + Clone + Eq + PartialEq + Debug + Hash, G> {
  guard: G,
  labels: LocationLabel<N>,
  owner: &'a PrometheusLabelled<N>,
  hold: Instant,
}
impl<'a, N: EncodeLabelSet + Clone + Eq + PartialEq + Debug + Hash, G: Deref> Deref for PrometheusLabelledGuard<'a, N, G> {
  type Target = G::Target;

  fn deref(&self) -> &G::Target {
    self.guard.deref()
  }
}
impl<'a, N: EncodeLabelSet + Clone + Eq + PartialEq + Debug + Hash, G> Drop for PrometheusLabelledGuard<'a, N, G> {
  fn drop(&mut self) {
    self.owner.hold.get_or_create(&self.labels).observe((Instant::now() - self.hold).as_secs_f64());
  }
}

impl<'a, N: EncodeLabelSet + Clone + Eq + PartialEq + Debug + Hash, G: DerefMut> DerefMut for PrometheusLabelledGuard<'a, N, G> {
  fn deref_mut(&mut self) -> &mut G::Target {
    self.guard.deref_mut()
  }
}
impl<T: Sized + Send + Sync + EncodeLabelSet + Clone + Eq + PartialEq + Debug + Hash> LabelledValue<T> for T {
  fn labels(&self) -> T {
    self.clone()
  }
}
