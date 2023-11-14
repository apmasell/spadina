use crate::database::schema::player::dsl as player_schema;
use diesel::expression::SelectableExpression;
use diesel::sql_types::Bool;
use diesel::sqlite::Sqlite;
use diesel::{BoxableExpression, ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl, SqliteConnection};

pub enum PlayerReference<S: AsRef<str>> {
  Id(i32),
  Name(S),
}

impl<S: AsRef<str>> PlayerReference<S> {
  pub fn as_expression<'a, T>(&'a self) -> Box<dyn BoxableExpression<T, Sqlite, SqlType = Bool> + 'a>
  where
    player_schema::id: SelectableExpression<T>,
    player_schema::name: SelectableExpression<T>,
  {
    use diesel::prelude::*;
    match self {
      PlayerReference::Id(id) => Box::new(player_schema::id.eq(*id)),
      PlayerReference::Name(name) => Box::new(player_schema::name.eq(name.as_ref())),
    }
  }
  pub fn get_id(self, db_connection: &mut diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<SqliteConnection>>) -> QueryResult<i32> {
    match self {
      PlayerReference::Id(id) => Ok(id),
      PlayerReference::Name(name) => {
        player_schema::player.select(player_schema::id).filter(player_schema::name.eq(name.as_ref())).first::<i32>(db_connection)
      }
    }
  }
}
