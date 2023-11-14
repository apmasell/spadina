use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::sql_types::Binary;
use diesel::sqlite::Sqlite;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, FromSqlRow, AsExpression, Serialize, Deserialize)]
#[serde(transparent)]
#[diesel(sql_type = Binary)]
pub struct AsJsonb<T>(pub T);

impl<T> AsRef<T> for AsJsonb<T> {
  fn as_ref(&self) -> &T {
    &self.0
  }
}
impl<T> AsMut<T> for AsJsonb<T> {
  fn as_mut(&mut self) -> &mut T {
    &mut self.0
  }
}

impl<T> Deref for AsJsonb<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl<T> DerefMut for AsJsonb<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<T: Serialize + Debug> ToSql<Binary, Sqlite> for AsJsonb<T> {
  fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
    out.set_value(serde_sqlite_jsonb::to_vec(&self.0)?);
    Ok(IsNull::No)
  }
}

impl<T: DeserializeOwned + Debug> FromSql<Binary, Sqlite> for AsJsonb<T> {
  fn from_sql(bytes: <Sqlite as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
    let slice = <*const [u8] as FromSql<Binary, Sqlite>>::from_sql(bytes)?;
    Ok(serde_sqlite_jsonb::from_slice(unsafe { &*slice })?)
  }
}
impl<T: Default> Default for AsJsonb<T> {
  fn default() -> Self {
    AsJsonb(Default::default())
  }
}
impl<T: PartialEq> PartialEq for AsJsonb<T> {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}
