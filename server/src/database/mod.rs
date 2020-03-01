pub mod persisted;
pub mod realm_scope;
pub mod schema;
use std::hash::Hasher;

use diesel::prelude::*;

pub(crate) struct Database(DbPool);
pub type DbPool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>>;

sql_function! { #[sql_name = "gen_calendar_id"]fn sql_gen_calendar_id() -> diesel::sql_types::Binary}
sql_function! { #[sql_name = ""]fn sql_not_null_json(x: diesel::sql_types::Nullable<diesel::sql_types::Jsonb>) -> diesel::sql_types::Jsonb}
sql_function! { #[sql_name = ""]fn sql_not_null_int(x: diesel::sql_types::Nullable<diesel::sql_types::Integer>) -> diesel::sql_types::Integer}
sql_function! { #[sql_name = ""]fn sql_not_null_str(x: diesel::sql_types::Nullable<diesel::sql_types::VarChar>) -> diesel::sql_types::VarChar}
sql_function! { #[sql_name = "COALESCE"]fn sql_coalesce_bool(x: diesel::sql_types::Nullable<diesel::sql_types::Bool>, y: diesel::sql_types::Bool) -> diesel::sql_types::Bool}
pub(crate) struct PlayerInfo {
  pub(crate) db_id: i32,
  pub(crate) debuted: bool,
  pub(crate) message_acl: crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
  pub(crate) location_acl: crate::access::AccessSetting<spadina_core::access::LocationAccess>,
  pub(crate) calendar_id: Vec<u8>,
}
pub struct RealmLoadInfo {
  pub seed: i32,
  pub solved: bool,
  pub state: Option<Vec<serde_json::Value>>,
  pub train: Option<u16>,
}
pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

impl Database {
  pub fn new(db_url: &str, default_realm: Option<impl AsRef<str>>) -> Self {
    let pool = {
      let manager = diesel::r2d2::ConnectionManager::<diesel::pg::PgConnection>::new(db_url);
      diesel::r2d2::Pool::builder().build(manager).expect("Failed to create pool.")
    };
    use diesel_migrations::MigrationHarness;
    use schema::realmtrain::dsl as realmtrain_schema;
    let mut db_connection = pool.get().expect("Failed to connect to database");
    db_connection.run_pending_migrations(MIGRATIONS).expect("Failed to migrate database to latest schema");
    if let Some(default_realm) = default_realm {
      diesel::insert_into(realmtrain_schema::realmtrain)
        .values(&(realmtrain_schema::asset.eq(default_realm.as_ref()), realmtrain_schema::allowed_first.eq(true)))
        .on_conflict(realmtrain_schema::asset)
        .do_update()
        .set(realmtrain_schema::allowed_first.eq(true))
        .execute(&mut db_connection)
        .expect("Failed to make sure default realm is present");
    }
    Database(pool)
  }
  pub fn get(&self) -> Result<diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>, impl std::error::Error> {
    self.0.get()
  }
  pub fn acl_read(&self, category: &str) -> diesel::result::QueryResult<crate::access::AccessSetting<spadina_core::access::SimpleAccess>> {
    use schema::serveracl::dsl as serveracl_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      serveracl_schema::serveracl
        .select(serveracl_schema::acl)
        .filter(serveracl_schema::category.eq(category))
        .get_result::<diesel_json::Json<crate::access::AccessSetting<spadina_core::access::SimpleAccess>>>(&mut db_connection)
        .optional()?
        .map(|j| j.0)
        .unwrap_or_default(),
    )
  }
  pub fn acl_write(
    &self,
    category: &str,
    acls: &crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
  ) -> diesel::result::QueryResult<()> {
    use schema::serveracl::dsl as serveracl_schema;
    let mut db_connection = self.0.get().unwrap();

    diesel::insert_into(serveracl_schema::serveracl)
      .values(&(serveracl_schema::category.eq(category), serveracl_schema::acl.eq(diesel_json::Json(acls))))
      .on_conflict(serveracl_schema::category)
      .do_update()
      .set(serveracl_schema::acl.eq(diesel_json::Json(acls)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn announcements_read(&self) -> diesel::QueryResult<Vec<spadina_core::communication::Announcement<std::sync::Arc<str>>>> {
    use diesel::prelude::*;
    use schema::announcement::dsl as announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    announcement_schema::announcement
      .select((
        announcement_schema::title,
        announcement_schema::body,
        announcement_schema::when,
        announcement_schema::realm,
        announcement_schema::public,
      ))
      .load_iter::<(
        String,
        String,
        diesel_json::Json<spadina_core::communication::AnnouncementTime>,
        diesel_json::Json<Option<spadina_core::realm::RealmTarget<std::sync::Arc<str>>>>,
        bool,
      ), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .map(|r| {
        r.map(|(title, body, when, realm, public)| spadina_core::communication::Announcement {
          title: std::sync::Arc::from(title),
          body: std::sync::Arc::from(body),
          when: when.0,
          realm: realm.0,
          public,
        })
      })
      .collect()
  }
  pub fn announcements_write(
    &self,
    announcements: &[spadina_core::communication::Announcement<impl AsRef<str> + std::fmt::Debug + serde::Serialize>],
  ) -> diesel::QueryResult<()> {
    use diesel::prelude::*;
    use schema::announcement::dsl as announcement_schema;
    self.0.get().unwrap().transaction::<(), diesel::result::Error, _>(|db_connection| {
      diesel::delete(announcement_schema::announcement).execute(db_connection)?;
      for a in announcements {
        diesel::insert_into(announcement_schema::announcement)
          .values((
            announcement_schema::title.eq(a.title.as_ref()),
            announcement_schema::body.eq(a.body.as_ref()),
            announcement_schema::when.eq(diesel_json::Json(&a.when)),
            announcement_schema::realm.eq(diesel_json::Json(&a.realm)),
            announcement_schema::public.eq(a.public),
          ))
          .execute(db_connection)?;
      }
      Ok(())
    })
  }
  pub fn banned_peers_list(&self) -> QueryResult<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::bannedpeers::dsl as bannedpeers_schema;
    Ok(
      bannedpeers_schema::bannedpeers
        .select(bannedpeers_schema::ban)
        .load_iter::<diesel_json::Json<spadina_core::access::BannedPeer<String>>, diesel::connection::DefaultLoadingMode>(&mut db_connection)?
        .map(|r| r.map(|j| j.0))
        .collect::<Result<_, _>>()?,
    )
  }
  pub fn banned_peers_write(&self, bans: &std::collections::HashSet<spadina_core::access::BannedPeer<String>>) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<(), diesel::result::Error, _>(|db_connection| {
      use schema::bannedpeers::dsl as bannedpeers_schema;
      diesel::delete(bannedpeers_schema::bannedpeers).execute(db_connection)?;
      let bans: Vec<_> = bans.iter().map(|ban| bannedpeers_schema::ban.eq(diesel_json::Json(ban))).collect();

      diesel::insert_into(bannedpeers_schema::bannedpeers).values(&bans).execute(db_connection)?;
      Ok(())
    })
  }
  pub fn bookmark_add(
    &self,
    db_id: i32,
    bookmark: &spadina_core::communication::Bookmark<impl AsRef<str> + serde::Serialize + std::fmt::Debug>,
  ) -> QueryResult<usize> {
    let mut db_connection = self.0.get().unwrap();
    use schema::bookmark::dsl as bookmark_schema;
    diesel::insert_into(bookmark_schema::bookmark)
      .values(&(bookmark_schema::player.eq(db_id), bookmark_schema::value.eq(diesel_json::Json(bookmark))))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn bookmark_get<R, C: FromIterator<R>>(
    &self,
    db_id: i32,
    filter: impl Fn(spadina_core::communication::Bookmark<String>) -> Option<R>,
  ) -> QueryResult<C> {
    let mut db_connection = self.0.get().unwrap();
    use schema::bookmark::dsl as bookmark_schema;
    bookmark_schema::bookmark
      .select(bookmark_schema::value)
      .filter(bookmark_schema::player.eq(db_id))
      .load_iter::<diesel_json::Json<spadina_core::communication::Bookmark<String>>, diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|bookmark| match bookmark {
        Ok(bookmark) => match filter(bookmark.0) {
          Some(bookmark) => Some(Ok(bookmark)),
          None => None,
        },
        Err(e) => Some(Err(e)),
      })
      .collect()
  }
  pub fn bookmark_rm(
    &self,
    db_id: i32,
    bookmark: &spadina_core::communication::Bookmark<impl AsRef<str> + serde::Serialize + std::fmt::Debug>,
  ) -> QueryResult<()> {
    use schema::bookmark::dsl as bookmark_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(bookmark_schema::bookmark.filter(bookmark_schema::player.eq(db_id).and(bookmark_schema::value.eq(&diesel_json::Json(bookmark)))))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_id(&self, db_id: i32) -> QueryResult<Vec<u8>> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id)))
      .set(player_schema::calendar_id.eq(sql_gen_calendar_id()))
      .returning(player_schema::calendar_id)
      .get_result(&mut db_connection)
  }
  pub fn calendar_reset(&self, player_name: &str) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::name.eq(player_name)))
      .set(player_schema::calendar_id.eq(sql_gen_calendar_id()))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_check(&self, calendar_id: &[u8]) -> QueryResult<bool> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let count = player_schema::player
      .select(diesel::dsl::count_star())
      .filter(player_schema::calendar_id.eq(calendar_id))
      .get_result::<i64>(&mut db_connection)?;
    Ok(count > 0)
  }
  pub fn calendar_list(
    &self,
    db_id: i32,
    local_server: &std::sync::Arc<str>,
  ) -> QueryResult<Vec<spadina_core::realm::RealmDirectoryEntry<crate::shstr::ShStr>>> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmcalendarsubscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    calendar_schema::realmcalendarsubscription
      .inner_join(realm_schema::realm.on(calendar_schema::realm.eq(realm_schema::id)))
      .inner_join(player_schema::player.on(player_schema::id.eq(realm_schema::owner)))
      .select((realm_schema::asset, player_schema::name, realm_schema::name))
      .filter(calendar_schema::player.eq(db_id))
      .load_iter::<(String, String, String), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .map(|r| match r {
        Ok((asset, owner, name)) => Ok(spadina_core::realm::RealmDirectoryEntry {
          activity: spadina_core::realm::RealmActivity::Unknown,
          asset: crate::shstr::ShStr::Single(asset),
          name: crate::shstr::ShStr::Single(name),
          owner: crate::shstr::ShStr::Single(owner),
          server: crate::shstr::ShStr::Shared(local_server.clone()),
          train: None,
        }),
        Err(e) => Err(e),
      })
      .collect()
  }
  pub fn calendar_rm(&self, db_id: i32, realm: &spadina_core::realm::LocalRealmTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmcalendarsubscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(
      calendar_schema::realmcalendarsubscription.filter(
        calendar_schema::player.eq(db_id).and(
          calendar_schema::realm.eq_any(
            realm_schema::realm
              .inner_join(player_schema::player.on(player_schema::id.eq(realm_schema::owner)))
              .select(realm_schema::id)
              .filter(realm_schema::asset.eq(realm.asset.as_ref()).and(player_schema::name.eq(realm.owner.as_ref()))),
          ),
        ),
      ),
    )
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_add(&self, db_id: i32, realm: &spadina_core::realm::LocalRealmTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmcalendarsubscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .inner_join(player_schema::player.on(player_schema::id.eq(realm_schema::owner)))
      .select((<i32 as diesel::expression::AsExpression<diesel::sql_types::Integer>>::as_expression(db_id), realm_schema::id))
      .filter(realm_schema::asset.eq(realm.asset.as_ref()).and(player_schema::name.eq(realm.owner.as_ref())))
      .insert_into(calendar_schema::realmcalendarsubscription)
      .into_columns((calendar_schema::player, calendar_schema::realm))
      .on_conflict((calendar_schema::player, calendar_schema::realm))
      .do_nothing()
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_rm_all(&self, db_id: i32) -> QueryResult<()> {
    use schema::realmcalendarsubscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(calendar_schema::realmcalendarsubscription.filter(calendar_schema::player.eq(db_id))).execute(&mut db_connection)?;
    Ok(())
  }
  pub fn direct_message_clean(&self) -> QueryResult<()> {
    use schema::localplayerchat::dsl as local_player_chat_schema;
    use schema::realmchat::dsl as realm_chat_schema;
    use schema::remoteplayerchat::dsl as remote_player_chat_schema;
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
  ) -> QueryResult<Vec<spadina_core::communication::DirectMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::localplayerchat::dsl as chat_schema;
    use schema::player::dsl as player_schema;
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
      .load_iter::<(diesel_json::Json<spadina_core::communication::MessageBody<String>>, chrono::DateTime<chrono::Utc>, bool), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(spadina_core::communication::DirectMessage { inbound, body: body.0, timestamp }),
      })
      .collect()
  }
  pub fn direct_message_write(
    &self,
    sender: &str,
    recipient: &str,
    body: &spadina_core::communication::MessageBody<impl AsRef<str> + serde::Serialize + std::fmt::Debug>,
  ) -> QueryResult<chrono::DateTime<chrono::Utc>> {
    use schema::localplayerchat::dsl as chat_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let recipient_id = player_schema::player.select(player_schema::id).filter(player_schema::name.eq(recipient)).first::<i32>(&mut db_connection)?;
    let timestamp = chrono::Utc::now();
    diesel::insert_into(chat_schema::localplayerchat)
      .values(&(
        chat_schema::recipient.eq(recipient_id),
        chat_schema::body.eq(diesel_json::Json(body)),
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
  ) -> diesel::QueryResult<(
    std::collections::HashMap<spadina_core::player::PlayerIdentifier<String>, spadina_core::communication::DirectMessageInfo>,
    chrono::DateTime<chrono::Utc>,
  )> {
    let mut db_connection = self.0.get().unwrap();
    use schema::localplayerchat::dsl as local_player_chat_schema;
    use schema::localplayerlastread::dsl as local_player_last_read_schema;
    use schema::player::dsl as player_schema;
    use schema::remoteplayerchat::dsl as remote_player_chat_schema;
    use schema::remoteplayerlastread::dsl as remote_player_last_read_schema;
    let mut stats = player_schema::player
      .left_join(local_player_last_read_schema::localplayerlastread.on(local_player_last_read_schema::sender.eq(player_schema::id)))
      .filter(local_player_last_read_schema::recipient.eq(db_id))
      .select((
        player_schema::name,
        local_player_chat_schema::localplayerchat
          .select(diesel::dsl::max(local_player_chat_schema::created))
          .filter(local_player_chat_schema::recipient.eq(db_id).and(local_player_chat_schema::recipient.eq(player_schema::id)))
          .single_value(),
        local_player_last_read_schema::when.nullable(),
      ))
      .load_iter::<(String, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>), diesel::connection::DefaultLoadingMode>(
        &mut db_connection,
      )?
      .filter_map(|r| match r {
        Ok((player, Some(last_received), last_read)) => Some(Ok((
          spadina_core::player::PlayerIdentifier::Local(player),
          spadina_core::communication::DirectMessageInfo { last_received, last_read },
        ))),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
      })
      .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    for item in remote_player_chat_schema::remoteplayerchat
      .filter(remote_player_chat_schema::player.eq(db_id).and(remote_player_chat_schema::inbound.eq(false)))
      .group_by((remote_player_chat_schema::remote_player, remote_player_chat_schema::remote_server))
      .select((
        remote_player_chat_schema::remote_player,
        remote_player_chat_schema::remote_server,
        diesel::dsl::max(remote_player_chat_schema::created),
        remote_player_last_read_schema::remoteplayerlastread
          .select(diesel::dsl::max(remote_player_last_read_schema::when))
          .filter(
            remote_player_last_read_schema::player
              .eq(remote_player_chat_schema::player)
              .and(remote_player_last_read_schema::remote_player.eq(remote_player_chat_schema::remote_player))
              .and(remote_player_last_read_schema::remote_server.eq(remote_player_chat_schema::remote_server)),
          )
          .single_value(),
      ))
      .load_iter::<(String, String, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>), diesel::connection::DefaultLoadingMode>(
        &mut db_connection,
      )?
      .filter_map(|r| match r {
        Ok((player, server, Some(last_received), last_read)) => Some(Ok((
          spadina_core::player::PlayerIdentifier::Remote { server, player },
          spadina_core::communication::DirectMessageInfo { last_received, last_read },
        ))),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
      })
    {
      let (player, info) = item?;
      stats.insert(player, info);
    }
    let last_login = player_schema::player
      .select(player_schema::last_login)
      .filter(player_schema::id.eq(db_id))
      .first::<chrono::DateTime<chrono::Utc>>(&mut db_connection)?;
    Ok((stats, last_login))
  }
  pub fn direct_message_last_read_set(&self, db_id: i32, sender: &str) -> QueryResult<chrono::DateTime<chrono::Utc>> {
    use schema::localplayerlastread::dsl as local_player_last_read_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = chrono::Utc::now();
    diesel::insert_into(local_player_last_read_schema::localplayerlastread)
      .values((
        local_player_last_read_schema::recipient.eq(db_id),
        local_player_last_read_schema::sender
          .eq(sql_not_null_int(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value())),
        local_player_last_read_schema::when.eq(now),
      ))
      .on_conflict((local_player_last_read_schema::recipient, local_player_last_read_schema::sender))
      .do_update()
      .set(local_player_last_read_schema::when.eq(now))
      .execute(&mut db_connection)?;
    Ok(now)
  }
  pub fn leaderboard(&self) -> QueryResult<std::collections::BTreeMap<String, i64>> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmtrain::dsl as train_schema;
    let mut db_connection = self.0.get().unwrap();
    player_schema::player
      .inner_join(realm_schema::realm)
      .inner_join(train_schema::realmtrain.on(train_schema::asset.eq(realm_schema::asset)))
      .filter(realm_schema::solved)
      .group_by(player_schema::name)
      .having(diesel::dsl::count_star().gt(1))
      .order_by(diesel::dsl::count_star())
      .select((player_schema::name, diesel::dsl::count_star()))
      .limit(10)
      .load_iter::<(String, i64), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .collect::<diesel::QueryResult<_>>()
  }
  pub fn player_debut(&self, db_id: i32) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id))).set(player_schema::debuted.eq(true)).execute(&mut db_connection)?;
    Ok(())
  }
  pub fn player_reset(&self, player_name: &str, reset: bool) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    use schema::player::dsl as player_schema;
    diesel::update(player_schema::player.filter(player_schema::name.eq(player_name)))
      .set(player_schema::reset.eq(reset))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn player_clean(&self) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<(), diesel::result::Error, _>(|db_connection| {
      use schema::bookmark::dsl as bookmark_schema;
      use schema::localplayerchat::dsl as localplayerchat_schema;
      use schema::player::dsl as player_schema;
      use schema::publickey::dsl as publickey_schema;
      use schema::realm::dsl as realm_schema;
      use schema::realmcalendarsubscription::dsl as realmcalendarsubscription_schema;
      use schema::realmchat::dsl as realmchat_schema;
      use schema::remoteplayerchat::dsl as remoteplayerchat_schema;
      diesel::delete(
        localplayerchat_schema::localplayerchat.filter(
          localplayerchat_schema::sender
            .eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))
            .or(localplayerchat_schema::recipient.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
        ),
      )
      .execute(db_connection)?;
      diesel::delete(
        realmcalendarsubscription_schema::realmcalendarsubscription
          .filter(realmcalendarsubscription_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        remoteplayerchat_schema::remoteplayerchat
          .filter(remoteplayerchat_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        realmchat_schema::realmchat.filter(
          realmchat_schema::realm.eq_any(
            realm_schema::realm
              .select(realm_schema::id)
              .filter(realm_schema::owner.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
          ),
        ),
      )
      .execute(db_connection)?;
      diesel::delete(
        realm_schema::realm.filter(realm_schema::owner.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        bookmark_schema::bookmark
          .filter(bookmark_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        publickey_schema::publickey
          .filter(publickey_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(player_schema::player.filter(player_schema::reset)).execute(db_connection)?;

      Ok(())
    })
  }
  pub(crate) fn player_load(&self, player_name: &str, create: bool) -> QueryResult<Option<PlayerInfo>> {
    use schema::player::dsl as player_schema;
    type PlayerInfoTuple = (
      i32,
      bool,
      diesel_json::Json<crate::access::AccessSetting<spadina_core::access::SimpleAccess>>,
      diesel_json::Json<crate::access::AccessSetting<spadina_core::access::LocationAccess>>,
      Vec<u8>,
    );
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      // Find or create the player's database entry
      let results: Option<PlayerInfoTuple> = player_schema::player
        .select((player_schema::id, player_schema::debuted, player_schema::message_acl, player_schema::online_acl, player_schema::calendar_id))
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
                player_schema::avatar.eq(diesel_json::Json(spadina_core::avatar::Avatar::default())),
                player_schema::message_acl
                  .eq(diesel_json::Json(crate::access::AccessSetting { default: spadina_core::access::SimpleAccess::Allow, rules: Vec::new() })),
                player_schema::online_acl.eq(diesel_json::Json(crate::access::AccessSetting::<spadina_core::access::LocationAccess>::default())),
                player_schema::new_realm_access_acl
                  .eq(diesel_json::Json(crate::access::AccessSetting::<spadina_core::access::SimpleAccess>::default())),
                player_schema::new_realm_admin_acl
                  .eq(diesel_json::Json(crate::access::AccessSetting::<spadina_core::access::SimpleAccess>::default())),
              ))
              .returning((
                player_schema::id,
                player_schema::debuted,
                player_schema::message_acl,
                player_schema::online_acl,
                player_schema::calendar_id,
              ))
              .get_result::<PlayerInfoTuple>(db_connection)
              .optional()?
          } else {
            None
          }
        }
        Some(record) => Some(record),
      };

      Ok(match db_record {
        Some((id, debuted, message_acl, online_acl, calendar_id)) => {
          diesel::update(player_schema::player.filter(player_schema::id.eq(id)))
            .set(player_schema::last_login.eq(chrono::Utc::now()))
            .execute(db_connection)?;

          Some(PlayerInfo { db_id: id, debuted, message_acl: message_acl.0, location_acl: online_acl.0, calendar_id })
        }
        None => None,
      })
    })
  }
  pub fn public_key_add(&self, db_id: i32, der: &[u8]) -> QueryResult<usize> {
    use schema::publickey::dsl as publickey_dsl;
    let fingerprint = spadina_core::auth::compute_fingerprint(der);
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(publickey_dsl::publickey)
      .values((publickey_dsl::player.eq(db_id), publickey_dsl::fingerprint.eq(fingerprint), publickey_dsl::public_key.eq(der)))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn public_key_get(&self, player_name: &str) -> QueryResult<Vec<(String, Vec<u8>)>> {
    use schema::player::dsl as player_dsl;
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    publickey_dsl::publickey
      .select((publickey_dsl::fingerprint, publickey_dsl::public_key))
      .filter(
        publickey_dsl::player.eq(sql_not_null_int(player_dsl::player.select(player_dsl::id).filter(player_dsl::name.eq(player_name)).single_value())),
      )
      .load(&mut db_connection)
  }
  pub fn public_key_list(&self, db_id: i32) -> QueryResult<Vec<spadina_core::auth::PublicKey<String>>> {
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    publickey_dsl::publickey
      .select((publickey_dsl::fingerprint, publickey_dsl::created, publickey_dsl::last_used))
      .filter(publickey_dsl::player.eq(&db_id))
      .load_iter::<(String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>), diesel::connection::DefaultLoadingMode>(
        &mut db_connection,
      )?
      .map(|r| match r {
        Ok((fingerprint, created, last_used)) => Ok(spadina_core::auth::PublicKey { fingerprint, created, last_used }),
        Err(e) => Err(e),
      })
      .collect()
  }
  pub fn public_key_touch(&self, player_name: &str, fingerprint: &str) -> QueryResult<()> {
    use schema::player::dsl as player_dsl;
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(
      publickey_dsl::publickey.filter(
        publickey_dsl::player
          .eq(sql_not_null_int(player_dsl::player.select(player_dsl::id).filter(player_dsl::name.eq(player_name)).single_value()))
          .and(publickey_dsl::fingerprint.eq(fingerprint)),
      ),
    )
    .set(publickey_dsl::last_used.eq(chrono::Utc::now()))
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn public_key_rm(&self, db_id: i32, fingerprint: &str) -> QueryResult<usize> {
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(publickey_dsl::publickey.filter(publickey_dsl::fingerprint.eq(fingerprint).and(publickey_dsl::player.eq(db_id))))
      .execute(&mut db_connection)
  }
  pub fn public_key_rm_all(&self, db_id: i32) -> QueryResult<usize> {
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(publickey_dsl::publickey.filter(publickey_dsl::player.eq(&db_id))).execute(&mut db_connection)
  }
  pub fn realm_acl_read<
    C: diesel::Column<Table = crate::database::schema::realm::dsl::realm, SqlType = diesel::sql_types::Jsonb>
      + diesel::expression::ValidGrouping<()>
      + diesel::SelectableExpression<schema::realm::table>
      + diesel::query_builder::QueryId
      + diesel::query_builder::QueryFragment<diesel::pg::Pg>,
  >(
    &self,
    id: i32,
    column: C,
  ) -> QueryResult<crate::access::AccessSetting<spadina_core::access::SimpleAccess>> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .select(column)
      .filter(realm_schema::id.eq(id))
      .first::<diesel_json::Json<crate::access::AccessSetting<spadina_core::access::SimpleAccess>>>(&mut db_connection)
      .map(|j| j.0)
  }
  pub fn realm_acl_write<C: diesel::Column<Table = crate::database::schema::realm::dsl::realm, SqlType = diesel::sql_types::Jsonb>>(
    &self,
    id: i32,
    column: C,
    access_control: &crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
  ) -> QueryResult<()> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(id)))
      .set(column.eq(diesel_json::Json(access_control)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub(crate) fn realm_acl_write_bulk(
    &self,
    scope: realm_scope::RealmListScope<impl AsRef<str>>,
    mut targets: std::collections::HashMap<
      spadina_core::realm::RealmAccessTarget,
      spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>,
    >,
  ) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;

    let mut db_connection = self.0.get().unwrap();
    let realms: Vec<_> =
      realm_schema::realm.inner_join(player_schema::player).select(realm_schema::id).filter(scope.as_expression()).load::<i32>(&mut db_connection)?;

    diesel::update(realm_schema::realm.filter(realm_schema::id.eq_any(realms)))
      .set((
        targets
          .remove(&spadina_core::realm::RealmAccessTarget::Admin)
          .map(|access_control| realm_schema::admin_acl.eq(diesel_json::Json(access_control))),
        targets
          .remove(&spadina_core::realm::RealmAccessTarget::Access)
          .map(|access_control| realm_schema::access_acl.eq(diesel_json::Json(access_control))),
      ))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn realm_announcements_write(
    &self,
    id: i32,
    announcements: &[spadina_core::realm::RealmAnnouncement<impl AsRef<str>>],
  ) -> diesel::QueryResult<()> {
    use diesel::prelude::*;
    use schema::realmannouncement::dsl as realm_announcement_schema;
    self.0.get().unwrap().transaction::<(), diesel::result::Error, _>(|db_connection| {
      diesel::delete(realm_announcement_schema::realmannouncement.filter(realm_announcement_schema::realm.eq(id))).execute(db_connection)?;
      let rows: Vec<_> = announcements
        .iter()
        .map(|a| {
          (
            realm_announcement_schema::title.eq(a.title.as_ref()),
            realm_announcement_schema::body.eq(a.body.as_ref()),
            realm_announcement_schema::when.eq(diesel_json::Json(&a.when)),
            realm_announcement_schema::public.eq(a.public),
          )
        })
        .collect();
      diesel::insert_into(realm_announcement_schema::realmannouncement).values(&rows).execute(db_connection)?;

      Ok(())
    })
  }
  pub fn realm_announcements_clean(&self) -> diesel::QueryResult<()> {
    use diesel::prelude::*;
    use schema::realmannouncement::dsl as realm_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = chrono::Utc::now();
    db_connection.transaction(|db_connection| {
      let ids = realm_announcement_schema::realmannouncement
        .select((realm_announcement_schema::id, realm_announcement_schema::when))
        .load_iter::<(i32, diesel_json::Json<spadina_core::communication::AnnouncementTime>), diesel::connection::DefaultLoadingMode>(db_connection)?
        .filter_map(|result| match result {
          Err(e) => Some(Err(e)),
          Ok((id, when)) => {
            if when.0.expires() < now {
              Some(Ok(id))
            } else {
              None
            }
          }
        })
        .collect::<QueryResult<Vec<_>>>()?;
      diesel::delete(realm_announcement_schema::realmannouncement.filter(realm_announcement_schema::id.eq_any(ids))).execute(db_connection)?;
      Ok(())
    })
  }
  pub fn realm_announcements_read(&self, id: i32) -> QueryResult<Vec<spadina_core::realm::RealmAnnouncement<String>>> {
    use schema::realmannouncement::dsl as realm_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_announcement_schema::realmannouncement
      .select((
        realm_announcement_schema::id,
        realm_announcement_schema::title,
        realm_announcement_schema::body,
        realm_announcement_schema::when,
        realm_announcement_schema::public,
      ))
      .filter(realm_announcement_schema::realm.eq(id))
      .load_iter::<(i32, String, String, diesel_json::Json<spadina_core::communication::AnnouncementTime>, bool), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .map(|result| result .map(|(_, title, body, when, public)|spadina_core::realm::RealmAnnouncement { title, body, public, when:when.0 }))
      .collect()
  }
  pub fn realm_announcements_fetch_all(
    &self,
    scope: realm_scope::RealmListScope<impl AsRef<str>>,
    player_secret_id: Option<Vec<u8>>,
    local_server: &str,
  ) -> QueryResult<Vec<(spadina_core::realm::LocalRealmTarget<String>, spadina_core::realm::RealmAnnouncement<String>)>> {
    use diesel::expression::AsExpression;
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmannouncement::dsl as realm_announcement_schema;
    type AnnouncementTuple = (
      String,
      String,
      i32,
      String,
      String,
      diesel_json::Json<spadina_core::communication::AnnouncementTime>,
      bool,
      bool,
      diesel_json::Json<crate::access::AccessSetting<spadina_core::access::SimpleAccess>>,
      String,
    );
    let mut db_connection = self.0.get().unwrap();
    let public_filter: Box<dyn diesel::BoxableExpression<_, diesel::pg::Pg, SqlType = diesel::sql_types::Bool>> = match player_secret_id.as_ref() {
      None => Box::new(realm_announcement_schema::public.as_expression()),
      Some(calendar_id) => Box::new(player_schema::calendar_id.eq(calendar_id).or(realm_announcement_schema::public).as_expression()),
    };
    let query = realm_announcement_schema::realmannouncement
      .inner_join(realm_schema::realm.on(realm_schema::id.eq(realm_announcement_schema::realm)))
      .inner_join(player_schema::player.on(player_schema::id.eq(realm_schema::owner)))
      .select((
        player_schema::name,
        realm_schema::asset,
        realm_announcement_schema::id,
        realm_announcement_schema::title,
        realm_announcement_schema::body,
        realm_announcement_schema::when,
        realm_announcement_schema::public,
        realm_announcement_schema::public,
        realm_schema::access_acl,
        player_schema::name,
      ))
      .filter(public_filter)
      .filter(scope.as_expression());

    match player_secret_id.as_ref() {
      Some(player_secret_id) => {
        use crate::database::schema::realmcalendarsubscription::dsl as calendar_schema;
        diesel::alias!(schema::player as realm_owner: RealmOwnerSchema);
        query
          .union(
            realm_announcement_schema::realmannouncement
              .inner_join(calendar_schema::realmcalendarsubscription.on(calendar_schema::realm.eq(realm_announcement_schema::realm)))
              .inner_join(player_schema::player.on(player_schema::id.eq(calendar_schema::player)))
              .inner_join(realm_schema::realm.on(realm_schema::id.eq(calendar_schema::realm)))
              .inner_join(realm_owner.on(realm_owner.field(player_schema::id).eq(realm_schema::owner)))
              .filter(player_schema::calendar_id.eq(player_secret_id))
              .select((
                realm_owner.field(player_schema::name),
                realm_schema::asset,
                realm_announcement_schema::id,
                realm_announcement_schema::title,
                realm_announcement_schema::body,
                realm_announcement_schema::when,
                realm_announcement_schema::public,
                realm_schema::owner.eq(player_schema::id),
                realm_schema::access_acl,
                player_schema::name,
              )),
          )
          .load_iter::<AnnouncementTuple, diesel::connection::DefaultLoadingMode>(&mut db_connection)
      }
      None => query.load_iter::<AnnouncementTuple, diesel::connection::DefaultLoadingMode>(&mut db_connection),
    }?
    .filter_map(|result| match result {
      Ok((owner, asset, _, title, body, when, public, visible, access, real_user_name)) => {
        if visible
          || access.0.check(&spadina_core::player::PlayerIdentifier::Local(real_user_name.as_str()), local_server)
            == spadina_core::access::SimpleAccess::Allow
        {
          Some(Ok((
            spadina_core::realm::LocalRealmTarget { owner, asset },
            spadina_core::realm::RealmAnnouncement { title, body, public, when: when.0 },
          )))
        } else {
          None
        }
      }
      Err(e) => Some(Err(e)),
    })
    .collect()
  }
  pub fn realm_chat_write(
    &self,
    db_id: i32,
    sender: &spadina_core::player::PlayerIdentifier<impl AsRef<str> + serde::Serialize + std::fmt::Debug>,
    body: &spadina_core::communication::MessageBody<impl AsRef<str> + serde::Serialize + std::fmt::Debug>,
    timestamp: &chrono::DateTime<chrono::Utc>,
  ) -> QueryResult<()> {
    use schema::realmchat::dsl as realm_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(realm_chat_schema::realmchat)
      .values((
        realm_chat_schema::body.eq(diesel_json::Json(body)),
        realm_chat_schema::principal.eq(diesel_json::Json(sender)),
        realm_chat_schema::created.eq(&timestamp),
        realm_chat_schema::realm.eq(db_id),
      ))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn realm_create(&self, asset: &str, owner: &str, name: &str, seed: i32, train: Option<u16>) -> diesel::QueryResult<i32> {
    let mut db_connection = self.0.get().unwrap();
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    diesel::insert_into(realm_schema::realm)
      .values((
        realm_schema::name.eq(name),
        realm_schema::owner
          .eq(sql_not_null_int(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(owner)).single_value())),
        realm_schema::asset.eq(asset),
        realm_schema::seed.eq(seed),
        realm_schema::access_acl.eq(sql_not_null_json(
          player_schema::player.select(player_schema::new_realm_access_acl).filter(player_schema::name.eq(&owner)).single_value(),
        )),
        realm_schema::admin_acl.eq(sql_not_null_json(
          player_schema::player.select(player_schema::new_realm_access_acl).filter(player_schema::name.eq(&owner)).single_value(),
        )),
        realm_schema::in_directory.eq(false),
        realm_schema::train.eq(train.map(|t| t as i32)),
      ))
      .returning(realm_schema::id)
      .on_conflict((realm_schema::owner, realm_schema::asset))
      .do_update()
      .set(realm_schema::updated_at.eq(chrono::Utc::now()))
      .get_result::<i32>(&mut db_connection)
  }
  pub fn realm_delete(&self, db_id: i32) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use schema::realm::dsl as realm_schema;
      use schema::realmannouncement::dsl as announcement_schema;
      use schema::realmcalendarsubscription::dsl as calendar_schema;
      use schema::realmchat::dsl as chat_schema;
      diesel::delete(announcement_schema::realmannouncement.filter(announcement_schema::realm.eq(db_id))).execute(db_connection)?;
      diesel::delete(calendar_schema::realmcalendarsubscription.filter(calendar_schema::realm.eq(db_id))).execute(db_connection)?;
      diesel::delete(chat_schema::realmchat.filter(chat_schema::realm.eq(db_id))).execute(db_connection)?;
      diesel::delete(
        chat_schema::realmchat
          .filter(chat_schema::realm.nullable().eq(realm_schema::realm.select(realm_schema::id).filter(realm_schema::id.eq(db_id)).single_value())),
      )
      .execute(db_connection)?;
      diesel::delete(realm_schema::realm.filter(realm_schema::id.eq(db_id))).execute(db_connection)?;
      Ok(())
    })
  }
  pub(crate) fn realm_find(&self, scope: realm_scope::RealmScope<impl AsRef<str>>) -> QueryResult<Option<(i32, String)>> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;

    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .inner_join(player_schema::player)
      .select((realm_schema::id, realm_schema::asset))
      .filter(scope.as_expression())
      .order_by(realm_schema::updated_at.desc())
      .get_result::<(i32, String)>(&mut db_connection)
      .optional()
  }
  pub(crate) fn realm_list(
    &self,
    server_name: &std::sync::Arc<str>,
    include_train: bool,
    predicate: realm_scope::RealmListScope<impl AsRef<str>>,
  ) -> Vec<spadina_core::realm::RealmDirectoryEntry<std::sync::Arc<str>>> {
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    let result = realm_schema::realm
      .inner_join(player_schema::player)
      .select((realm_schema::asset, player_schema::name, realm_schema::name, realm_schema::train))
      .filter(predicate.as_expression())
      .load_iter::<(String, String, String, Option<i32>), diesel::connection::DefaultLoadingMode>(&mut db_connection)
      .and_then(|entries| {
        entries
          .map(|result| {
            result.map(|(asset, owner, name, train)| spadina_core::realm::RealmDirectoryEntry {
              asset: asset.into(),
              owner: owner.into(),
              name: name.into(),
              activity: spadina_core::realm::RealmActivity::Unknown,
              server: server_name.clone(),
              train: if include_train {
                match train {
                  Some(train) => u16::try_from(train).ok(),
                  None => None,
                }
              } else {
                None
              },
            })
          })
          .collect()
      });
    match result {
      Ok(entries) => entries,
      Err(e) => {
        eprintln!("Failed to get realms from DB: {}", e);
        vec![]
      }
    }
  }
  pub fn realm_load(&self, db_id: i32) -> QueryResult<RealmLoadInfo> {
    use schema::realm::dsl as realm_schema;

    let mut db_connection = self.0.get().unwrap();

    realm_schema::realm
      .select((realm_schema::state, realm_schema::seed, realm_schema::solved, realm_schema::train))
      .filter(realm_schema::id.eq(db_id))
      .first::<(Option<diesel_json::Json<Vec<serde_json::Value>>>, i32, bool, Option<i32>)>(&mut db_connection)
      .map(|(state, seed, solved, train)| RealmLoadInfo {
        state: state.map(|s| s.0),
        seed,
        solved,
        train: train.map(|v| u16::try_from(v).ok()).flatten(),
      })
  }
  pub fn realm_messages(
    &self,
    db_id: i32,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) -> QueryResult<Vec<spadina_core::location::LocationMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::realmchat::dsl as realmchat_schema;
    realmchat_schema::realmchat
      .select((realmchat_schema::principal, realmchat_schema::created, realmchat_schema::body))
      .filter(realmchat_schema::realm.eq(db_id).and(realmchat_schema::created.ge(&from)).and(realmchat_schema::created.lt(&to)))
      .load_iter::<(
        diesel_json::Json<spadina_core::player::PlayerIdentifier<String>>,
        chrono::DateTime<chrono::Utc>,
        diesel_json::Json<spadina_core::communication::MessageBody<String>>,
      ), diesel::connection::DefaultLoadingMode>(&mut db_connection)?
      .map(|r| r.map(|(sender, timestamp, body)| spadina_core::location::LocationMessage { body: body.0, sender: sender.0, timestamp }))
      .collect()
  }
  pub fn realm_settings_read(&self, db_id: i32) -> QueryResult<crate::realm::RealmSettings> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .select(realm_schema::settings)
      .filter(realm_schema::id.eq(db_id))
      .first::<diesel_json::Json<crate::realm::RealmSettings>>(&mut db_connection)
      .map(|j| j.0)
  }
  pub fn realm_settings_write(&self, db_id: i32, settings: &crate::realm::RealmSettings) -> QueryResult<()> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id)))
      .set(realm_schema::settings.eq(diesel_json::Json(&settings)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_push_state(&self, db_id: i32, state: Vec<serde_json::Value>, solved: bool) -> QueryResult<()> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id)))
      .set((realm_schema::state.eq(diesel_json::Json(state)), realm_schema::solved.eq(solved)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_name_read(&self, db_id: i32) -> QueryResult<(String, bool)> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    realm_schema::realm
      .select((realm_schema::name, realm_schema::in_directory))
      .filter(realm_schema::id.eq(db_id))
      .first::<(String, bool)>(&mut db_connection)
  }
  pub fn realm_name_write(&self, db_id: i32, name: &str, in_directory: bool) -> QueryResult<()> {
    use schema::realm::dsl as realm_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(realm_schema::realm.filter(realm_schema::id.eq(db_id)))
      .set((realm_schema::name.eq(name), realm_schema::in_directory.eq(in_directory)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn realm_next_train_asset(&self, owner: &str, train: u16) -> QueryResult<Option<String>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::player::dsl as player_schema;
    use schema::realm::dsl as realm_schema;
    use schema::realmtrain::dsl as realmtrain_schema;
    let first: Box<dyn diesel::expression::BoxableExpression<_, _, SqlType = diesel::sql_types::Bool>> = if train == 0 {
      Box::new(realmtrain_schema::allowed_first)
    } else {
      Box::new(<bool as diesel::expression::AsExpression<diesel::sql_types::Bool>>::as_expression(true))
    };
    let mut results: Vec<String> = realmtrain_schema::realmtrain
      .select(realmtrain_schema::asset)
      .filter(first.and(realmtrain_schema::asset.ne_all(realm_schema::realm.select(realm_schema::asset).filter(
        realm_schema::owner.nullable().eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(&owner)).single_value()),
      ))))
      .load(&mut db_connection)?;
    results.sort_by_key(|a| {
      use std::hash::Hash;
      let mut hasher = std::collections::hash_map::DefaultHasher::new();
      a.hash(&mut hasher);
      owner.hash(&mut hasher);
      hasher.finish()
    });
    Ok(results.into_iter().next())
  }
  pub fn remote_direct_message_get(
    &self,
    db_id: i32,
    remote_player: &str,
    remote_server: &str,
  ) -> QueryResult<Vec<spadina_core::communication::DirectMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::remoteplayerchat::dsl as chat_schema;
    chat_schema::remoteplayerchat
      .select((chat_schema::body, chat_schema::created, chat_schema::inbound))
      .filter(chat_schema::player.eq(db_id).and(chat_schema::remote_player.eq(remote_player)).and(chat_schema::remote_server.eq(remote_server)))
      .load_iter::<(diesel_json::Json<spadina_core::communication::MessageBody<String>>, chrono::DateTime<chrono::Utc>, bool), diesel::connection::DefaultLoadingMode >(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(spadina_core::communication::DirectMessage {body:body.0, timestamp, inbound} )
      })
      .collect()
  }

  pub fn remote_direct_message_write(
    &self,
    db_id: i32,
    remote_player: &str,
    remote_server: &str,
    body: &spadina_core::communication::MessageBody<impl AsRef<str> + std::fmt::Debug + serde::Serialize>,
    inbound: Option<chrono::DateTime<chrono::Utc>>,
  ) -> QueryResult<chrono::DateTime<chrono::Utc>> {
    let (inbound, timestamp) = match inbound {
      Some(timestamp) => (true, timestamp),
      None => (false, chrono::Utc::now()),
    };
    use schema::remoteplayerchat::dsl as remoteplayerchat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(remoteplayerchat_schema::remoteplayerchat)
      .values(&(
        remoteplayerchat_schema::player.eq(db_id),
        remoteplayerchat_schema::remote_player.eq(remote_player),
        remoteplayerchat_schema::remote_server.eq(remote_server),
        remoteplayerchat_schema::body.eq(diesel_json::Json(&body)),
        remoteplayerchat_schema::created.eq(&timestamp),
        remoteplayerchat_schema::inbound.eq(inbound),
      ))
      .execute(&mut db_connection)?;
    Ok(timestamp)
  }
  pub fn remote_direct_message_last_read_set(&self, db_id: i32, player: &str, server: &str) -> QueryResult<chrono::DateTime<chrono::Utc>> {
    use schema::remoteplayerlastread::dsl as remote_player_last_read_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = chrono::Utc::now();
    diesel::insert_into(remote_player_last_read_schema::remoteplayerlastread)
      .values((
        remote_player_last_read_schema::player.eq(db_id),
        remote_player_last_read_schema::remote_player.eq(player),
        remote_player_last_read_schema::remote_server.eq(server),
        remote_player_last_read_schema::when.eq(now),
      ))
      .on_conflict((
        remote_player_last_read_schema::player,
        remote_player_last_read_schema::remote_player,
        remote_player_last_read_schema::remote_server,
      ))
      .do_update()
      .set(remote_player_last_read_schema::when.eq(now))
      .execute(&mut db_connection)?;
    Ok(now)
  }
  pub fn train_add(&self, asset: &str, allowed_first: bool) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    use schema::realmtrain::dsl as realmtrain_schema;
    diesel::insert_into(realmtrain_schema::realmtrain)
      .values((realmtrain_schema::asset.eq(asset), realmtrain_schema::allowed_first.eq(allowed_first)))
      .on_conflict(realmtrain_schema::asset)
      .do_update()
      .set(realmtrain_schema::allowed_first.eq(allowed_first))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub(crate) fn player_avatar_read(&self, db_id: i32) -> diesel::QueryResult<spadina_core::avatar::Avatar> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      player_schema::player
        .select(player_schema::avatar)
        .filter(player_schema::id.eq(db_id))
        .get_result::<diesel_json::Json<spadina_core::avatar::Avatar>>(&mut db_connection)
        .optional()?
        .map(|v| v.0)
        .unwrap_or_default(),
    )
  }
  pub(crate) fn player_avatar_write(&self, db_id: i32, avatar: &spadina_core::avatar::Avatar) -> diesel::QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id)))
      .set(player_schema::avatar.eq(diesel_json::Json(avatar)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn player_acl<T: PlayerAccess>(&self, id: i32, column: T) -> QueryResult<crate::access::AccessSetting<T::Verb>> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      player_schema::player
        .select(column)
        .filter(player_schema::id.eq(id))
        .get_result::<diesel_json::Json<crate::access::AccessSetting<T::Verb>>>(&mut db_connection)
        .optional()?
        .map(|j| j.0)
        .unwrap_or_default(),
    )
  }

  pub fn player_acl_write<T: PlayerAccess>(&self, id: i32, column: T, access_control: &crate::access::AccessSetting<T::Verb>) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(id)))
      .set(column.eq(diesel_json::Json(access_control)))
      .execute(&mut db_connection)?;
    Ok(())
  }
}
pub trait PlayerAccess:
  diesel::Column<Table = schema::player::dsl::player, SqlType = diesel::sql_types::Jsonb>
  + diesel::SelectableExpression<schema::player::table>
  + diesel::expression::ValidGrouping<()>
  + diesel::query_builder::QueryId
  + diesel::query_builder::QueryFragment<diesel::pg::Pg>
{
  type Verb: serde::de::DeserializeOwned + serde::Serialize + Copy + Default + std::fmt::Debug + 'static;
}
impl PlayerAccess for crate::database::schema::player::dsl::message_acl {
  type Verb = spadina_core::access::SimpleAccess;
}
impl PlayerAccess for crate::database::schema::player::dsl::new_realm_access_acl {
  type Verb = spadina_core::access::SimpleAccess;
}
impl PlayerAccess for crate::database::schema::player::dsl::new_realm_admin_acl {
  type Verb = spadina_core::access::SimpleAccess;
}

impl PlayerAccess for crate::database::schema::player::dsl::online_acl {
  type Verb = spadina_core::access::LocationAccess;
}
