use diesel::prelude::*;
use rand::Rng;

pub struct Database(DbPool);
type DbConnection = diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>;
type DbPool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>;

sql_function! { #[sql_name = ""]fn sql_not_null_bytes(x: diesel::sql_types::Nullable<diesel::sql_types::Binary>) -> diesel::sql_types::Binary}
sql_function! { #[sql_name = ""]fn sql_not_null_int(x: diesel::sql_types::Nullable<diesel::sql_types::Integer>) -> diesel::sql_types::Integer}
sql_function! { #[sql_name = ""]fn sql_not_null_str(x: diesel::sql_types::Nullable<diesel::sql_types::VarChar>) -> diesel::sql_types::VarChar}
sql_function! { #[sql_name = "COALESCE"]fn sql_coalesce_bool(x: diesel::sql_types::Nullable<diesel::sql_types::Bool>, y: diesel::sql_types::Bool) -> diesel::sql_types::Bool}
pub(crate) struct PlayerInfo {
  pub(crate) db_id: i32,
  pub(crate) debuted: bool,
  pub(crate) avatar: crate::prometheus_locks::labelled_rwlock::PrometheusLabelledRwLock<'static, puzzleverse_core::avatar::Avatar>,
  pub(crate) message_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) online_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) location_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) access_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) admin_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
}
pub(crate) enum RealmScope<'a> {
  ByPrincipal(&'a str),
  Train { owner: i32, train: i32 },
}
pub(crate) enum RealmListScope<'a> {
  ByAsset { asset: &'a str, owner: i32 },
  ByPrincipal { ids: &'a [&'a str] },
  InDirectory,
  Owner(i32),
  Train { owner: i32, train: i32 },
}

