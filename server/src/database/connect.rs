use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ConnectionError, MysqlConnection, PgConnection, SqliteConnection};
use std::error::Error;

pub enum DatabaseBackend {
  SQLite(Pool<ConnectionManager<SqliteConnection>>),
  #[cfg(feature = "postgres")]
  PostgreSQL(Pool<ConnectionManager<PgConnection>>),
  #[cfg(feature = "mysql")]
  MySql(Pool<ConnectionManager<MysqlConnection>>),
}

impl DatabaseBackend {
  pub fn try_connect(database_url: String) -> Result<DatabaseBackend, Box<dyn Error + Sync + Send + 'static>> {
    if database_url.starts_with("sqlite://") {
      let manager = ConnectionManager::<SqliteConnection>::new(&database_url["sqlite:/".len()..]);
      Ok(DatabaseBackend::SQLite(Pool::builder().build(manager)?))
    } else if database_url.starts_with("postgresql://") {
      #[cfg(feature = "postgres")]
      {
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        Ok(DatabaseBackend::PostgreSQL(Pool::builder().build(manager)?))
      }
      #[cfg(not(feature = "postgres"))]
      Err(ConnectionError::InvalidConnectionUrl("PostgreSQL is not enabled.").into())
    } else if database_url.starts_with("mysql://") {
      #[cfg(feature = "mysql")]
      {
        let manager = ConnectionManager::<MysqlConnection>::new(database_url);
        Ok(DatabaseBackend::MySql(Pool::builder().build(manager)?))
      }
      #[cfg(not(feature = "mysql"))]
      Err(ConnectionError::InvalidConnectionUrl("MySQL is not enabled.").into())
    } else {
      Err(ConnectionError::InvalidConnectionUrl(format!("Unsupported database URL: {}", database_url)).into())
    }
  }
}
