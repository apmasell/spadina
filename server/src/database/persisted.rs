use crate::prometheus_locks::rwlock::RwLockStatus;
use crate::prometheus_locks::{LabelledValue, PrometheusLabelled};
use diesel::result::QueryResult;
use prometheus_client::encoding::EncodeLabelSet;
use spadina_core::UpdateResult;
use std::ops::Deref;
use tokio::sync::watch;

pub(crate) struct PersistedGlobal<'a, G: Persistence + LabelledValue<N>, N: EncodeLabelSet> {
  database: super::Database,
  discriminator: G,
  labels: N,
  value: tokio::sync::RwLock<G::Value>,
  prometheus: &'a PrometheusLabelled<RwLockStatus<N>>,
}
pub(crate) struct PersistedLocal<G: Persistence> {
  database: super::Database,
  discriminator: G,
  value: G::Value,
}
pub(crate) struct PersistedWatch<G: Persistence> {
  database: super::Database,
  discriminator: G,
  sender: watch::Sender<G::Value>,
  receiver: watch::Receiver<G::Value>,
}
pub(crate) trait Persistence: Copy {
  type Value: Clone + Send + Sync;
  fn load(&self, database: &super::Database) -> QueryResult<Self::Value>;
  fn store(&self, database: &super::Database, value: &Self::Value) -> QueryResult<()>;
}

impl<G: Persistence> PersistedLocal<G> {
  pub fn new(database: super::Database, discriminator: G) -> QueryResult<Self> {
    let value = discriminator.load(&database)?;
    Ok(Self { discriminator, database, value })
  }
  pub fn read(&self) -> &G::Value {
    &self.value
  }
  pub fn mutate(&mut self, mutator: impl FnOnce(&mut G::Value) -> UpdateResult) -> UpdateResult {
    let result = mutator(&mut self.value);
    if result == UpdateResult::Success {
      match self.discriminator.store(&self.database, &self.value) {
        Err(e) => {
          eprintln!("Failed to persist local value to database: {}", e);
          UpdateResult::InternalError
        }
        Ok(()) => UpdateResult::Success,
      }
    } else {
      result
    }
  }
}
impl<G: Persistence> Deref for PersistedLocal<G> {
  type Target = G::Value;

  fn deref(&self) -> &Self::Target {
    self.read()
  }
}
impl<'a, G: Persistence + LabelledValue<N>, N: EncodeLabelSet + Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static>
  PersistedGlobal<'a, G, N>
{
  pub fn new(database: super::Database, discriminator: G, prometheus: &'a PrometheusLabelled<RwLockStatus<N>>) -> QueryResult<Self> {
    let value = discriminator.load(&database)?;
    let labels = discriminator.labels();
    Ok(Self { discriminator, labels, database, value: tokio::sync::RwLock::new(value), prometheus })
  }
  pub async fn read<'b, R>(&'b self, location: &'static str, read: impl FnOnce(&G::Value) -> R) -> R
  where
    'a: 'b,
  {
    let guard = self.prometheus.acquire(location, RwLockStatus(self.labels.clone(), false), self.value.read()).await;
    read(&*guard)
  }
  pub async fn write<'b, F>(&'b self, location: &'static str, write: F) -> UpdateResult
  where
    'a: 'b,
    for<'w> F: FnOnce(&'w mut G::Value) -> Option<bool>,
  {
    let mut guard = self.prometheus.acquire(location, RwLockStatus(self.labels.clone(), true), self.value.write()).await;
    write(&mut *guard)
      .map(|changed| {
        if changed {
          match self.discriminator.store(&self.database, &*guard) {
            Err(e) => {
              eprintln!("Failed to persist global value to database: {}", e);
              UpdateResult::InternalError
            }
            Ok(()) => UpdateResult::Success,
          }
        } else {
          UpdateResult::Redundant
        }
      })
      .unwrap_or(UpdateResult::InternalError)
  }
}
impl<G: Persistence> PersistedWatch<G> {
  pub fn new(database: super::Database, discriminator: G) -> QueryResult<Self> {
    let (sender, receiver) = watch::channel(discriminator.load(&database)?);
    Ok(Self { discriminator, database, sender, receiver })
  }
  pub fn read(&self) -> G::Value {
    self.receiver.borrow().clone()
  }
  pub fn watch(&self) -> watch::Receiver<G::Value> {
    self.receiver.clone()
  }
  pub fn write(&self, writer: impl FnOnce(&mut G::Value)) -> UpdateResult {
    self.sender.send_modify(writer);
    match self.discriminator.store(&self.database, &*self.receiver.borrow()) {
      Err(e) => {
        eprintln!("Failed to persist global value to database: {}", e);
        UpdateResult::InternalError
      }
      Ok(_) => UpdateResult::Success,
    }
  }
}
