use crate::database::schema;
use diesel::expression::SelectableExpression;
use diesel::pg::Pg;
use diesel::sql_types::Bool;
use diesel::{BoxableExpression, ExpressionMethods, PgConnection, QueryDsl, QueryResult, RunQueryDsl};

pub enum PlayerReference<S: AsRef<str>> {
  Id(i32),
  Name(S),
}

impl<S: AsRef<str>> PlayerReference<S> {
  pub fn as_expression<'a, T>(&'a self) -> Box<dyn BoxableExpression<T, Pg, SqlType = Bool> + 'a>
  where
    schema::player::dsl::id: SelectableExpression<T>,
    schema::player::dsl::name: SelectableExpression<T>,
  {
    use crate::database::schema::player::dsl as player_schema;
    use diesel::prelude::*;
    match self {
      PlayerReference::Id(id) => Box::new(player_schema::id.eq(*id)),
      PlayerReference::Name(name) => Box::new(player_schema::name.eq(name.as_ref())),
    }
  }
  pub fn get_id(self, db_connection: &mut diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>) -> QueryResult<i32> {
    match self {
      PlayerReference::Id(id) => Ok(id),
      PlayerReference::Name(name) => {
        schema::player::table.select(schema::player::id).filter(schema::player::name.eq(name.as_ref())).first::<i32>(db_connection)
      }
    }
  }
}
