use super::schema::player::dsl as player_schema;
use crate::database::persisted::Persistence;
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use diesel::QueryResult;
use spadina_core::access::{AccessSetting, OnlineAccess, Privilege, SimpleAccess};
use spadina_core::avatar::Avatar;

#[derive(Copy, Clone)]
pub struct PlayerAvatar(pub i32);
#[derive(Copy, Clone)]
pub struct PlayerDefaultLocationAccess(pub i32);
#[derive(Copy, Clone)]
pub struct PlayerOnlineAccess(pub i32);
#[derive(Copy, Clone)]
pub struct PlayerMessageAccess(pub i32);

impl Persistence for PlayerAvatar {
  type Value = Avatar;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.player_avatar_read(self.0)
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.player_avatar_write(self.0, value)
  }
}
impl Persistence for PlayerDefaultLocationAccess {
  type Value = AccessSetting<String, Privilege>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.player_acl(PlayerReference::Id(self.0), player_schema::default_location_acl).map(|acl| acl.unwrap_or_default())
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.player_acl_write(self.0, player_schema::default_location_acl, value)
  }
}

impl Persistence for PlayerOnlineAccess {
  type Value = AccessSetting<String, OnlineAccess>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.player_acl(PlayerReference::Id(self.0), player_schema::online_acl).map(|acl| acl.unwrap_or_default())
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.player_acl_write(self.0, player_schema::online_acl, value)
  }
}
impl Persistence for PlayerMessageAccess {
  type Value = AccessSetting<String, SimpleAccess>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.player_acl(PlayerReference::Id(self.0), player_schema::message_acl).map(|acl| acl.unwrap_or_default())
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.player_acl_write(self.0, player_schema::message_acl, value)
  }
}
