use crate::database::persisted::Persistence;
use crate::database::Database;
use diesel::result::QueryResult;
use spadina_core::access::{AccessSetting, Privilege};
use spadina_core::location::communication::Announcement;
use spadina_core::reference_converter::AsArc;
use std::sync::Arc;

#[derive(Copy, Clone)]
pub(crate) struct LocationAccess(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct LocationAnnouncements(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct LocationName(pub i32);

impl Persistence for LocationAccess {
  type Value = AccessSetting<Arc<str>, Privilege>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.location_acl_read(self.0)
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.location_acl_write(self.0, value)
  }
}

impl Persistence for LocationAnnouncements {
  type Value = Vec<Announcement<Arc<str>>>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    Ok(database.location_announcements_read(self.0)?.into_iter().map(|a| a.convert(AsArc::<str>::default())).collect())
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.location_announcements_write(self.0, value)
  }
}
impl Persistence for LocationName {
  type Value = Arc<str>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.location_name_read(self.0).map(|name| Arc::from(name))
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.location_name_write(self.0, &value)
  }
}
