use crate::database::schema;
use diesel::{Column, SelectableExpression};
use spadina_core::access;

pub trait PlayerAccess:
  Column<Table = schema::player::table, SqlType = diesel::sql_types::Binary>
  + SelectableExpression<schema::player::table>
  + diesel::expression::ValidGrouping<()>
  + diesel::query_builder::QueryId
  + diesel::query_builder::QueryFragment<diesel::sqlite::Sqlite>
{
  type Verb: serde::de::DeserializeOwned + serde::Serialize + Copy + Default + std::fmt::Debug + 'static;
}

impl PlayerAccess for schema::player::dsl::message_acl {
  type Verb = access::SimpleAccess;
}

impl PlayerAccess for schema::player::dsl::default_location_acl {
  type Verb = access::Privilege;
}

impl PlayerAccess for schema::player::dsl::online_acl {
  type Verb = access::OnlineAccess;
}
