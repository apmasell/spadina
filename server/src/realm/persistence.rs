#[derive(Copy, Clone)]
pub(crate) struct RealmAccess(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct RealmAdmin(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct RealmAnnouncements(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct NameAndInDirectory(pub i32);
#[derive(Copy, Clone)]
pub(crate) struct Settings(pub i32);

impl crate::database::persisted::Persistance for RealmAccess {
  type Value = crate::access::AccessSetting<spadina_core::access::SimpleAccess>;

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.realm_acl_read(self.0, crate::database::schema::realm::dsl::access_acl)
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.realm_acl_write(self.0, crate::database::schema::realm::dsl::access_acl, value)
  }
}
impl crate::database::persisted::Persistance for RealmAdmin {
  type Value = crate::access::AccessSetting<spadina_core::access::SimpleAccess>;
  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.realm_acl_read(self.0, crate::database::schema::realm::dsl::access_acl)
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.realm_acl_write(self.0, crate::database::schema::realm::dsl::access_acl, value)
  }
}
impl crate::database::persisted::Persistance for RealmAnnouncements {
  type Value = Vec<spadina_core::realm::RealmAnnouncement<crate::shstr::ShStr>>;

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    Ok(database.realm_announcements_read(self.0)?.into_iter().map(|a| a.convert_str()).collect())
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.realm_announcements_write(self.0, value)
  }
}
impl crate::database::persisted::Persistance for NameAndInDirectory {
  type Value = (std::sync::Arc<str>, bool);

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.realm_name_read(self.0).map(|(name, in_directory)| (std::sync::Arc::from(name), in_directory))
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.realm_name_write(self.0, &value.0, value.1)
  }
}
impl crate::database::persisted::Persistance for Settings {
  type Value = crate::realm::RealmSettings;

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    Ok(database.realm_settings_read(self.0)?.into_iter().map(|(k, v)| (k.upgrade(), v)).collect())
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.realm_settings_write(self.0, value)
  }
}
