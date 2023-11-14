pub mod avatar;
pub mod database_location;
pub mod database_location_directory;
pub mod location_persistence;
pub mod location_scope;
pub mod persisted;
pub mod player_access;
pub mod player_persistence;
pub mod player_reference;
pub mod schema;
pub mod setting;

use chrono::{DateTime, Utc};
use diesel::connection::DefaultLoadingMode;
use std::fmt::Debug;
use std::sync::Arc;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use futures::Stream;
use futures::StreamExt;
use player_access::PlayerAccess;
use player_reference::PlayerReference;
use serde::Serialize;
use spadina_core::access::AccessSetting;
use spadina_core::avatar::Avatar;
use spadina_core::location::communication::ChatMessage;
use spadina_core::location::directory::{Activity, DirectoryEntry, Visibility};
use spadina_core::location::target::UnresolvedTarget;
use spadina_core::location::Descriptor;
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::AsReference;
use spadina_core::{access, communication};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Clone)]
pub(crate) struct Database(DbPool, broadcast::Sender<i32>);
pub type DbPool = Pool<ConnectionManager<PgConnection>>;

sql_function! { #[sql_name = "gen_calendar_id"]fn sql_gen_calendar_id() -> Binary}

pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

impl Database {
  pub fn new(db_url: &str) -> Self {
    let pool = {
      let manager = ConnectionManager::<PgConnection>::new(db_url);
      Pool::builder().build(manager).expect("Failed to create pool.")
    };
    use diesel_migrations::MigrationHarness;
    let mut db_connection = pool.get().expect("Failed to connect to database");
    db_connection.run_pending_migrations(MIGRATIONS).expect("Failed to migrate database to latest schema");
    let (tx, _) = broadcast::channel(200);
    Database(pool, tx)
  }
  pub fn announcements_read(&self) -> QueryResult<Vec<communication::Announcement<Arc<str>>>> {
    use diesel::prelude::*;
    use schema::announcement::dsl as announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    announcement_schema::announcement
      .select((
        announcement_schema::title,
        announcement_schema::body,
        announcement_schema::when,
        announcement_schema::location,
        announcement_schema::public,
      ))
      .load_iter::<(
        String,
        String,
        diesel_json::Json<communication::AnnouncementTime>,
        diesel_json::Json<UnresolvedTarget<Arc<str>>>,
        bool,
      ), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| {
        r.map(|(title, body, when, location, public)| communication::Announcement {
          title: Arc::from(title),
          body: Arc::from(body),
          when: when.0,
          location: location.0,
          public,
        })
      })
      .collect()
  }
  pub fn announcements_write(&self, announcements: &[communication::Announcement<impl AsRef<str> + Debug + serde::Serialize>]) -> QueryResult<()> {
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
            announcement_schema::location.eq(diesel_json::Json(&a.location)),
            announcement_schema::public.eq(a.public),
          ))
          .execute(db_connection)?;
      }
      Ok(())
    })
  }
  pub fn bookmark_add(
    &self,
    db_id: i32,
    bookmark: &spadina_core::resource::Resource<impl AsRef<str> + serde::Serialize + Debug>,
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
    filter: impl Fn(spadina_core::resource::Resource<String>) -> Option<R>,
  ) -> QueryResult<C> {
    let mut db_connection = self.0.get().unwrap();
    use schema::bookmark::dsl as bookmark_schema;
    bookmark_schema::bookmark
      .select(bookmark_schema::value)
      .filter(bookmark_schema::player.eq(db_id))
      .load_iter::<diesel_json::Json<spadina_core::resource::Resource<String>>, DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|bookmark| match bookmark {
        Ok(bookmark) => match filter(bookmark.0) {
          Some(bookmark) => Some(Ok(bookmark)),
          None => None,
        },
        Err(e) => Some(Err(e)),
      })
      .collect()
  }
  pub fn bookmark_rm(&self, db_id: i32, bookmark: &spadina_core::resource::Resource<impl AsRef<str> + serde::Serialize + Debug>) -> QueryResult<()> {
    use schema::bookmark::dsl as bookmark_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(bookmark_schema::bookmark.filter(bookmark_schema::player.eq(db_id).and(bookmark_schema::value.eq(&diesel_json::Json(bookmark)))))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_reset(&self, player: PlayerReference<impl AsRef<str>>) -> QueryResult<Vec<u8>> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player.as_expression()))
      .set(player_schema::calendar_id.eq(sql_gen_calendar_id()))
      .returning(player_schema::calendar_id)
      .get_result(&mut db_connection)
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
  pub fn calendar_list(&self, db_id: i32, local_server: &Arc<str>) -> QueryResult<Vec<DirectoryEntry<Arc<str>>>> {
    use schema::location::dsl as location_schema;
    use schema::locationcalendarsubscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    calendar_schema::locationcalendarsubscription
      .inner_join(location_schema::location.on(calendar_schema::location.eq(location_schema::id)))
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select((
        location_schema::descriptor,
        player_schema::name,
        location_schema::name,
        location_schema::updated_at,
        location_schema::created,
        location_schema::visibility,
      ))
      .filter(calendar_schema::player.eq(db_id))
      .load_iter::<(diesel_json::Json<Descriptor<Arc<str>>>, String, String, DateTime<Utc>, DateTime<Utc>, i16), DefaultLoadingMode>(
        &mut db_connection,
      )?
      .map(|r| match r {
        Ok((descriptor, owner, name, updated, created, visibility)) => Ok(DirectoryEntry {
          activity: Activity::Unknown,
          descriptor: descriptor.0,
          name: Arc::from(name),
          owner: Arc::from(owner),
          server: local_server.clone(),
          updated,
          created,
          visibility: Visibility::try_from(visibility).unwrap_or(Visibility::Archived),
        }),
        Err(e) => Err(e),
      })
      .collect()
  }
  pub fn calendar_rm(&self, db_id: i32, target: &spadina_core::location::target::LocalTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::locationcalendarsubscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(
      calendar_schema::locationcalendarsubscription.filter(
        calendar_schema::player.eq(db_id).and(
          calendar_schema::location.eq_any(
            location_schema::location
              .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
              .select(location_schema::id)
              .filter(
                location_schema::descriptor
                  .eq(diesel_json::Json(target.descriptor.reference(AsReference::<str>::default())))
                  .and(player_schema::name.eq(target.owner.as_ref())),
              ),
          ),
        ),
      ),
    )
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_add(&self, db_id: i32, location: &spadina_core::location::target::LocalTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::locationcalendarsubscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select((<i32 as diesel::expression::AsExpression<diesel::sql_types::Integer>>::as_expression(db_id), location_schema::id))
      .filter(
        location_schema::descriptor
          .eq(diesel_json::Json(location.descriptor.reference(AsReference::<str>::default())))
          .and(player_schema::name.eq(location.owner.as_ref())),
      )
      .insert_into(calendar_schema::locationcalendarsubscription)
      .into_columns((calendar_schema::player, calendar_schema::location))
      .on_conflict((calendar_schema::player, calendar_schema::location))
      .do_nothing()
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_rm_all(&self, db_id: i32) -> QueryResult<()> {
    use schema::locationcalendarsubscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(calendar_schema::locationcalendarsubscription.filter(calendar_schema::player.eq(db_id))).execute(&mut db_connection)?;
    Ok(())
  }
  pub fn direct_message_clean(&self) -> QueryResult<()> {
    use schema::localplayerchat::dsl as local_player_chat_schema;
    use schema::locationchat::dsl as location_chat_schema;
    use schema::remoteplayerchat::dsl as remote_player_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    let horizon = Utc::now() - chrono::Duration::days(30);
    diesel::delete(location_chat_schema::locationchat.filter(location_chat_schema::created.le(&horizon))).execute(&mut db_connection)?;
    diesel::delete(local_player_chat_schema::localplayerchat.filter(local_player_chat_schema::created.le(&horizon))).execute(&mut db_connection)?;
    diesel::delete(remote_player_chat_schema::remoteplayerchat.filter(remote_player_chat_schema::created.le(&horizon)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn direct_message_get(
    &self,
    db_id: i32,
    name: &str,
    from: &DateTime<Utc>,
    to: &DateTime<Utc>,
  ) -> QueryResult<Vec<communication::DirectMessage<String>>> {
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
      .load_iter::<(diesel_json::Json<communication::MessageBody<String>>, DateTime<Utc>, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(communication::DirectMessage { inbound, body: body.0, timestamp }),
      })
      .collect()
  }
  pub fn direct_message_write(
    &self,
    sender: &str,
    recipient: &str,
    body: &communication::MessageBody<impl AsRef<str> + serde::Serialize + Debug>,
  ) -> QueryResult<DateTime<Utc>> {
    use schema::localplayerchat::dsl as chat_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let recipient_id = player_schema::player.select(player_schema::id).filter(player_schema::name.eq(recipient)).first::<i32>(&mut db_connection)?;
    let timestamp = Utc::now();
    if body.is_transient() {
      diesel::insert_into(schema::localplayerlastread::dsl::localplayerlastread)
        .values((
          schema::localplayerlastread::dsl::recipient.eq(recipient_id),
          schema::localplayerlastread::dsl::sender
            .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value().assume_not_null()),
          schema::localplayerlastread::dsl::when.eq(timestamp),
        ))
        .on_conflict((schema::localplayerlastread::dsl::recipient, schema::localplayerlastread::dsl::sender))
        .do_update()
        .set(schema::localplayerlastread::dsl::when.eq(timestamp))
        .execute(&mut db_connection)?;
    } else {
      diesel::insert_into(chat_schema::localplayerchat)
        .values(&(
          chat_schema::recipient.eq(recipient_id),
          chat_schema::body.eq(diesel_json::Json(body)),
          chat_schema::created.eq(&timestamp),
          chat_schema::sender
            .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value().assume_not_null()),
        ))
        .execute(&mut db_connection)?;
    }
    Ok(timestamp)
  }
  pub fn direct_message_stats(
    &self,
    db_id: i32,
  ) -> QueryResult<(std::collections::HashMap<PlayerIdentifier<String>, communication::DirectMessageInfo>, DateTime<Utc>)> {
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
      .load_iter::<(String, Option<DateTime<Utc>>, Option<DateTime<Utc>>), DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|r| match r {
        Ok((player, Some(last_received), last_read)) => {
          Some(Ok((PlayerIdentifier::Local(player), communication::DirectMessageInfo { last_received, last_read })))
        }
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
      .load_iter::<(String, String, Option<DateTime<Utc>>, Option<DateTime<Utc>>), DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|r| match r {
        Ok((player, server, Some(last_received), last_read)) => {
          Some(Ok((PlayerIdentifier::Remote { server, player }, communication::DirectMessageInfo { last_received, last_read })))
        }
        Ok(_) => None,
        Err(e) => Some(Err(e)),
      })
    {
      let (player, info) = item?;
      stats.insert(player, info);
    }
    let last_login =
      player_schema::player.select(player_schema::last_login).filter(player_schema::id.eq(db_id)).first::<DateTime<Utc>>(&mut db_connection)?;
    Ok((stats, last_login))
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
      use schema::location::dsl as location_schema;
      use schema::locationcalendarsubscription::dsl as locationcalendarsubscription_schema;
      use schema::locationchat::dsl as locationchat_schema;
      use schema::player::dsl as player_schema;
      use schema::publickey::dsl as publickey_schema;
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
        locationcalendarsubscription_schema::locationcalendarsubscription
          .filter(locationcalendarsubscription_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        remoteplayerchat_schema::remoteplayerchat
          .filter(remoteplayerchat_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        locationchat_schema::locationchat.filter(
          locationchat_schema::location.eq_any(
            location_schema::location
              .select(location_schema::id)
              .filter(location_schema::owner.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
          ),
        ),
      )
      .execute(db_connection)?;
      diesel::delete(
        location_schema::location.filter(location_schema::owner.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
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
  pub fn player_load(&self, player_name: &str) -> QueryResult<(i32, Vec<u8>)> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = Utc::now();
    diesel::insert_into(player_schema::player)
      .values((
        player_schema::name.eq(player_name),
        player_schema::avatar.eq(diesel_json::Json(Avatar::default_for(player_name))),
        player_schema::message_acl.eq(diesel_json::Json(AccessSetting::<&str, _> { default: access::SimpleAccess::Allow, rules: Vec::new() })),
        player_schema::online_acl.eq(diesel_json::Json(AccessSetting::<&str, access::OnlineAccess>::default())),
        player_schema::default_location_acl.eq(diesel_json::Json(AccessSetting::<&str, access::Privilege>::default())),
        player_schema::last_login.eq(now),
      ))
      .on_conflict(player_schema::name)
      .do_update()
      .set(player_schema::last_login.eq(now))
      .returning((player_schema::id, player_schema::calendar_id))
      .get_result(&mut db_connection)
  }
  pub fn public_key_add(&self, db_id: i32, der: &[u8]) -> QueryResult<usize> {
    use schema::publickey::dsl as publickey_dsl;
    let fingerprint = spadina_core::net::server::auth::compute_fingerprint(der);
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(publickey_dsl::publickey)
      .values((publickey_dsl::player.eq(db_id), publickey_dsl::fingerprint.eq(fingerprint), publickey_dsl::public_key.eq(der)))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn public_key_get(&self, player_name: &str, fingerprint: &str) -> QueryResult<Option<Vec<u8>>> {
    use schema::player::dsl as player_dsl;
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(
      publickey_dsl::publickey.filter(
        publickey_dsl::player
          .eq(player_dsl::player.select(player_dsl::id).filter(player_dsl::name.eq(player_name)).single_value().assume_not_null())
          .and(publickey_dsl::fingerprint.eq(fingerprint)),
      ),
    )
    .set(publickey_dsl::last_used.eq(Utc::now()))
    .returning(publickey_dsl::public_key)
    .get_result(&mut db_connection)
    .optional()
  }
  pub fn public_key_list(&self, db_id: i32) -> QueryResult<Vec<spadina_core::net::server::auth::PublicKey<String>>> {
    use schema::publickey::dsl as publickey_dsl;
    let mut db_connection = self.0.get().unwrap();
    publickey_dsl::publickey
      .select((publickey_dsl::fingerprint, publickey_dsl::created, publickey_dsl::last_used))
      .filter(publickey_dsl::player.eq(&db_id))
      .load_iter::<(String, DateTime<Utc>, Option<DateTime<Utc>>), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| match r {
        Ok((fingerprint, created, last_used)) => Ok(spadina_core::net::server::auth::PublicKey { fingerprint, created, last_used }),
        Err(e) => Err(e),
      })
      .collect()
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
  pub fn location_acl_read(&self, id: i32) -> QueryResult<AccessSetting<Arc<str>, access::Privilege>> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .select(location_schema::acl)
      .filter(location_schema::id.eq(id))
      .first::<diesel_json::Json<AccessSetting<Arc<str>, access::Privilege>>>(&mut db_connection)
      .map(|a| a.0)
  }
  pub fn location_acl_write(
    &self,
    id: i32,
    access_control: &AccessSetting<impl AsRef<str> + Serialize + Debug, access::Privilege>,
  ) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(location_schema::location.filter(location_schema::id.eq(id)))
      .set(location_schema::acl.eq(diesel_json::Json(access_control)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub(crate) fn location_acl_write_bulk(
    &self,
    scope: location_scope::LocationListScope<impl AsRef<str> + Debug>,
    acl: &AccessSetting<impl AsRef<str> + Serialize + Debug, access::Privilege>,
  ) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;

    let mut db_connection = self.0.get().unwrap();
    let locations: Vec<_> = location_schema::location
      .inner_join(player_schema::player)
      .select(location_schema::id)
      .filter(scope.as_expression())
      .load::<i32>(&mut db_connection)?;

    diesel::update(location_schema::location.filter(location_schema::id.eq_any(locations)))
      .set(location_schema::acl.eq(diesel_json::Json(acl)))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn location_announcements_write(
    &self,
    id: i32,
    announcements: &[spadina_core::location::communication::Announcement<impl AsRef<str>>],
  ) -> QueryResult<()> {
    use diesel::prelude::*;
    use schema::locationannouncement::dsl as location_announcement_schema;
    self.0.get().unwrap().transaction::<(), diesel::result::Error, _>(|db_connection| {
      diesel::delete(location_announcement_schema::locationannouncement.filter(location_announcement_schema::location.eq(id)))
        .execute(db_connection)?;
      let rows: Vec<_> = announcements
        .iter()
        .map(|a| {
          (
            location_announcement_schema::title.eq(a.title.as_ref()),
            location_announcement_schema::body.eq(a.body.as_ref()),
            location_announcement_schema::when.eq(diesel_json::Json(&a.when)),
            location_announcement_schema::public.eq(a.public),
          )
        })
        .collect();
      diesel::insert_into(location_announcement_schema::locationannouncement).values(&rows).execute(db_connection)?;

      Ok(())
    })
  }
  pub fn location_announcements_clean(&self) -> QueryResult<()> {
    use diesel::prelude::*;
    use schema::locationannouncement::dsl as location_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = Utc::now();
    db_connection.transaction(|db_connection| {
      let ids = location_announcement_schema::locationannouncement
        .select((location_announcement_schema::id, location_announcement_schema::when))
        .load_iter::<(i32, diesel_json::Json<communication::AnnouncementTime>), DefaultLoadingMode>(db_connection)?
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
      diesel::delete(location_announcement_schema::locationannouncement.filter(location_announcement_schema::id.eq_any(ids)))
        .execute(db_connection)?;
      Ok(())
    })
  }
  pub fn location_announcements_read(&self, id: i32) -> QueryResult<Vec<spadina_core::location::communication::Announcement<String>>> {
    use schema::locationannouncement::dsl as location_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    location_announcement_schema::locationannouncement
      .select((
        location_announcement_schema::id,
        location_announcement_schema::title,
        location_announcement_schema::body,
        location_announcement_schema::when,
        location_announcement_schema::public,
      ))
      .filter(location_announcement_schema::location.eq(id))
      .load_iter::<(i32, String, String, diesel_json::Json<communication::AnnouncementTime>, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| {
        result.map(|(_, title, body, when, public)| spadina_core::location::communication::Announcement { title, body, public, when: when.0 })
      })
      .collect()
  }
  pub fn location_announcements_fetch_all(
    &self,
    scope: location_scope::LocationListScope<impl AsRef<str> + Debug>,
    player_secret_id: Option<Vec<u8>>,
    local_server: &str,
  ) -> QueryResult<Vec<(spadina_core::location::target::LocalTarget<String>, spadina_core::location::communication::Announcement<String>)>> {
    use diesel::expression::AsExpression;
    use schema::location::dsl as location_schema;
    use schema::locationannouncement::dsl as location_announcement_schema;
    use schema::player::dsl as player_schema;
    type AnnouncementTuple = (
      String,
      diesel_json::Json<Descriptor<String>>,
      i32,
      String,
      String,
      diesel_json::Json<communication::AnnouncementTime>,
      bool,
      bool,
      diesel_json::Json<AccessSetting<String, access::SimpleAccess>>,
      String,
    );
    let mut db_connection = self.0.get().unwrap();
    let public_filter: Box<dyn BoxableExpression<_, diesel::pg::Pg, SqlType = diesel::sql_types::Bool>> = match player_secret_id.as_ref() {
      None => Box::new(location_announcement_schema::public.as_expression()),
      Some(calendar_id) => Box::new(player_schema::calendar_id.eq(calendar_id).or(location_announcement_schema::public).as_expression()),
    };
    let query = location_announcement_schema::locationannouncement
      .inner_join(location_schema::location.on(location_schema::id.eq(location_announcement_schema::location)))
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select((
        player_schema::name,
        location_schema::descriptor,
        location_announcement_schema::id,
        location_announcement_schema::title,
        location_announcement_schema::body,
        location_announcement_schema::when,
        location_announcement_schema::public,
        location_announcement_schema::public,
        location_schema::acl,
        player_schema::name,
      ))
      .filter(public_filter)
      .filter(scope.as_expression());

    match player_secret_id.as_ref() {
      Some(player_secret_id) => {
        use crate::database::schema::locationcalendarsubscription::dsl as calendar_schema;
        diesel::alias!(schema::player as location_owner: RealmOwnerSchema);
        query
          .union(
            location_announcement_schema::locationannouncement
              .inner_join(calendar_schema::locationcalendarsubscription.on(calendar_schema::location.eq(location_announcement_schema::location)))
              .inner_join(player_schema::player.on(player_schema::id.eq(calendar_schema::player)))
              .inner_join(location_schema::location.on(location_schema::id.eq(calendar_schema::location)))
              .inner_join(location_owner.on(location_owner.field(player_schema::id).eq(location_schema::owner)))
              .filter(player_schema::calendar_id.eq(player_secret_id))
              .select((
                location_owner.field(player_schema::name),
                location_schema::descriptor,
                location_announcement_schema::id,
                location_announcement_schema::title,
                location_announcement_schema::body,
                location_announcement_schema::when,
                location_announcement_schema::public,
                location_schema::owner.eq(player_schema::id),
                location_schema::acl,
                player_schema::name,
              )),
          )
          .load_iter::<AnnouncementTuple, DefaultLoadingMode>(&mut db_connection)
      }
      None => query.load_iter::<AnnouncementTuple, DefaultLoadingMode>(&mut db_connection),
    }?
    .filter_map(|result| match result {
      Ok((owner, descriptor, _, title, body, when, public, visible, access, real_user_name)) => {
        if visible || access.0.check(&PlayerIdentifier::Local(real_user_name.as_str()), local_server) == access::SimpleAccess::Allow {
          Some(Ok((
            spadina_core::location::target::LocalTarget { owner, descriptor: descriptor.0 },
            spadina_core::location::communication::Announcement { title, body, public, when: when.0 },
          )))
        } else {
          None
        }
      }
      Err(e) => Some(Err(e)),
    })
    .collect()
  }
  pub(crate) fn location_change_visibility(
    &self,
    visibility: Visibility,
    predicate: location_scope::LocationListScope<impl AsRef<str> + Debug>,
  ) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let ids = db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      let ids = location_schema::location
        .inner_join(player_schema::player)
        .select(location_schema::id)
        .filter(predicate.as_expression().and(location_schema::visibility.ne(visibility as i16)))
        .load::<i32>(db_connection)?;
      diesel::update(location_schema::location.filter(location_schema::id.eq_any(&ids)))
        .set((location_schema::visibility.eq(visibility as i16), location_schema::visibility_changed.eq(Utc::now())))
        .execute(db_connection)?;
      Ok(ids)
    })?;
    for id in ids {
      let _ = self.1.send(id);
    }
    Ok(())
  }

  pub fn location_chat_write(
    &self,
    db_id: i32,
    sender: &PlayerIdentifier<impl AsRef<str> + serde::Serialize + Debug>,
    body: &communication::MessageBody<impl AsRef<str> + serde::Serialize + Debug>,
    timestamp: &DateTime<Utc>,
  ) -> QueryResult<()> {
    use schema::locationchat::dsl as location_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(location_chat_schema::locationchat)
      .values((
        location_chat_schema::body.eq(diesel_json::Json(body)),
        location_chat_schema::principal.eq(diesel_json::Json(sender)),
        location_chat_schema::created.eq(&timestamp),
        location_chat_schema::location.eq(db_id),
      ))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn location_create(&self, descriptor: &Descriptor<&str>, owner: &str, name: &str, state: serde_json::Value) -> QueryResult<i32> {
    let mut db_connection = self.0.get().unwrap();
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;
    diesel::insert_into(location_schema::location)
      .values((
        location_schema::name.eq(name),
        location_schema::owner
          .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(owner)).single_value().assume_not_null()),
        location_schema::descriptor.eq(diesel_json::Json(descriptor)),
        location_schema::state.eq(state),
        location_schema::acl.eq(
          player_schema::player.select(player_schema::default_location_acl).filter(player_schema::name.eq(owner)).single_value().assume_not_null(),
        ),
        location_schema::visibility.eq(Visibility::Private as i16),
      ))
      .returning(location_schema::id)
      .on_conflict((location_schema::owner, location_schema::descriptor))
      .do_update()
      .set(location_schema::updated_at.eq(Utc::now()))
      .get_result::<i32>(&mut db_connection)
  }
  pub fn location_delete(&self, db_id: i32) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use schema::location::dsl as location_schema;
      use schema::locationannouncement::dsl as announcement_schema;
      use schema::locationcalendarsubscription::dsl as calendar_schema;
      use schema::locationchat::dsl as chat_schema;
      diesel::delete(announcement_schema::locationannouncement.filter(announcement_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(calendar_schema::locationcalendarsubscription.filter(calendar_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(chat_schema::locationchat.filter(chat_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(
        chat_schema::locationchat.filter(
          chat_schema::location
            .nullable()
            .eq(location_schema::location.select(location_schema::id).filter(location_schema::id.eq(db_id)).single_value()),
        ),
      )
      .execute(db_connection)?;
      diesel::delete(location_schema::location.filter(location_schema::id.eq(db_id))).execute(db_connection)?;
      Ok(())
    })
  }
  pub(crate) fn location_find(&self, scope: location_scope::LocationScope<impl AsRef<str>>) -> QueryResult<Option<i32>> {
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;

    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .inner_join(player_schema::player)
      .select(location_schema::id)
      .filter(scope.as_expression())
      .get_result::<i32>(&mut db_connection)
      .optional()
  }
  pub(crate) fn location_list(
    &self,
    server_name: &Arc<str>,
    predicate: location_scope::LocationListScope<impl AsRef<str> + Debug>,
  ) -> QueryResult<Vec<DirectoryEntry<Arc<str>>>> {
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .inner_join(player_schema::player)
      .select((
        location_schema::descriptor,
        player_schema::name,
        location_schema::name,
        location_schema::updated_at,
        location_schema::created,
        location_schema::visibility,
      ))
      .filter(predicate.as_expression())
      .load_iter::<(diesel_json::Json<Descriptor<Arc<str>>>, String, String, DateTime<Utc>, DateTime<Utc>, i16), DefaultLoadingMode>(
        &mut db_connection,
      )
      .and_then(|entries| {
        entries
          .map(|result| {
            result.map(|(descriptor, owner, name, updated, created, visibility)| DirectoryEntry {
              descriptor: descriptor.0,
              owner: owner.into(),
              name: name.into(),
              activity: Activity::Unknown,
              server: server_name.clone(),
              updated,
              created,
              visibility: Visibility::try_from(visibility).unwrap_or(Visibility::Archived),
            })
          })
          .collect()
      })
  }
  pub fn location_messages(&self, db_id: i32, from: DateTime<Utc>, to: DateTime<Utc>) -> QueryResult<Vec<ChatMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::locationchat::dsl as locationchat_schema;
    locationchat_schema::locationchat
      .select((locationchat_schema::principal, locationchat_schema::created, locationchat_schema::body))
      .filter(locationchat_schema::location.eq(db_id).and(locationchat_schema::created.ge(&from)).and(locationchat_schema::created.lt(&to)))
      .load_iter::<(
        diesel_json::Json<PlayerIdentifier<String>>,
        DateTime<Utc>,
        diesel_json::Json<communication::MessageBody<String>>,
      ), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| r.map(|(sender, timestamp, body)| ChatMessage { body: body.0, sender: sender.0, timestamp }))
      .collect()
  }
  pub fn location_state_read(&self, db_id: i32) -> QueryResult<serde_json::Value> {
    use schema::location::dsl as location_schema;

    let mut db_connection = self.0.get().unwrap();

    location_schema::location.select(location_schema::state).filter(location_schema::id.eq(db_id)).first::<serde_json::Value>(&mut db_connection)
  }

  pub fn location_state_write(&self, db_id: i32, state: serde_json::Value) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(location_schema::location.filter(location_schema::id.eq(db_id)))
      .set(location_schema::state.eq(diesel_json::Json(state)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn location_name_read(&self, db_id: i32) -> QueryResult<String> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location.select(location_schema::name).filter(location_schema::id.eq(db_id)).first::<String>(&mut db_connection)
  }
  pub fn location_name_write(&self, db_id: i32, name: &str) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(location_schema::location.filter(location_schema::id.eq(db_id)))
      .set(location_schema::name.eq(name))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn location_visibility(&self, db_id: i32) -> QueryResult<(Visibility, impl Stream<Item = Visibility> + Unpin)> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    let database = self.clone();
    Ok((
      Visibility::try_from(
        location_schema::location.select(location_schema::visibility).filter(location_schema::id.eq(db_id)).get_result::<i16>(&mut db_connection)?,
      )
      .unwrap_or(Visibility::Archived),
      BroadcastStream::new(self.1.subscribe()).filter_map(move |id| {
        let database = database.clone();
        let result = if id == Ok(db_id) {
          let mut db_connection = database.0.get().unwrap();
          location_schema::location
            .select(location_schema::visibility)
            .filter(location_schema::id.eq(db_id))
            .get_result::<i16>(&mut db_connection)
            .map(|v| Visibility::try_from(v).ok())
            .ok()
            .flatten()
        } else {
          None
        };

        std::future::ready(result)
      }),
    ))
  }
  pub fn remote_direct_message_get(
    &self,
    db_id: i32,
    remote_player: &str,
    remote_server: &str,
  ) -> QueryResult<Vec<communication::DirectMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::remoteplayerchat::dsl as chat_schema;
    chat_schema::remoteplayerchat
      .select((chat_schema::body, chat_schema::created, chat_schema::inbound))
      .filter(chat_schema::player.eq(db_id).and(chat_schema::remote_player.eq(remote_player)).and(chat_schema::remote_server.eq(remote_server)))
      .load_iter::<(diesel_json::Json<communication::MessageBody<String>>, DateTime<Utc>, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(communication::DirectMessage { body: body.0, timestamp, inbound }),
      })
      .collect()
  }

  pub fn remote_direct_message_write(
    &self,
    player: PlayerReference<impl AsRef<str>>,
    remote_player: &str,
    remote_server: &str,
    body: &communication::MessageBody<impl AsRef<str> + Debug + serde::Serialize>,
    inbound: Option<DateTime<Utc>>,
  ) -> QueryResult<DateTime<Utc>> {
    let (inbound, timestamp) = match inbound {
      Some(timestamp) => (true, timestamp),
      None => (false, Utc::now()),
    };
    use schema::remoteplayerchat::dsl as remoteplayerchat_schema;
    let mut db_connection = self.0.get().unwrap();
    if body.is_transient() {
      diesel::insert_into(schema::remoteplayerlastread::dsl::remoteplayerlastread)
        .values((
          schema::remoteplayerlastread::dsl::player.eq(player.get_id(&mut db_connection)?),
          schema::remoteplayerlastread::dsl::remote_player.eq(remote_player),
          schema::remoteplayerlastread::dsl::remote_server.eq(remote_server),
          schema::remoteplayerlastread::dsl::when.eq(timestamp),
        ))
        .on_conflict((
          schema::remoteplayerlastread::dsl::player,
          schema::remoteplayerlastread::dsl::remote_player,
          schema::remoteplayerlastread::dsl::remote_server,
        ))
        .do_update()
        .set(schema::remoteplayerlastread::dsl::when.eq(timestamp))
        .execute(&mut db_connection)?;
    } else {
      diesel::insert_into(remoteplayerchat_schema::remoteplayerchat)
        .values(&(
          remoteplayerchat_schema::player.eq(player.get_id(&mut db_connection)?),
          remoteplayerchat_schema::remote_player.eq(remote_player),
          remoteplayerchat_schema::remote_server.eq(remote_server),
          remoteplayerchat_schema::body.eq(diesel_json::Json(&body)),
          remoteplayerchat_schema::created.eq(&timestamp),
          remoteplayerchat_schema::inbound.eq(inbound),
        ))
        .execute(&mut db_connection)?;
    }
    Ok(timestamp)
  }
  pub(crate) fn player_avatar_read(&self, db_id: i32) -> QueryResult<Avatar> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      player_schema::player
        .select(player_schema::avatar)
        .filter(player_schema::id.eq(db_id))
        .get_result::<diesel_json::Json<Avatar>>(&mut db_connection)?
        .0,
    )
  }
  pub(crate) fn player_avatar_write(&self, db_id: i32, avatar: &Avatar) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id)))
      .set(player_schema::avatar.eq(diesel_json::Json(avatar)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn player_acl<T: PlayerAccess>(&self, player: PlayerReference<&str>, column: T) -> QueryResult<Option<AccessSetting<String, T::Verb>>> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      player_schema::player
        .select(column)
        .filter(player.as_expression())
        .get_result::<diesel_json::Json<AccessSetting<String, T::Verb>>>(&mut db_connection)
        .optional()?
        .map(|j| j.0),
    )
  }

  pub fn player_acl_write<T: PlayerAccess>(
    &self,
    id: i32,
    column: T,
    access_control: &AccessSetting<impl AsRef<str> + Serialize + Debug, T::Verb>,
  ) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(id)))
      .set(column.eq(diesel_json::Json(access_control)))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn setting_read<T: setting::Setting>(&self) -> QueryResult<T::Stored> {
    use schema::serversetting::dsl as serversetting_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      serversetting_schema::serversetting
        .select(serversetting_schema::data)
        .filter(serversetting_schema::category.eq(std::str::from_utf8(&[T::CODE]).unwrap()))
        .get_result::<diesel_json::Json<T::Stored>>(&mut db_connection)
        .optional()?
        .map(|j| j.0)
        .unwrap_or_default(),
    )
  }
  pub fn setting_write<T: setting::Setting>(&self, data: &T::Stored) -> QueryResult<()> {
    use schema::serversetting::dsl as serversetting_schema;
    let mut db_connection = self.0.get().unwrap();

    diesel::insert_into(serversetting_schema::serversetting)
      .values(&(serversetting_schema::category.eq(std::str::from_utf8(&[T::CODE]).unwrap()), serversetting_schema::data.eq(diesel_json::Json(data))))
      .on_conflict(serversetting_schema::category)
      .do_update()
      .set(serversetting_schema::data.eq(diesel_json::Json(data)))
      .execute(&mut db_connection)?;
    Ok(())
  }
}