impl Database {
  pub fn new(pool: DbPool, default_realm: &str) -> Self {
    use crate::schema::realmtrain::dsl as realmtrain_schema;
    use diesel_migrations::MigrationHarness;
    let mut db_connection = pool.get().expect("Failed to connect to database");
    db_connection.run_pending_migrations(super::MIGRATIONS).expect("Failed to migrate database to latest schema");
    diesel::insert_into(realmtrain_schema::realmtrain)
      .values(&(realmtrain_schema::asset.eq(default_realm), realmtrain_schema::allowed_first.eq(true)))
      .on_conflict(realmtrain_schema::asset)
      .do_update()
      .set(realmtrain_schema::allowed_first.eq(true))
      .execute(&mut db_connection)
      .expect("Failed to make sure default realm is present");
    Database(pool)
  }
  pub fn acl_write(
    &self,
    player_name: &str,
    target: &puzzleverse_core::AccessTarget,
    default: &puzzleverse_core::AccessDefault,
    acls: &[puzzleverse_core::AccessControl],
  ) -> diesel::result::QueryResult<usize> {
    let encoded = rmp_serde::to_vec(&(default, acls)).unwrap();
    use crate::schema::player::dsl as player_schema;
    use crate::schema::serveracl::dsl as serveracl_schema;
    let mut db_connection = self.0.get().unwrap();
    fn update_server_acl(db_connection: &mut DbConnection, category: &str, encoded: &[u8]) -> diesel::QueryResult<usize> {
      diesel::insert_into(serveracl_schema::serveracl)
        .values(&(serveracl_schema::category.eq(category), serveracl_schema::acl.eq(&encoded)))
        .on_conflict(serveracl_schema::category)
        .do_update()
        .set(serveracl_schema::acl.eq(&encoded))
        .execute(db_connection)
    }
    fn update_player_acl<T: diesel::query_source::Column<Table = player_schema::player, SqlType = diesel::sql_types::Binary>>(
      db_connection: &mut DbConnection,
      player_name: &str,
      encoded: &[u8],
      column: T,
    ) -> diesel::QueryResult<usize> {
      diesel::update(player_schema::player.filter(player_schema::name.eq(player_name))).set(column.eq(encoded)).execute(db_connection)
    }
    match &target {
      puzzleverse_core::AccessTarget::AccessServer => update_server_acl(&mut db_connection, "a", &encoded),
      puzzleverse_core::AccessTarget::AdminServer => update_server_acl(&mut db_connection, "A", &encoded),
      puzzleverse_core::AccessTarget::DirectMessagesUser => update_player_acl(&mut db_connection, &player_name, &encoded, player_schema::message_acl),
      puzzleverse_core::AccessTarget::DirectMessagesServer => update_server_acl(&mut db_connection, "m", &encoded),
      puzzleverse_core::AccessTarget::CheckOnline => update_player_acl(&mut db_connection, &player_name, &encoded, player_schema::online_acl),
      puzzleverse_core::AccessTarget::NewRealmDefaultAccess => {
        update_player_acl(&mut db_connection, &player_name, &encoded, player_schema::new_realm_access_acl)
      }
      puzzleverse_core::AccessTarget::NewRealmDefaultAdmin => {
        update_player_acl(&mut db_connection, &player_name, &encoded, player_schema::new_realm_admin_acl)
      }
      puzzleverse_core::AccessTarget::ViewLocation => update_player_acl(&mut db_connection, &player_name, &encoded, player_schema::location_acl),
    }
  }
  pub fn announcements(&self, announcements: &[puzzleverse_core::Announcement]) {
    use crate::schema::announcement::dsl as announcement_schema;
    use diesel::prelude::*;
    if let Err(e) = self.0.get().unwrap().transaction::<(), diesel::result::Error, _>(|db_connection| {
      diesel::delete(announcement_schema::announcement).execute(db_connection)?;
      let rows: Vec<_> = announcements
        .iter()
        .map(|a| {
          let event = rmp_serde::to_vec(&a.event).unwrap();
          let realm = rmp_serde::to_vec(&a.realm).unwrap();
          (
            announcement_schema::contents.eq(&a.text),
            announcement_schema::expires.eq(&a.expires),
            announcement_schema::event.eq(event),
            announcement_schema::realm.eq(realm),
          )
        })
        .collect();
      diesel::insert_into(announcement_schema::announcement).values(&rows).execute(db_connection)?;

      Ok(())
    }) {
      eprintln!("failed to update announcements: {}", e);
    }
  }
  pub fn banned_peers_add(&self, peers: &[String]) -> QueryResult<usize> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::bannedpeers::dsl as bannedpeers_schema;
    let rows: Vec<_> = peers.iter().map(|p| bannedpeers_schema::server.eq(p)).collect();
    diesel::insert_into(bannedpeers_schema::bannedpeers).values(&rows).on_conflict_do_nothing().execute(&mut db_connection)
  }
  pub fn banned_peers_remove(&self, peers: &[String]) -> QueryResult<usize> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::bannedpeers::dsl as bannedpeers_schema;
    diesel::delete(bannedpeers_schema::bannedpeers.filter(bannedpeers_schema::server.eq_any(peers))).execute(&mut db_connection)
  }
  pub fn bookmark_add(&self, db_id: i32, bookmark_type: &puzzleverse_core::BookmarkType, asset: &str) -> QueryResult<usize> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::bookmark::dsl as bookmark_schema;
    diesel::insert_into(bookmark_schema::bookmark)
      .values(&(bookmark_schema::player.eq(db_id), bookmark_schema::asset.eq(asset), bookmark_schema::kind.eq(crate::id_for_type(bookmark_type))))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn bookmark_get(&self, db_id: i32, bookmark_type: &puzzleverse_core::BookmarkType) -> QueryResult<Vec<String>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::bookmark::dsl as bookmark_schema;
    bookmark_schema::bookmark
      .select(bookmark_schema::asset)
      .filter(bookmark_schema::player.eq(db_id).and(bookmark_schema::kind.eq(crate::id_for_type(&bookmark_type))))
      .load::<String>(&mut db_connection)
  }
  pub fn bookmark_rm(&self, db_id: i32, bookmark_type: &puzzleverse_core::BookmarkType, asset: &str) -> QueryResult<usize> {
    use crate::schema::bookmark::dsl as bookmark_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(bookmark_schema::bookmark.filter(
      bookmark_schema::player.eq(db_id).and(bookmark_schema::asset.eq(&asset)).and(bookmark_schema::kind.eq(crate::id_for_type(&bookmark_type))),
    ))
    .execute(&mut db_connection)
  }
  pub fn direct_message_clean(&self) -> QueryResult<()> {
    use crate::schema::localplayerchat::dsl as local_player_chat_schema;
    use crate::schema::realmchat::dsl as realm_chat_schema;
    use crate::schema::remoteplayerchat::dsl as remote_player_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    let horizon = chrono::Utc::now() - chrono::Duration::days(30);
    diesel::delete(realm_chat_schema::realmchat.filter(realm_chat_schema::created.le(&horizon))).execute(&mut db_connection)?;
    diesel::delete(local_player_chat_schema::localplayerchat.filter(local_player_chat_schema::created.le(&horizon))).execute(&mut db_connection)?;
    diesel::delete(remote_player_chat_schema::remoteplayerchat.filter(remote_player_chat_schema::created.le(&horizon)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn direct_message_get(
    &self,
    db_id: i32,
    name: &str,
    from: &chrono::DateTime<chrono::Utc>,
    to: &chrono::DateTime<chrono::Utc>,
  ) -> QueryResult<Vec<(String, chrono::DateTime<chrono::Utc>, bool)>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::localplayerchat::dsl as chat_schema;
    use crate::schema::player::dsl as player_schema;
    chat_schema::localplayerchat
      .select((chat_schema::body, chat_schema::created, chat_schema::recipient.eq(db_id)))
      .filter(
        chat_schema::created.ge(&from).and(chat_schema::created.lt(&to)).and(
          chat_schema::recipient
            .eq(db_id)
            .and(
              chat_schema::sender.nullable().eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&name)).single_value()),
            )
            .or(
              chat_schema::sender.eq(db_id).and(
                chat_schema::recipient
                  .nullable()
                  .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&name)).single_value()),
              ),
            ),
        ),
      )
      .load::<(String, chrono::DateTime<chrono::Utc>, bool)>(&mut db_connection)
  }
  pub fn direct_message_write(&self, sender: &str, recipient: &str, body: &str) -> QueryResult<chrono::DateTime<chrono::Utc>> {
    use crate::schema::localplayerchat::dsl as chat_schema;
    use crate::schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let recipient_id = player_schema::player.select(player_schema::id).filter(player_schema::name.eq(recipient)).first::<i32>(&mut db_connection)?;
    let timestamp = chrono::Utc::now();
    diesel::insert_into(chat_schema::localplayerchat)
      .values(&(
        chat_schema::recipient.eq(recipient_id),
        chat_schema::body.eq(body),
        chat_schema::created.eq(&timestamp),
        chat_schema::sender
          .eq(sql_not_null_int(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value())),
      ))
      .execute(&mut db_connection)?;
    Ok(timestamp)
  }
  pub fn direct_message_stats(
    &self,
    db_id: i32,
  ) -> diesel::QueryResult<(Vec<(String, chrono::DateTime<chrono::Utc>)>, chrono::DateTime<chrono::Utc>)> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::player::dsl as player_schema;
    use crate::views::lastmessage::dsl as lastmessage_schema;
    let stats = lastmessage_schema::lastmessage
      .select((lastmessage_schema::principal, lastmessage_schema::last_time))
      .filter(lastmessage_schema::id.eq(db_id))
      .load::<(String, chrono::DateTime<chrono::Utc>)>(&mut db_connection)?;
    let last_login = player_schema::player
      .select(player_schema::last_login)
      .filter(player_schema::id.eq(db_id))
      .first::<chrono::DateTime<chrono::Utc>>(&mut db_connection)?;
    Ok((stats, last_login))
  }
  pub fn player_debut(&self, player: &str) -> QueryResult<usize> {
    use crate::schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::name.eq(player))).set(player_schema::debuted.eq(true)).execute(&mut db_connection)
  }
  pub fn player_delete(&self, db_id: i32) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<(), diesel::result::Error, _>(|db_connection| {
      use crate::schema::bookmark::dsl as bookmark_schema;
      use crate::schema::localplayerchat::dsl as localplayerchat_schema;
      use crate::schema::player::dsl as player_schema;
      use crate::schema::publickey::dsl as publickey_schema;
      use crate::schema::realm::dsl as realm_schema;
      use crate::schema::realmchat::dsl as realmchat_schema;
      use crate::schema::remoteplayerchat::dsl as remoteplayerchat_schema;
      diesel::delete(
        localplayerchat_schema::localplayerchat.filter(localplayerchat_schema::sender.eq(db_id).or(localplayerchat_schema::recipient.eq(db_id))),
      )
      .execute(db_connection)?;
      diesel::delete(remoteplayerchat_schema::remoteplayerchat.filter(remoteplayerchat_schema::player.eq(db_id))).execute(db_connection)?;
      diesel::delete(
        realmchat_schema::realmchat
          .filter(realmchat_schema::realm.eq_any(realm_schema::realm.select(realm_schema::id).filter(realm_schema::owner.eq(db_id)))),
      )
      .execute(db_connection)?;
      diesel::delete(realm_schema::realm.filter(realm_schema::owner.eq(db_id))).execute(db_connection)?;
      diesel::delete(bookmark_schema::bookmark.filter(bookmark_schema::player.eq(db_id))).execute(db_connection)?;
      diesel::delete(publickey_schema::publickey.filter(publickey_schema::player.eq(db_id))).execute(db_connection)?;
      diesel::delete(player_schema::player.filter(player_schema::id.eq(db_id))).execute(db_connection)?;

      Ok(())
    })
  }
  pub(crate) fn player_load(&self, player_name: &str, create: bool) -> QueryResult<Option<PlayerInfo>> {
    use crate::schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      // Find or create the player's database entry
      let results: Option<(i32, bool, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = player_schema::player
        .select((
          player_schema::id,
          player_schema::debuted,
          player_schema::avatar,
          player_schema::message_acl,
          player_schema::online_acl,
          player_schema::location_acl,
          player_schema::new_realm_access_acl,
          player_schema::new_realm_admin_acl,
        ))
        .filter(player_schema::name.eq(player_name))
        .get_result(db_connection)
        .optional()?;
      let db_record = match results {
        None => {
          if create {
            diesel::insert_into(player_schema::player)
              .values((
                player_schema::name.eq(player_name),
                player_schema::debuted.eq(false),
                player_schema::avatar.eq(vec![]),
                player_schema::message_acl.eq(vec![]),
                player_schema::online_acl.eq(vec![]),
                player_schema::location_acl.eq(vec![]),
                player_schema::new_realm_access_acl.eq(vec![]),
                player_schema::new_realm_admin_acl.eq(vec![]),
              ))
              .returning((
                player_schema::id,
                player_schema::debuted,
                player_schema::avatar,
                player_schema::message_acl,
                player_schema::online_acl,
                player_schema::location_acl,
                player_schema::new_realm_access_acl,
                player_schema::new_realm_admin_acl,
              ))
              .get_result::<(i32, bool, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)>(db_connection)
              .optional()?
          } else {
            None
          }
        }
        Some(record) => Some(record),
      };

      Ok(match db_record {
        Some((id, debuted, avatar, message_acl, online_acl, location_acl, access_acl, admin_acl)) => {
          diesel::update(player_schema::player.filter(player_schema::id.eq(id)))
            .set(player_schema::last_login.eq(chrono::Utc::now()))
            .execute(db_connection)?;

          Some(PlayerInfo {
            db_id: id,
            debuted,
            avatar: crate::PLAYER_AVATAR_LOCK.create(
              player_name,
              match rmp_serde::from_slice(&avatar) {
                Ok(avatar) => avatar,
                Err(e) => {
                  eprintln!("Avatar for {} is corrupt: {}", player_name, e);
                  Default::default()
                }
              },
            ),
            message_acl: parse_player_acl(
              message_acl,
              (puzzleverse_core::AccessDefault::Deny, vec![puzzleverse_core::AccessControl::AllowLocal(None)]),
            ),
            online_acl: parse_player_acl(online_acl, (puzzleverse_core::AccessDefault::Deny, vec![])),
            location_acl: parse_player_acl(location_acl, (puzzleverse_core::AccessDefault::Deny, vec![])),
            access_acl: parse_player_acl(access_acl, (puzzleverse_core::AccessDefault::Deny, vec![])),
            admin_acl: parse_player_acl(admin_acl, (puzzleverse_core::AccessDefault::Deny, vec![])),
          })
        }
        None => None,
      })
    })
  }
  pub fn public_key_add(&self, db_id: i32, name: &str, der: &[u8]) -> QueryResult<usize> {
    use crate::schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(publickey_dsl::publickey)
      .values((publickey_dsl::player.eq(db_id), publickey_dsl::name.eq(name), publickey_dsl::public_key.eq(der)))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn public_key_get(&self, player: &str) -> QueryResult<Vec<Vec<u8>>> {
    use crate::schema::player::dsl as player_dsl;
    use crate::schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    publickey_dsl::publickey
      .select(publickey_dsl::public_key)
      .filter(
        publickey_dsl::player.eq(sql_not_null_int(player_dsl::player.select(player_dsl::id).filter(player_dsl::name.eq(player)).single_value())),
      )
      .load::<Vec<u8>>(&mut db_connection)
  }
  pub fn public_key_list(&self, db_id: i32) -> QueryResult<Vec<String>> {
    use crate::schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    publickey_dsl::publickey.select(publickey_dsl::name).filter(publickey_dsl::player.eq(&db_id)).load::<String>(&mut db_connection)
  }
  pub fn public_key_rm(&self, db_id: i32, name: &str) -> QueryResult<usize> {
    use crate::schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(publickey_dsl::publickey.filter(publickey_dsl::name.eq(name).and(publickey_dsl::player.eq(db_id)))).execute(&mut db_connection)
  }
  pub fn public_key_rm_all(&self, db_id: i32) -> QueryResult<usize> {
    use crate::schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(publickey_dsl::publickey.filter(publickey_dsl::player.eq(&db_id))).execute(&mut db_connection)
  }
  pub fn realm_acl(
    &self,
    id: i32,
    target: puzzleverse_core::RealmAccessTarget,
    default: puzzleverse_core::AccessDefault,
    acls: Vec<puzzleverse_core::AccessControl>,
  ) -> QueryResult<usize> {
    use crate::schema::realm::dsl as realm_schema;
    fn update<C: diesel::Column<Table = realm_schema::realm, SqlType = diesel::sql_types::Binary>>(
      db_connection: &mut DbConnection,
      id: i32,
      output: &[u8],
      column: C,
    ) -> QueryResult<usize> {
      diesel::update(realm_schema::realm.filter(realm_schema::id.eq(id))).set(column.eq(output)).execute(db_connection)
    }
    let mut db_connection = self.0.get().unwrap();
    let output = rmp_serde::to_vec::<crate::AccessControlSetting>(&(default, acls)).unwrap();
    match target {
      puzzleverse_core::RealmAccessTarget::Access => update(&mut db_connection, id, &output, realm_schema::access_acl),
      puzzleverse_core::RealmAccessTarget::Admin => update(&mut db_connection, id, &output, realm_schema::admin_acl),
    }
  }
  pub fn realm_chat_write(&self, db_id: i32, sender: &str, body: &str, timestamp: &chrono::DateTime<chrono::Utc>) -> QueryResult<()> {
    use crate::schema::realmchat::dsl as realm_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(realm_chat_schema::realmchat)
      .values((
        realm_chat_schema::body.eq(&body),
        realm_chat_schema::principal.eq(&sender),
        realm_chat_schema::created.eq(&timestamp),
        realm_chat_schema::realm.eq(db_id),
      ))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_count(&self, name: &str, db_id: i32) -> QueryResult<i64> {
    use crate::schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .select(diesel::dsl::count_star())
      .filter(realm_schema::name.eq(&name).and(realm_schema::owner.eq(db_id)))
      .first(&mut db_connection)
  }
  pub fn realm_create(&self, asset: &str, owner: &str, name: Option<String>, seed: Option<i32>, train: Option<u16>) -> diesel::QueryResult<String> {
    let mut db_connection = self.0.get().unwrap();
    create_realm(&mut db_connection, asset, owner, name, seed, train)
  }
  pub fn realm_delete(&self, target: &str, db_id: i32) -> QueryResult<usize> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use crate::schema::realm::dsl as realm_schema;
      use crate::schema::realmchat::dsl as chat_schema;
      diesel::delete(chat_schema::realmchat.filter(chat_schema::realm.nullable().eq(
        realm_schema::realm.select(realm_schema::id).filter(realm_schema::principal.eq(target).and(realm_schema::owner.eq(db_id))).single_value(),
      )))
      .execute(db_connection)?;
      diesel::delete(realm_schema::realm.filter(realm_schema::principal.eq(target).and(realm_schema::owner.eq(db_id)))).execute(db_connection)
    })
  }
  pub(crate) fn realm_find(&self, scope: RealmScope) -> QueryResult<Option<(String, String)>> {
    use crate::schema::realm::dsl as realm_schema;
    fn query<
      P: diesel::Expression<SqlType = diesel::sql_types::Bool>
        + diesel::expression::NonAggregate
        + diesel::expression::AppearsOnTable<crate::schema::realm::table>
        + diesel::query_builder::QueryFragment<diesel::pg::Pg>
        + diesel::query_builder::QueryId,
    >(
      db_connection: &mut DbConnection,
      predicate: P,
    ) -> QueryResult<Option<(String, String)>> {
      realm_schema::realm
        .select((realm_schema::principal, realm_schema::asset))
        .filter(predicate)
        .order_by(realm_schema::updated_at.desc())
        .get_result::<(String, String)>(db_connection)
        .optional()
    }

    let mut db_connection = self.0.get().unwrap();
    match scope {
      RealmScope::ByPrincipal(id) => query(&mut db_connection, realm_schema::principal.eq(id)),
      RealmScope::Train { owner, train } => {
        query(&mut db_connection, realm_schema::owner.eq(owner).and(sql_coalesce_bool(realm_schema::train.eq(train), false)))
      }
    }
  }
  pub(crate) fn realm_list(&self, server_name: &str, predicate: RealmListScope) -> Vec<puzzleverse_core::Realm> {
    use crate::schema::realm::dsl as realm_schema;
    fn query<
      P: diesel::Expression<SqlType = diesel::sql_types::Bool>
        + diesel::expression::NonAggregate
        + diesel::expression::AppearsOnTable<crate::schema::realm::table>
        + diesel::query_builder::QueryFragment<diesel::pg::Pg>
        + diesel::query_builder::QueryId,
    >(
      db_connection: &mut DbConnection,
      predicate: P,
    ) -> QueryResult<Vec<(String, String, chrono::DateTime<chrono::Utc>, Option<i32>)>> {
      realm_schema::realm
        .select((realm_schema::principal, realm_schema::name, realm_schema::updated_at, realm_schema::train))
        .filter(predicate)
        .load::<(String, String, chrono::DateTime<chrono::Utc>, Option<i32>)>(db_connection)
    }
    let mut db_connection = self.0.get().unwrap();
    let (include_train, result) = match predicate {
      RealmListScope::ByAsset { asset, owner } => {
        (false, query(&mut db_connection, realm_schema::owner.eq(owner).and(realm_schema::asset.eq(&asset))))
      }
      RealmListScope::ByPrincipal { ids } => (false, query(&mut db_connection, realm_schema::principal.eq_any(ids))),
      RealmListScope::InDirectory => (false, query(&mut db_connection, realm_schema::in_directory)),
      RealmListScope::Owner(owner) => (true, query(&mut db_connection, realm_schema::owner.eq(owner))),
      RealmListScope::Train { owner, train } => {
        (true, query(&mut db_connection, realm_schema::owner.eq(owner).and(sql_coalesce_bool(realm_schema::train.eq(train), false))))
      }
    };
    match result {
      Ok(entries) => entries
        .into_iter()
        .map(|(id, name, accessed, train)| puzzleverse_core::Realm {
          id,
          name,
          accessed: Some(accessed), // TODO: Are there any reasons to restrict this information?
          activity: puzzleverse_core::RealmActivity::Unknown,
          server: Some(server_name.to_string()),
          train: if include_train { train.map(|t| t as u16) } else { None },
        })
        .collect(),
      Err(diesel::result::Error::NotFound) => vec![],
      Err(e) => {
        eprintln!("Failed to get realms from DB: {}", e);
        vec![]
      }
    }
  }
  pub fn realm_load(
    &self,
    principal: &str,
  ) -> QueryResult<(i32, bool, Vec<u8>, Vec<u8>, Vec<u8>, i32, Vec<u8>, String, String, bool, Option<String>, Option<i32>)> {
    use crate::schema::player::dsl as player_schema;
    use crate::schema::realm::dsl as realm_schema;

    let mut db_connection = self.0.get().unwrap();

    realm_schema::realm
      .select((
        realm_schema::id,
        realm_schema::initialised,
        realm_schema::admin_acl,
        realm_schema::access_acl,
        realm_schema::state,
        realm_schema::seed,
        realm_schema::settings,
        realm_schema::asset,
        realm_schema::name,
        realm_schema::in_directory,
        player_schema::player.select(player_schema::name).filter(player_schema::id.eq(realm_schema::owner)).single_value(),
        realm_schema::train,
      ))
      .filter(realm_schema::principal.eq(principal))
      .first(&mut db_connection)
  }
  pub fn realm_messages(
    &self,
    db_id: i32,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) -> QueryResult<Vec<(String, chrono::DateTime<chrono::Utc>, String)>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::realmchat::dsl as realmchat_schema;
    realmchat_schema::realmchat
      .select((realmchat_schema::principal, realmchat_schema::created, realmchat_schema::body))
      .filter(realmchat_schema::realm.eq(db_id).and(realmchat_schema::created.ge(from)).and(realmchat_schema::created.lt(to)))
      .load::<(String, chrono::DateTime<chrono::Utc>, String)>(&mut db_connection)
  }
  pub fn realm_push_settings(&self, db_id: i32, settings: &crate::realm::RealmSettings) -> QueryResult<()> {
    use crate::schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    let settings = rmp_serde::to_vec(settings).unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id))).set(realm_schema::settings.eq(&settings)).execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_push_state(&self, db_id: i32, state: Vec<u8>) -> QueryResult<()> {
    use crate::schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id)))
      .set((realm_schema::state.eq(state), realm_schema::initialised.eq(true)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_rename(&self, db_id: i32, name: &str, in_directory: bool) -> QueryResult<()> {
    use crate::schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id)))
      .set((realm_schema::name.eq(name), realm_schema::in_directory.eq(in_directory)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_upsert_by_asset(&self, owner: &str, asset: &str) -> QueryResult<String> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use crate::schema::player::dsl as player_schema;
      use crate::schema::realm::dsl as realm_schema;
      let result = realm_schema::realm
        .select(realm_schema::principal)
        .filter(realm_schema::asset.eq(&asset).and(
          realm_schema::owner.nullable().eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&owner)).single_value()),
        ))
        .get_result::<String>(db_connection)
        .optional()?;

      match result {
        Some(id) => Ok(id),
        None => Ok(create_realm(db_connection, &asset, &owner, None, None, None)?),
      }
    })
  }
  pub fn realm_upsert_by_train(&self, owner: &str, train: u16) -> QueryResult<Option<String>> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use crate::schema::player::dsl as player_schema;
      use crate::schema::realm::dsl as realm_schema;
      let result = realm_schema::realm
        .select(realm_schema::principal)
        .filter(realm_schema::train.eq(train as i32).and(
          realm_schema::owner.nullable().eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&owner)).single_value()),
        ))
        .get_result::<String>(db_connection)
        .optional()?;

      match result {
        Some(id) => Ok(Some(id)),
        None => {
          use crate::schema::realmtrain::dsl as realmtrain_schema;
          use rand::seq::SliceRandom;
          let mut train_query = realmtrain_schema::realmtrain
            .select(realmtrain_schema::asset)
            .filter(
              realmtrain_schema::asset.ne_all(
                realm_schema::realm.select(realm_schema::asset).filter(
                  realm_schema::owner
                    .nullable()
                    .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&owner)).single_value()),
                ),
              ),
            )
            .into_boxed();
          if train == 0 {
            train_query = train_query.filter(realmtrain_schema::allowed_first)
          };
          let mut assets = train_query.load::<String>(db_connection)?;
          assets.shuffle(&mut rand::thread_rng());
          match assets.get(0) {
            Some(asset) => Ok(Some(create_realm(db_connection, &asset, &owner, None, None, Some(train))?)),
            None => {
              diesel::update(player_schema::player.filter(player_schema::name.eq(&owner)))
                .set(player_schema::waiting_for_train.eq(true))
                .execute(db_connection)?;
              Ok(None)
            }
          }
        }
      }
    })
  }
  pub fn remote_direct_message_get(
    &self,
    db_id: i32,
    player: &str,
    remote_server: &str,
  ) -> QueryResult<Vec<(String, chrono::DateTime<chrono::Utc>, bool)>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::remoteplayerchat::dsl as chat_schema;
    chat_schema::remoteplayerchat
      .select((chat_schema::body, chat_schema::created, chat_schema::state.eq("r")))
      .filter(chat_schema::player.eq(db_id).and(chat_schema::remote_player.eq(&player)).and(chat_schema::remote_server.eq(&remote_server)))
      .load::<(String, chrono::DateTime<chrono::Utc>, bool)>(&mut db_connection)
  }
  pub fn remote_direct_messages_peers(&self) -> QueryResult<Vec<String>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::remoteplayerchat::dsl as chat_schema;
    chat_schema::remoteplayerchat.select(chat_schema::remote_server).distinct().filter(chat_schema::state.eq("O")).load::<String>(&mut db_connection)
  }
  pub fn remote_direct_messages_queued(&self, server_name: &str) -> QueryResult<Vec<(String, String, chrono::DateTime<chrono::Utc>, String)>> {
    let mut db_connection = self.0.get().unwrap();
    use crate::schema::player::dsl as player_schema;
    use crate::schema::remoteplayerchat::dsl as chat_schema;
    chat_schema::remoteplayerchat
      .select((
        chat_schema::body,
        chat_schema::remote_player,
        chat_schema::created,
        sql_not_null_str(player_schema::player.select(player_schema::name).filter(player_schema::id.eq(chat_schema::player)).single_value()),
      ))
      .filter(chat_schema::remote_server.eq(server_name).and(chat_schema::state.eq("O")))
      .load::<(String, String, chrono::DateTime<chrono::Utc>, String)>(&mut db_connection)
  }
  pub fn remote_direct_messages_receive(
    &self,
    server_name: &str,
    messages: Vec<crate::peer::PeerDirectMessage>,
  ) -> QueryResult<Vec<(String, String, String, chrono::DateTime<chrono::Utc>)>> {
    use crate::schema::player::dsl as player_schema;
    use crate::schema::remoteplayerchat::dsl as chat_schema;
    let mut db_connection = self.0.get().unwrap();
    let ids_for_user: std::collections::HashMap<_, _> = player_schema::player
      .select((player_schema::name, player_schema::id))
      .filter(player_schema::name.eq_any(messages.iter().map(|m| &m.recipient).collect::<Vec<_>>()))
      .load::<(String, i32)>(&mut db_connection)?
      .into_iter()
      .collect();
    diesel::insert_into(chat_schema::remoteplayerchat)
      .values(
        messages
          .into_iter()
          .flat_map(|m| {
            ids_for_user.get(&m.recipient).map(|id| {
              (
                chat_schema::player.eq(id),
                chat_schema::remote_server.eq(server_name),
                chat_schema::remote_player.eq(m.sender),
                chat_schema::body.eq(m.body),
                chat_schema::created.eq(m.timestamp),
                chat_schema::state.eq("r"),
              )
            })
          })
          .collect::<Vec<_>>(),
      )
      .on_conflict_do_nothing()
      .returning((
        sql_not_null_str(player_schema::player.select(player_schema::name).filter(player_schema::id.eq(chat_schema::player)).single_value()),
        chat_schema::remote_player,
        chat_schema::body,
        chat_schema::created,
      ))
      .load::<(String, String, String, chrono::DateTime<chrono::Utc>)>(&mut db_connection)
  }
  pub fn remote_direct_messages_sent(&self, server_name: &str) -> QueryResult<usize> {
    use crate::schema::remoteplayerchat::dsl as chat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(chat_schema::remoteplayerchat.filter(chat_schema::remote_server.eq(server_name).and(chat_schema::state.eq("O"))))
      .set(chat_schema::state.eq("o"))
      .execute(&mut db_connection)
  }

  pub fn remote_direct_message_write(
    &self,
    db_id: i32,
    player: &str,
    remote_name: &str,
    body: &str,
    timestamp: &chrono::DateTime<chrono::Utc>,
    state: &str,
  ) -> QueryResult<usize> {
    use crate::schema::remoteplayerchat::dsl as remoteplayerchat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(remoteplayerchat_schema::remoteplayerchat)
      .values(&(
        remoteplayerchat_schema::player.eq(db_id),
        remoteplayerchat_schema::remote_player.eq(&player),
        remoteplayerchat_schema::remote_server.eq(&remote_name),
        remoteplayerchat_schema::body.eq(&body),
        remoteplayerchat_schema::created.eq(&timestamp),
        remoteplayerchat_schema::state.eq(state),
      ))
      .execute(&mut db_connection)
  }
  pub fn train_add(&self, asset: &str, allowed_first: bool) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    let waiting_players = db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use crate::schema::player::dsl as player_schema;
      use crate::schema::realm::dsl as realm_schema;
      use crate::schema::realmtrain::dsl as realmtrain_schema;
      diesel::insert_into(realmtrain_schema::realmtrain)
        .values((realmtrain_schema::asset.eq(asset), realmtrain_schema::allowed_first.eq(allowed_first)))
        .on_conflict(realmtrain_schema::asset)
        .do_update()
        .set(realmtrain_schema::allowed_first.eq(allowed_first))
        .execute(db_connection)?;
      player_schema::player
        .select((
          player_schema::name,
          realm_schema::realm.select(diesel::dsl::max(realm_schema::train)).filter(realm_schema::owner.eq(player_schema::id)).single_value(),
        ))
        .filter(player_schema::waiting_for_train.and(diesel::dsl::not(diesel::dsl::exists(
          realm_schema::realm.filter(realm_schema::owner.eq(player_schema::id).and(realm_schema::asset.eq(asset))),
        ))))
        .load::<(String, Option<i32>)>(db_connection)
    })?;
    for (owner, train) in waiting_players {
      use crate::schema::player::dsl as player_schema;
      create_realm(&mut db_connection, asset, &owner, None, None, Some(train.unwrap_or(0) as u16))?;
      diesel::update(player_schema::player.filter(player_schema::name.eq(&owner)))
        .set(player_schema::waiting_for_train.eq(false))
        .execute(&mut db_connection)?;
    }
    Ok(())
  }

  pub(crate) fn player_avatar(&self, db_id: i32, avatar: &puzzleverse_core::avatar::Avatar) -> diesel::QueryResult<()> {
    use crate::schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let avatar = rmp_serde::to_vec(avatar).unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id))).set(player_schema::avatar.eq(avatar)).execute(&mut db_connection)?;
    Ok(())
  }
}
fn create_realm(
  db_connection: &mut DbConnection,
  asset: &str,
  owner: &str,
  name: Option<String>,
  seed: Option<i32>,
  train: Option<u16>,
) -> diesel::QueryResult<String> {
  use sha3::Digest;
  let mut principal_hash = sha3::Sha3_512::new();
  principal_hash.update(owner.as_bytes());
  principal_hash.update(&[0]);
  principal_hash.update(asset.as_bytes());
  principal_hash.update(&[0]);
  principal_hash.update(chrono::Utc::now().to_rfc3339().as_bytes());
  let principal = hex::encode(principal_hash.finalize());
  use crate::schema::player::dsl as player_schema;
  use crate::schema::realm::dsl as realm_schema;
  diesel::insert_into(realm_schema::realm)
    .values((
      realm_schema::principal.eq(&principal),
      realm_schema::name.eq(name.unwrap_or(format!("{}'s {}", owner, asset))),
      realm_schema::owner.eq(sql_not_null_int(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(owner)).single_value())),
      realm_schema::asset.eq(asset),
      realm_schema::state.eq(vec![]),
      realm_schema::seed.eq(seed.unwrap_or(rand::thread_rng().gen())),
      realm_schema::access_acl.eq(sql_not_null_bytes(
        player_schema::player.select(player_schema::new_realm_access_acl).filter(player_schema::name.eq(&owner)).single_value(),
      )),
      realm_schema::admin_acl.eq(sql_not_null_bytes(
        player_schema::player.select(player_schema::new_realm_access_acl).filter(player_schema::name.eq(&owner)).single_value(),
      )),
      realm_schema::in_directory.eq(false),
      realm_schema::initialised.eq(false),
      realm_schema::train.eq(train.map(|t| t as i32)),
    ))
    .returning(realm_schema::principal)
    .on_conflict((realm_schema::owner, realm_schema::asset))
    .do_update()
    .set(realm_schema::updated_at.eq(chrono::Utc::now()))
    .get_result::<String>(db_connection)
}

