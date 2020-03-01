#[derive(Debug, Clone, Copy)]
pub(crate) struct Avatar(pub i32);
impl crate::database::persisted::Persistance for Avatar {
  type Value = spadina_core::avatar::Avatar;

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.player_avatar_read(self.0)
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.player_avatar_write(self.0, value)
  }
}
