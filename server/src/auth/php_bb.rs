use diesel::prelude::*;
struct PhpBBDb<T>
where
  T: 'static + diesel::r2d2::R2D2Connection,
{
  connection: diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<T>>,
}
/// Create a phpBB-backed store
pub(crate) fn new<T, B>(database_url: String) -> Result<Box<dyn crate::auth::AuthProvider>, String>
where
  T: 'static
    + diesel::Connection<Backend = B, TransactionManager = diesel::connection::AnsiTransactionManager>
    + diesel::r2d2::R2D2Connection
    + diesel::connection::LoadConnection,
  B: 'static
    + for<'a> diesel::backend::Backend<BindCollector<'a> = diesel::query_builder::bind_collector::RawBytesBindCollector<B>>
    + diesel::backend::DieselReserveSpecialization,
  B::QueryBuilder: Default + diesel::query_builder::QueryBuilder<B>,
  bool: diesel::deserialize::FromSql<diesel::sql_types::Bool, B>,
  *const str: diesel::deserialize::FromSql<diesel::sql_types::Text, B>,
{
  let manager = diesel::r2d2::ConnectionManager::<T>::new(database_url);
  Ok(Box::new(PhpBBDb {
    connection: diesel::r2d2::Pool::builder().build(manager).map_err(|e| format!("Failed to create phpBB database connection: {}", e))?,
  }))
}
#[async_trait::async_trait]
impl<T, B> crate::auth::Password for PhpBBDb<T>
where
  T: 'static
    + diesel::Connection<Backend = B, TransactionManager = diesel::connection::AnsiTransactionManager>
    + diesel::r2d2::R2D2Connection
    + diesel::connection::LoadConnection,
  B: 'static
    + for<'a> diesel::backend::Backend<BindCollector<'a> = diesel::query_builder::bind_collector::RawBytesBindCollector<B>>
    + diesel::backend::DieselReserveSpecialization,
  B::QueryBuilder: Default + diesel::query_builder::QueryBuilder<B>,
  bool: diesel::deserialize::FromSql<diesel::sql_types::Bool, B>,
  *const str: diesel::deserialize::FromSql<diesel::sql_types::Text, B>,
{
  async fn check(self: &Self, username: &str, password: &str, _: &crate::database::Database) -> bool {
    match self.connection.get() {
      Ok(mut db_connection) => {
        use diesel::query_builder::QueryBuilder;
        #[derive(diesel::QueryableByName, PartialEq, Debug)]
        struct PhpBBPassword {
          #[diesel(sql_type = diesel::sql_types::Text)]
          pub user_password: String,
        }
        let mut query = B::QueryBuilder::default();
        query.push_sql("SELECT user_password FROM phpbb_users WHERE username_clean = ");
        query.push_bind_param();
        query.push_sql(" AND user_type IN (0, 3)");
        match diesel::sql_query(query.finish()).bind::<diesel::sql_types::Text, _>(username).get_results::<PhpBBPassword>(&mut db_connection) {
          Ok(results) => results.iter().any(|h| phpbb_pwhash::check_hash(&h.user_password, password) == phpbb_pwhash::CheckHashResult::Valid),
          Err(diesel::result::Error::NotFound) => false,
          Err(e) => {
            eprintln!("Failed to check phpBB password for {}: {}", username, e);
            false
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to check phpBB password for {}: {}", username, e);
        false
      }
    }
  }

  async fn is_locked(&self, username: &str, _: &crate::database::Database) -> spadina_core::access::AccountLockState {
    match self.connection.get() {
      Ok(mut db_connection) => {
        use diesel::query_builder::QueryBuilder;
        #[derive(diesel::QueryableByName, PartialEq, Debug)]
        struct PhpBBIsLocked {
          #[diesel(sql_type = diesel::sql_types::Bool)]
          pub locked: bool,
        }
        let mut query = B::QueryBuilder::default();
        query.push_sql("SELECT user_type NOT IN (0, 3) AS locked FROM phpbb_users WHERE username_clean = ");
        query.push_bind_param();
        match diesel::sql_query(query.finish()).bind::<diesel::sql_types::Text, _>(username).get_results::<PhpBBIsLocked>(&mut db_connection) {
          Ok(results) => {
            if results.iter().any(|l| l.locked) {
              spadina_core::access::AccountLockState::PermanentlyLocked
            } else {
              spadina_core::access::AccountLockState::PermanentlyUnlocked
            }
          }
          Err(diesel::result::Error::NotFound) => spadina_core::access::AccountLockState::Unknown,
          Err(e) => {
            eprintln!("Failed to check phpBB password for {}: {}", username, e);
            spadina_core::access::AccountLockState::Unknown
          }
        }
      }
      Err(e) => {
        eprintln!("Failed to get connection to check phpBB password for {}: {}", username, e);
        spadina_core::access::AccountLockState::Unknown
      }
    }
  }

  async fn lock(&self, _: &str, _: bool, _: &crate::database::Database) -> spadina_core::UpdateResult {
    spadina_core::UpdateResult::NotAllowed
  }
}