fn parse_player_acl(acl: Vec<u8>, default: crate::AccessControlSetting) -> std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>> {
  std::sync::Arc::new(tokio::sync::Mutex::new(if acl.is_empty() {
    default
  } else {
    match rmp_serde::from_slice(acl.as_slice()) {
      Ok(v) => v,
      Err(e) => {
        eprintln!("ACL in database is corrupt: {}", e);
        default
      }
    }
  }))
}

pub(crate) fn read_acl(
  db_pool: &DbPool,
  category: &str,
  default: crate::AccessControlSetting,
) -> std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>> {
  let mut db_connection = db_pool.get().unwrap();
  use crate::schema::serveracl::dsl as serveracl_schema;

  std::sync::Arc::new(tokio::sync::Mutex::new(
    match serveracl_schema::serveracl
      .select(serveracl_schema::acl)
      .filter(serveracl_schema::category.eq(category))
      .first::<Vec<u8>>(&mut db_connection)
    {
      Err(diesel::result::Error::NotFound) => default,
      Err(e) => {
        eprintln!("Failed to get server ACL: {}", e);
        default
      }
      Ok(acl) => match rmp_serde::from_slice(acl.as_slice()) {
        Ok(v) => v,
        Err(e) => {
          eprintln!("ACL in database is corrupt: {}", e);
          default
        }
      },
    },
  ))
}
