use crate::database::persisted::Persistence;
use crate::database::Database;
use crate::metrics::SettingLabel;
use crate::prometheus_locks::LabelledValue;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

pub trait Setting: Copy + Send + Sync {
  const CODE: u8;
  const METRIC: &'static str;
  type Stored: Serialize + DeserializeOwned + Default + Clone + Send + Sync + Debug + 'static;
}
impl<S: Setting> Persistence for S {
  type Value = S::Stored;
  fn load(&self, database: &Database) -> diesel::result::QueryResult<Self::Value> {
    database.setting_read::<S>()
  }

  fn store(&self, database: &Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.setting_write::<S>(value)
  }
}
impl<S: Setting> LabelledValue<SettingLabel> for S {
  fn labels(&self) -> SettingLabel {
    SettingLabel { setting: S::METRIC }
  }
}
