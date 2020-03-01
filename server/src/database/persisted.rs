pub(crate) struct PersistedGlobal<'a, G: Persistance + crate::prometheus_locks::LabelledValue<N>, N: prometheus_client::encoding::EncodeLabelSet> {
  database: std::sync::Arc<super::Database>,
  discriminator: G,
  labels: N,
  value: tokio::sync::RwLock<G::Value>,
  prometheus: &'a crate::prometheus_locks::PrometheusLabelled<crate::prometheus_locks::rwlock::RwLockStatus<N>>,
}
pub(crate) struct PersistedLocal<G: Persistance> {
  database: std::sync::Arc<super::Database>,
  discriminator: G,
  value: G::Value,
}
pub(crate) struct PersistedWatch<G: Persistance> {
  database: std::sync::Arc<super::Database>,
  discriminator: G,
  sender: tokio::sync::watch::Sender<G::Value>,
  receiver: tokio::sync::watch::Receiver<G::Value>,
}
pub(crate) trait Persistance: Copy {
  type Value: Clone + Send + Sync;
  fn load(&self, database: &super::Database) -> diesel::result::QueryResult<Self::Value>;
  fn store(&self, database: &super::Database, value: &Self::Value) -> diesel::result::QueryResult<()>;
}

impl<G: Persistance> PersistedLocal<G> {
  pub fn new(database: std::sync::Arc<super::Database>, discriminator: G) -> diesel::result::QueryResult<Self> {
    let value = discriminator.load(&database)?;
    Ok(Self { discriminator, database, value: value })
  }
  pub fn read(&self) -> &G::Value {
    &self.value
  }
  pub fn mutate(&mut self, mutator: impl FnOnce(&mut G::Value) -> spadina_core::UpdateResult) -> spadina_core::UpdateResult {
    let result = mutator(&mut self.value);
    if result == spadina_core::UpdateResult::Success {
      match self.discriminator.store(&self.database, &self.value) {
        Err(e) => {
          eprintln!("Failed to persist local value to database: {}", e);
          spadina_core::UpdateResult::InternalError
        }
        Ok(()) => spadina_core::UpdateResult::Success,
      }
    } else {
      result
    }
  }
}
impl<
    'a,
    G: Persistance + crate::prometheus_locks::LabelledValue<N>,
    N: prometheus_client::encoding::EncodeLabelSet + Clone + std::hash::Hash + Eq + std::fmt::Debug + Send + Sync + 'static,
  > PersistedGlobal<'a, G, N>
{
  pub fn new(
    database: std::sync::Arc<super::Database>,
    discriminator: G,
    prometheus: &'a crate::prometheus_locks::PrometheusLabelled<crate::prometheus_locks::rwlock::RwLockStatus<N>>,
  ) -> diesel::result::QueryResult<Self> {
    let value = discriminator.load(&database)?;
    let labels = discriminator.labels();
    Ok(Self { discriminator, labels, database, value: tokio::sync::RwLock::new(value), prometheus })
  }
  pub async fn read<'b, R>(&'b self, location: &'static str, read: impl FnOnce(&G::Value) -> R) -> R
  where
    'a: 'b,
  {
    let guard = self.prometheus.acquire(location, crate::prometheus_locks::rwlock::RwLockStatus(self.labels.clone(), false), self.value.read()).await;
    read(&*guard)
  }
  pub async fn write<'b, F>(&'b self, location: &'static str, write: F) -> spadina_core::UpdateResult
  where
    'a: 'b,
    for<'w> F: FnOnce(&'w mut G::Value),
  {
    let mut guard =
      self.prometheus.acquire(location, crate::prometheus_locks::rwlock::RwLockStatus(self.labels.clone(), true), self.value.write()).await;
    write(&mut *guard);
    match self.discriminator.store(&self.database, &*guard) {
      Err(e) => {
        eprintln!("Failed to persist global value to database: {}", e);
        spadina_core::UpdateResult::InternalError
      }
      Ok(()) => spadina_core::UpdateResult::Success,
    }
  }
}
impl<G: Persistance> PersistedWatch<G> {
  pub fn new(database: std::sync::Arc<super::Database>, discriminator: G) -> diesel::result::QueryResult<Self> {
    let (sender, receiver) = tokio::sync::watch::channel(discriminator.load(&database)?);
    Ok(Self { discriminator, database, sender, receiver })
  }
  pub fn read(&self) -> G::Value {
    self.receiver.borrow().clone()
  }
  pub fn watch(&self) -> tokio::sync::watch::Receiver<G::Value> {
    self.receiver.clone()
  }
  pub fn write(&self, writer: impl FnOnce(&mut G::Value)) -> spadina_core::UpdateResult {
    self.sender.send_modify(writer);
    match self.discriminator.store(&self.database, &*self.receiver.borrow()) {
      Err(e) => {
        eprintln!("Failed to persist global value to database: {}", e);
        spadina_core::UpdateResult::InternalError
      }
      Ok(_) => spadina_core::UpdateResult::Success,
    }
  }
}
