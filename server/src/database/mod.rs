pub mod connect;
pub mod database_location;
pub mod database_location_directory;
pub mod diesel_serde_jsonb;
pub mod location_persistence;
pub mod location_scope;
pub mod persisted;
pub mod player_access;
pub mod player_persistence;
pub mod player_reference;
pub mod schema;
pub mod setting;

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use diesel::connection::DefaultLoadingMode;
use diesel::dsl::{count_star, sql};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Error, Pool};
use diesel::sql_types;
use diesel::upsert::excluded;
use diesel_serde_jsonb::AsJsonb;
use futures::Stream;
use futures::StreamExt;
use icu::collator::{AlternateHandling, Collator, CollatorOptions, Strength};
use player_access::PlayerAccess;
use player_reference::PlayerReference;
use serde::Serialize;
use spadina_core::access::AccessSetting;
use spadina_core::avatar::Avatar;
use spadina_core::location::communication::{Announcement, ChatMessage};
use spadina_core::location::directory::{Activity, DirectoryEntry, Visibility};
use spadina_core::location::target::{AbsoluteTarget, LocalTarget, UnresolvedTarget};
use spadina_core::location::Descriptor;
use spadina_core::net::server::auth::PublicKey;
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::{AsReference, AsSingle};
use spadina_core::shared_ref::SharedRef;
use spadina_core::{access, communication};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Clone)]
pub(crate) struct Database(DbPool, broadcast::Sender<i32>);

pub(crate) struct StaleRemoteCalendar {
  pub(crate) player: String,
  pub(crate) server: String,
}

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type CalendarCacheEntries<S> = Vec<(LocalTarget<S>, Announcement<S>)>;
pub type Announcements = Vec<(AbsoluteTarget<SharedRef<str>>, Announcement<String>)>;

pub const MIGRATIONS: diesel_migrations::EmbeddedMigrations = diesel_migrations::embed_migrations!();

fn create_collator() -> Collator {
  let mut options = CollatorOptions::new();
  options.strength = Some(Strength::Primary);
  options.alternate_handling = Some(AlternateHandling::Shifted);
  Collator::try_new(&Default::default(), options).expect("Failed to initialize Unicode sorting")
}

lazy_static::lazy_static! {
    static ref COLLATOR: Collator = create_collator();
}
define_sql_function!(fn get_kind_from_descriptor(descriptor: sql_types::Binary) -> sql_types::Binary);

impl Database {
  pub fn new(db_file: PathBuf) -> Self {
    #[derive(Debug)]
    struct Customizer;
    impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for Customizer {
      fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), Error> {
        conn.register_collation("UNICODE_NOCASE", |a, b| COLLATOR.compare(a, b)).map_err(Error::QueryError)?;
        get_kind_from_descriptor_utils::register_impl(conn, |descriptor: *const [u8]| {
          let descriptor = unsafe { &*descriptor };
          match serde_sqlite_jsonb::from_slice::<Descriptor<&str>>(descriptor) {
            Ok(descriptor) => serde_sqlite_jsonb::to_vec(&descriptor.kind()).unwrap_or_else(|e| {
              eprintln!("Failed to write descriptor kind in database call: {}", e);
              vec![0_u8]
            }),
            Err(e) => {
              eprintln!("Corrupt descriptor in database: {}", e);
              vec![0_u8]
            }
          }
        })
        .map_err(Error::QueryError)?;
        Ok(())
      }
    }
    let pool = {
      eprintln!("Connecting to database: {}", db_file.display());
      let manager = ConnectionManager::<SqliteConnection>::new(db_file.display().to_string());
      Pool::builder().connection_customizer(Box::new(Customizer)).build(manager).expect("Failed to create pool.")
    };
    use diesel_migrations::MigrationHarness;
    eprintln!("Apply migrations");
    let mut db_connection = pool.get().expect("Failed to connect to database");
    db_connection.run_pending_migrations(MIGRATIONS).expect("Failed to migrate database to latest schema");
    let (tx, _) = broadcast::channel(200);
    Database(pool, tx)
  }
  pub fn announcements_read(&self) -> QueryResult<Vec<communication::Announcement<Arc<str>>>> {
    use diesel::prelude::*;
    use schema::announcement::dsl as announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = announcement_schema::announcement
      .select((
        announcement_schema::title,
        announcement_schema::body,
        announcement_schema::when,
        announcement_schema::location,
        announcement_schema::public,
      ))
      .load_iter::<(String, String, AsJsonb<communication::AnnouncementTime>, AsJsonb<UnresolvedTarget<Arc<str>>>, bool), DefaultLoadingMode>(
        &mut db_connection,
      )?
      .map(|r| {
        r.map(|(title, body, when, location, public)| communication::Announcement {
          title: Arc::from(title),
          body: Arc::from(body),
          when: when.0,
          location: location.0,
          public,
        })
      })
      .collect();
    results
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
            announcement_schema::when.eq(AsJsonb(&a.when)),
            announcement_schema::location.eq(AsJsonb(&a.location)),
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
      .values(&(bookmark_schema::player.eq(db_id), bookmark_schema::value.eq(AsJsonb(bookmark))))
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
    let results = bookmark_schema::bookmark
      .select(bookmark_schema::value)
      .filter(bookmark_schema::player.eq(db_id))
      .load_iter::<AsJsonb<spadina_core::resource::Resource<String>>, DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|bookmark| match bookmark {
        Ok(bookmark) => match filter(bookmark.0) {
          Some(bookmark) => Some(Ok(bookmark)),
          None => None,
        },
        Err(e) => Some(Err(e)),
      })
      .collect();
    results
  }
  pub fn bookmark_rm(&self, db_id: i32, bookmark: &spadina_core::resource::Resource<impl AsRef<str> + serde::Serialize + Debug>) -> QueryResult<()> {
    use schema::bookmark::dsl as bookmark_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(bookmark_schema::bookmark.filter(bookmark_schema::player.eq(db_id).and(bookmark_schema::value.eq(&AsJsonb(bookmark)))))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_reset(&self, player: PlayerReference<impl AsRef<str>>) -> QueryResult<Vec<u8>> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let mut calendar_id = vec![0_u8; 10];
    openssl::rand::rand_bytes(&mut calendar_id).map_err(|e| diesel::result::Error::QueryBuilderError(e.into()))?;
    diesel::update(player_schema::player.filter(player.as_expression()))
      .set(player_schema::calendar_id.eq(&calendar_id))
      .execute(&mut db_connection)?;
    Ok(calendar_id)
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
  pub fn calendar_list(&self, db_id: i32) -> QueryResult<Vec<LocalTarget<String>>> {
    use schema::location::dsl as location_schema;
    use schema::location_calendar_subscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = calendar_schema::location_calendar_subscription
      .inner_join(location_schema::location.on(calendar_schema::location.eq(location_schema::id)))
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select((location_schema::descriptor, player_schema::name))
      .filter(calendar_schema::player.eq(db_id))
      .load_iter::<(AsJsonb<Descriptor<String>>, String), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| match r {
        Ok((descriptor, owner)) => Ok(LocalTarget { descriptor: descriptor.0, owner }),
        Err(e) => Err(e),
      })
      .collect();
    results
  }
  pub fn calendar_list_entries(&self, db_id: i32, local_server: &Arc<str>) -> QueryResult<Vec<DirectoryEntry<Arc<str>>>> {
    use schema::location::dsl as location_schema;
    use schema::location_calendar_subscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = calendar_schema::location_calendar_subscription
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
      .load_iter::<(AsJsonb<Descriptor<Arc<str>>>, String, String, NaiveDateTime, NaiveDateTime, i16), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| match r {
        Ok((descriptor, owner, name, updated, created, visibility)) => Ok(DirectoryEntry {
          activity: Activity::Unknown,
          descriptor: descriptor.0,
          name: Arc::from(name),
          owner: Arc::from(owner),
          server: local_server.clone(),
          updated: Utc.from_utc_datetime(&updated),
          created: Utc.from_utc_datetime(&created),
          visibility: Visibility::try_from(visibility).unwrap_or(Visibility::Archived),
        }),
        Err(e) => Err(e),
      })
      .collect();
    results
  }
  pub fn calendar_rm(&self, db_id: i32, target: &LocalTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::location_calendar_subscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(
      calendar_schema::location_calendar_subscription.filter(
        calendar_schema::player.eq(db_id).and(
          calendar_schema::location.eq_any(
            location_schema::location
              .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
              .select(location_schema::id)
              .filter(
                location_schema::descriptor
                  .eq(AsJsonb(target.descriptor.reference(AsReference::<str>::default())))
                  .and(player_schema::name.eq(target.owner.as_ref())),
              ),
          ),
        ),
      ),
    )
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_rm_remote(&self, db_id: i32, target: &AbsoluteTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::remote_calendar_subscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(
      calendar_schema::remote_calendar_subscription.filter(
        calendar_schema::player
          .eq(db_id)
          .and(calendar_schema::descriptor.eq(AsJsonb(target.descriptor.reference(AsReference::<str>::default()))))
          .and(calendar_schema::owner.eq(target.owner.as_ref()))
          .and(calendar_schema::server.eq(target.server.as_ref())),
      ),
    )
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_add(&self, db_id: i32, location: &LocalTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::location_calendar_subscription::dsl as calendar_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select((<i32 as diesel::expression::AsExpression<sql_types::Integer>>::as_expression(db_id), location_schema::id))
      .filter(
        location_schema::descriptor
          .eq(AsJsonb(location.descriptor.reference(AsReference::<str>::default())))
          .and(player_schema::name.eq(location.owner.as_ref())),
      )
      .insert_into(calendar_schema::location_calendar_subscription)
      .into_columns((calendar_schema::player, calendar_schema::location))
      .on_conflict((calendar_schema::player, calendar_schema::location))
      .do_nothing()
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_add_remote(&self, db_id: i32, location: &AbsoluteTarget<impl AsRef<str>>) -> QueryResult<()> {
    use schema::remote_calendar_subscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(calendar_schema::remote_calendar_subscription)
      .values((
        calendar_schema::player.eq(db_id),
        calendar_schema::descriptor.eq(AsJsonb(location.descriptor.reference(AsReference::<str>::default()))),
        calendar_schema::owner.eq(location.owner.as_ref()),
        calendar_schema::server.eq(location.server.as_ref()),
      ))
      .on_conflict((calendar_schema::player, calendar_schema::descriptor, calendar_schema::owner, calendar_schema::server))
      .do_nothing()
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn calendar_rm_all(&self, db_id: i32) -> QueryResult<()> {
    use schema::location_calendar_subscription::dsl as calendar_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(calendar_schema::location_calendar_subscription.filter(calendar_schema::player.eq(db_id))).execute(&mut db_connection)?;
    Ok(())
  }
  pub(crate) fn calender_cache_refresh(&self) -> QueryResult<Vec<StaleRemoteCalendar>> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<Vec<StaleRemoteCalendar>, diesel::result::Error, _>(|db_connection| {
      use diesel::expression::AsExpression;
      use schema::calendar_cache::dsl as calendar_cache_schema;
      use schema::player::dsl as player_schema;
      use schema::remote_calendar_subscription::dsl as remote_calendar_subscription_schema;
      diesel::delete(
        calendar_cache_schema::calendar_cache.filter(
          remote_calendar_subscription_schema::remote_calendar_subscription
            .filter(
              remote_calendar_subscription_schema::player
                .eq(calendar_cache_schema::player)
                .and(remote_calendar_subscription_schema::server.eq(calendar_cache_schema::server)),
            )
            .select(count_star())
            .single_value()
            .eq(0)
            .or(calendar_cache_schema::last_used.assume_not_null().lt((Utc::now() - Duration::days(7)).naive_utc())),
        ),
      )
      .execute(db_connection)?;

      diesel::alias!(schema::calendar_cache as original_cache: OriginalCacheSchema);

      let now = Utc::now();
      diesel::insert_into(calendar_cache_schema::calendar_cache)
        .values(
          remote_calendar_subscription_schema::remote_calendar_subscription
            .filter(
              original_cache
                .filter(
                  original_cache.field(calendar_cache_schema::server).eq(remote_calendar_subscription_schema::server).and(
                    original_cache
                      .field(calendar_cache_schema::last_requested)
                      .lt(<NaiveDateTime as AsExpression<sql_types::Nullable<sql_types::Timestamp>>>::as_expression(
                        (now - Duration::minutes(30)).naive_utc(),
                      ))
                      .or(original_cache.field(calendar_cache_schema::last_used).gt(<NaiveDateTime as AsExpression<
                        sql_types::Nullable<sql_types::Timestamp>,
                      >>::as_expression(
                        (now - Duration::hours(1)).naive_utc()
                      ))),
                  ),
                )
                .select(count_star())
                .single_value()
                .eq(0),
            )
            .group_by((remote_calendar_subscription_schema::player, remote_calendar_subscription_schema::server))
            .select((
              remote_calendar_subscription_schema::player,
              remote_calendar_subscription_schema::server,
              sql::<sql_types::Binary>("jsonb('[]')"),
              <Option<NaiveDateTime> as AsExpression<sql_types::Nullable<sql_types::Timestamp>>>::as_expression(None),
              <NaiveDateTime as AsExpression<sql_types::Nullable<sql_types::Timestamp>>>::as_expression(now.naive_utc()),
              <Option<NaiveDateTime> as AsExpression<sql_types::Nullable<sql_types::Timestamp>>>::as_expression(None),
              <NaiveDateTime as AsExpression<sql_types::Timestamp>>::as_expression(now.naive_utc()),
            )),
        )
        .on_conflict((calendar_cache_schema::player, calendar_cache_schema::server))
        .do_update()
        .set(calendar_cache_schema::last_requested.eq(excluded(calendar_cache_schema::last_requested)))
        .returning((
          player_schema::player
            .select(player_schema::name)
            .filter(player_schema::id.eq(calendar_cache_schema::player))
            .single_value()
            .assume_not_null(),
          calendar_cache_schema::server,
        ))
        .load_iter::<(String, String), DefaultLoadingMode>(db_connection)?
        .map(|row| row.map(|(player, server)| StaleRemoteCalendar { player, server }))
        .collect()
    })
  }
  pub(crate) fn calender_cache_fetch_locations_by_server(&self, player: &str, remote_server: &str) -> QueryResult<Vec<LocalTarget<String>>> {
    use schema::player::dsl as player_schema;
    use schema::remote_calendar_subscription::dsl as remote_calendar_subscription_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = remote_calendar_subscription_schema::remote_calendar_subscription
      .select((remote_calendar_subscription_schema::owner, remote_calendar_subscription_schema::descriptor))
      .filter(
        remote_calendar_subscription_schema::player
          .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(player)).single_value().assume_not_null())
          .and(remote_calendar_subscription_schema::server.eq(remote_server)),
      )
      .load_iter::<(String, AsJsonb<Descriptor<String>>), DefaultLoadingMode>(&mut db_connection)?
      .map(|record| match record {
        Ok((owner, descriptor)) => Ok(LocalTarget { descriptor: descriptor.0, owner }),
        Err(e) => Err(e),
      })
      .collect();
    results
  }
  pub(crate) fn calendar_cache_update(
    &self,
    player: &str,
    server: &str,
    entries: CalendarCacheEntries<impl AsRef<str> + Serialize + Debug>,
  ) -> QueryResult<()> {
    use schema::calendar_cache::dsl as calendar_cache_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(calendar_cache_schema::calendar_cache)
      .values((
        calendar_cache_schema::player
          .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(player)).single_value().assume_not_null()),
        calendar_cache_schema::server.eq(server),
        calendar_cache_schema::last_updated.eq(Utc::now().naive_utc()),
        calendar_cache_schema::calendar_entries.eq(AsJsonb(entries)),
      ))
      .on_conflict((calendar_cache_schema::player, calendar_cache_schema::server))
      .do_update()
      .set((
        calendar_cache_schema::last_updated.eq(excluded(calendar_cache_schema::last_updated)),
        calendar_cache_schema::calendar_entries.eq(excluded(calendar_cache_schema::calendar_entries)),
      ))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn direct_message_clean(&self) -> QueryResult<()> {
    use schema::local_player_chat::dsl as local_player_chat_schema;
    use schema::location_chat::dsl as location_chat_schema;
    use schema::remote_player_chat::dsl as remote_player_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    let now = Utc::now();
    diesel::delete(location_chat_schema::location_chat.filter(location_chat_schema::created.le((now - Duration::days(30)).naive_utc())))
      .execute(&mut db_connection)?;
    diesel::delete(local_player_chat_schema::local_player_chat.filter(local_player_chat_schema::created.le((now - Duration::days(30)).naive_utc())))
      .execute(&mut db_connection)?;
    diesel::delete(
      remote_player_chat_schema::remote_player_chat.filter(remote_player_chat_schema::created.le((now - Duration::days(30)).naive_utc())),
    )
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
    use schema::local_player_chat::dsl as chat_schema;
    use schema::player::dsl as player_schema;
    let results = chat_schema::local_player_chat
      .select((chat_schema::body, chat_schema::created, chat_schema::recipient.eq(db_id)))
      .filter(
        chat_schema::created.ge(from.naive_utc()).and(chat_schema::created.lt(to.naive_utc())).and(
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
      .load_iter::<(AsJsonb<communication::MessageBody<String>>, NaiveDateTime, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(communication::DirectMessage { inbound, body: body.0, timestamp: Utc.from_utc_datetime(&timestamp) }),
      })
      .collect();
    results
  }
  pub fn direct_message_write(
    &self,
    sender: &str,
    recipient: &str,
    body: &communication::MessageBody<impl AsRef<str> + serde::Serialize + Debug>,
  ) -> QueryResult<DateTime<Utc>> {
    use schema::local_player_chat::dsl as chat_schema;
    use schema::local_player_last_read::dsl as last_read_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    let recipient_id = player_schema::player.select(player_schema::id).filter(player_schema::name.eq(recipient)).first::<i32>(&mut db_connection)?;
    Ok(Utc.from_utc_datetime(
      &(if body.is_transient() {
        diesel::insert_into(last_read_schema::local_player_last_read)
          .values((
            last_read_schema::recipient.eq(recipient_id),
            last_read_schema::sender
              .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value().assume_not_null()),
            last_read_schema::when.eq(Utc::now().naive_utc()),
          ))
          .on_conflict((last_read_schema::recipient, last_read_schema::sender))
          .do_update()
          .set(last_read_schema::when.eq(excluded(last_read_schema::when)))
          .returning(last_read_schema::when)
          .get_result(&mut db_connection)
      } else {
        diesel::insert_into(chat_schema::local_player_chat)
          .values((
            chat_schema::recipient.eq(recipient_id),
            chat_schema::body.eq(AsJsonb(body)),
            chat_schema::created.eq(Utc::now().naive_utc()),
            chat_schema::sender
              .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(sender)).single_value().assume_not_null()),
          ))
          .returning(chat_schema::created)
          .get_result(&mut db_connection)
      }?),
    ))
  }
  pub fn direct_message_stats(
    &self,
    db_id: i32,
  ) -> QueryResult<(std::collections::HashMap<PlayerIdentifier<String>, communication::DirectMessageInfo>, DateTime<Utc>)> {
    let mut db_connection = self.0.get().unwrap();
    use schema::local_player_chat::dsl as local_player_chat_schema;
    use schema::local_player_last_read::dsl as local_player_last_read_schema;
    use schema::player::dsl as player_schema;
    use schema::remote_player_chat::dsl as remote_player_chat_schema;
    use schema::remote_player_last_read::dsl as remote_player_last_read_schema;
    let mut stats = player_schema::player
      .left_join(local_player_last_read_schema::local_player_last_read.on(local_player_last_read_schema::sender.eq(player_schema::id)))
      .filter(local_player_last_read_schema::recipient.eq(db_id))
      .select((
        player_schema::name,
        local_player_chat_schema::local_player_chat
          .select(diesel::dsl::max(local_player_chat_schema::created))
          .filter(local_player_chat_schema::recipient.eq(db_id).and(local_player_chat_schema::recipient.eq(player_schema::id)))
          .single_value(),
        local_player_last_read_schema::when.nullable(),
      ))
      .load_iter::<(String, Option<NaiveDateTime>, Option<NaiveDateTime>), DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|r| match r {
        Ok((player, Some(last_received), last_read)) => Some(Ok((
          PlayerIdentifier::Local(player),
          communication::DirectMessageInfo {
            last_received: Utc.from_utc_datetime(&last_received),
            last_read: last_read.map(|last_read| Utc.from_utc_datetime(&last_read)),
          },
        ))),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
      })
      .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    for item in remote_player_chat_schema::remote_player_chat
      .filter(remote_player_chat_schema::player.eq(db_id).and(remote_player_chat_schema::inbound.eq(false)))
      .group_by((remote_player_chat_schema::remote_player, remote_player_chat_schema::remote_server))
      .select((
        remote_player_chat_schema::remote_player,
        remote_player_chat_schema::remote_server,
        diesel::dsl::max(remote_player_chat_schema::created),
        remote_player_last_read_schema::remote_player_last_read
          .select(diesel::dsl::max(remote_player_last_read_schema::when))
          .filter(
            remote_player_last_read_schema::player
              .eq(remote_player_chat_schema::player)
              .and(remote_player_last_read_schema::remote_player.eq(remote_player_chat_schema::remote_player))
              .and(remote_player_last_read_schema::remote_server.eq(remote_player_chat_schema::remote_server)),
          )
          .single_value(),
      ))
      .load_iter::<(String, String, Option<NaiveDateTime>, Option<NaiveDateTime>), DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|r| match r {
        Ok((player, server, Some(last_received), last_read)) => Some(Ok((
          PlayerIdentifier::Remote { server, player },
          communication::DirectMessageInfo {
            last_received: Utc.from_utc_datetime(&last_received),
            last_read: last_read.map(|last_read| Utc.from_utc_datetime(&last_read)),
          },
        ))),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
      })
    {
      let (player, info) = item?;
      stats.insert(player, info);
    }
    let last_login =
      player_schema::player.select(player_schema::last_login).filter(player_schema::id.eq(db_id)).first::<NaiveDateTime>(&mut db_connection)?;
    Ok((stats, Utc.from_utc_datetime(&last_login)))
  }
  pub fn player_reset(&self, player_name: &str) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    use schema::player::dsl as player_schema;
    diesel::update(player_schema::player.filter(player_schema::name.eq(player_name)))
      .set(player_schema::reset.eq(true))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn player_clean(&self) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<(), diesel::result::Error, _>(|db_connection| {
      use schema::bookmark::dsl as bookmark_schema;
      use schema::local_player_chat::dsl as local_player_chat_schema;
      use schema::location::dsl as location_schema;
      use schema::location_calendar_subscription::dsl as location_calendar_subscription_schema;
      use schema::location_chat::dsl as location_chat_schema;
      use schema::player::dsl as player_schema;
      use schema::public_key::dsl as public_key_schema;
      use schema::remote_player_chat::dsl as remote_player_chat_schema;
      diesel::delete(
        local_player_chat_schema::local_player_chat.filter(
          local_player_chat_schema::sender
            .eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))
            .or(local_player_chat_schema::recipient.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
        ),
      )
      .execute(db_connection)?;
      diesel::delete(
        location_calendar_subscription_schema::location_calendar_subscription
          .filter(location_calendar_subscription_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        remote_player_chat_schema::remote_player_chat
          .filter(remote_player_chat_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(
        location_chat_schema::location_chat.filter(
          location_chat_schema::location.eq_any(
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
        public_key_schema::public_key
          .filter(public_key_schema::player.eq_any(player_schema::player.select(player_schema::id).filter(player_schema::reset))),
      )
      .execute(db_connection)?;
      diesel::delete(player_schema::player.filter(player_schema::reset)).execute(db_connection)?;

      Ok(())
    })
  }
  pub fn player_load(&self, player_name: &str) -> QueryResult<(i32, Vec<u8>)> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(player_schema::player)
      .values((
        player_schema::name.eq(player_name),
        player_schema::avatar.eq(AsJsonb(Avatar::default_for(player_name))),
        player_schema::message_acl.eq(AsJsonb(AccessSetting::<&str, _> { default: access::SimpleAccess::Allow, rules: Vec::new() })),
        player_schema::online_acl.eq(AsJsonb(AccessSetting::<&str, access::OnlineAccess>::default())),
        player_schema::default_location_acl.eq(AsJsonb(AccessSetting::<&str, access::Privilege>::default())),
        player_schema::last_login.eq(Utc::now().naive_utc()),
      ))
      .on_conflict(player_schema::name)
      .do_update()
      .set(player_schema::last_login.eq(excluded(player_schema::last_login)))
      .returning((player_schema::id, player_schema::calendar_id))
      .get_result(&mut db_connection)
  }
  pub fn public_key_add(&self, db_id: i32, der: &[u8]) -> QueryResult<usize> {
    use schema::public_key::dsl as public_key_dsl;
    let fingerprint = spadina_core::net::server::auth::compute_fingerprint(der);
    let mut db_connection = self.0.get().unwrap();
    diesel::insert_into(public_key_dsl::public_key)
      .values((public_key_dsl::player.eq(db_id), public_key_dsl::fingerprint.eq(fingerprint), public_key_dsl::key.eq(der)))
      .on_conflict_do_nothing()
      .execute(&mut db_connection)
  }
  pub fn public_key_get(&self, player_name: &str, fingerprint: &str) -> QueryResult<Option<Vec<u8>>> {
    use schema::player::dsl as player_schema;
    use schema::public_key::dsl as public_key_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(
      public_key_schema::public_key.filter(
        public_key_schema::player
          .eq(player_schema::player.select(player_schema::id).filter(player_schema::name.eq(player_name)).single_value().assume_not_null())
          .and(public_key_schema::fingerprint.eq(fingerprint)),
      ),
    )
    .set(public_key_schema::last_used.eq(Utc::now().naive_utc()))
    .returning(public_key_schema::key)
    .get_result(&mut db_connection)
    .optional()
  }
  pub fn public_key_list(&self, db_id: i32) -> QueryResult<BTreeMap<String, PublicKey>> {
    use schema::public_key::dsl as public_key_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = public_key_schema::public_key
      .select((public_key_schema::fingerprint, public_key_schema::created, public_key_schema::last_used))
      .filter(public_key_schema::player.eq(&db_id))
      .load_iter::<(String, NaiveDateTime, Option<NaiveDateTime>), DefaultLoadingMode>(&mut db_connection)?
      .map(|r| match r {
        Ok((fingerprint, created, last_used)) => Ok((
          fingerprint,
          PublicKey { created: Utc.from_utc_datetime(&created), last_used: last_used.map(|last_used| Utc.from_utc_datetime(&last_used)) },
        )),
        Err(e) => Err(e),
      })
      .collect();
    results
  }
  pub fn public_key_rm(&self, db_id: i32, fingerprint: &str) -> QueryResult<usize> {
    use schema::public_key::dsl as public_key_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(public_key_schema::public_key.filter(public_key_schema::fingerprint.eq(fingerprint).and(public_key_schema::player.eq(db_id))))
      .execute(&mut db_connection)
  }
  pub fn public_key_rm_all(&self, db_id: i32) -> QueryResult<usize> {
    use schema::public_key::dsl as public_key_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(public_key_schema::public_key.filter(public_key_schema::player.eq(&db_id))).execute(&mut db_connection)
  }
  pub fn location_acl_read(&self, id: i32) -> QueryResult<AccessSetting<Arc<str>, access::Privilege>> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    location_schema::location
      .select(location_schema::acl)
      .filter(location_schema::id.eq(id))
      .first::<AsJsonb<AccessSetting<Arc<str>, access::Privilege>>>(&mut db_connection)
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
      .set(location_schema::acl.eq(AsJsonb(access_control)))
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
      .inner_join(player_schema::player.on(player_schema::id.eq(location_schema::owner)))
      .select(location_schema::id)
      .filter(scope.as_expression())
      .load::<i32>(&mut db_connection)?;

    diesel::update(location_schema::location.filter(location_schema::id.eq_any(locations)))
      .set(location_schema::acl.eq(AsJsonb(acl)))
      .execute(&mut db_connection)?;
    Ok(())
  }

  pub fn location_announcements_write(&self, id: i32, announcements: &[Announcement<impl AsRef<str>>]) -> QueryResult<()> {
    use diesel::prelude::*;
    use schema::location_announcement::dsl as location_announcement_schema;
    self.0.get().unwrap().transaction::<(), diesel::result::Error, _>(|db_connection| {
      diesel::delete(location_announcement_schema::location_announcement.filter(location_announcement_schema::location.eq(id)))
        .execute(db_connection)?;
      let rows: Vec<_> = announcements
        .iter()
        .map(|a| {
          (
            location_announcement_schema::title.eq(a.title.as_ref()),
            location_announcement_schema::body.eq(a.body.as_ref()),
            location_announcement_schema::when.eq(AsJsonb(&a.when)),
            location_announcement_schema::expires.eq(a.when.expires().naive_utc()),
            location_announcement_schema::public.eq(a.public),
          )
        })
        .collect();
      diesel::insert_into(location_announcement_schema::location_announcement).values(&rows).execute(db_connection)?;

      Ok(())
    })
  }
  pub fn location_announcements_clean(&self) -> QueryResult<()> {
    use diesel::prelude::*;
    use schema::location_announcement::dsl as location_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(location_announcement_schema::location_announcement.filter(location_announcement_schema::expires.gt(Utc::now().naive_utc())))
      .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn location_announcements_read(&self, id: i32) -> QueryResult<Vec<Announcement<String>>> {
    use schema::location_announcement::dsl as location_announcement_schema;
    let mut db_connection = self.0.get().unwrap();
    let results = location_announcement_schema::location_announcement
      .select((
        location_announcement_schema::id,
        location_announcement_schema::title,
        location_announcement_schema::body,
        location_announcement_schema::when,
        location_announcement_schema::public,
      ))
      .filter(location_announcement_schema::location.eq(id))
      .load_iter::<(i32, String, String, AsJsonb<communication::AnnouncementTime>, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| result.map(|(_, title, body, when, public)| Announcement { title, body, public, when: when.0 }))
      .collect();
    results
  }
  pub fn location_announcements_fetch_all(
    &self,
    scope: location_scope::LocationListScope<impl AsRef<str> + Debug>,
    player_secret_id: Option<Vec<u8>>,
    local_server: &Arc<str>,
  ) -> QueryResult<Announcements> {
    use diesel::expression::AsExpression;
    use schema::location::dsl as location_schema;
    use schema::location_announcement::dsl as location_announcement_schema;
    use schema::player::dsl as player_schema;
    type AnnouncementTuple = (
      String,
      AsJsonb<Descriptor<String>>,
      i32,
      String,
      String,
      AsJsonb<communication::AnnouncementTime>,
      bool,
      bool,
      AsJsonb<AccessSetting<String, access::SimpleAccess>>,
      String,
    );
    let mut db_connection = self.0.get().unwrap();
    let public_filter: Box<dyn BoxableExpression<_, diesel::sqlite::Sqlite, SqlType = sql_types::Bool>> = match player_secret_id.as_ref() {
      None => Box::new(location_announcement_schema::public.as_expression()),
      Some(calendar_id) => Box::new(player_schema::calendar_id.eq(calendar_id).or(location_announcement_schema::public).as_expression()),
    };
    let query = location_announcement_schema::location_announcement
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

    let mut entries = match player_secret_id.as_ref() {
      Some(player_secret_id) => {
        use schema::location_calendar_subscription::dsl as calendar_schema;
        diesel::alias!(schema::player as location_owner: RealmOwnerSchema);
        query
          .union(
            location_announcement_schema::location_announcement
              .inner_join(calendar_schema::location_calendar_subscription.on(calendar_schema::location.eq(location_announcement_schema::location)))
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
            AbsoluteTarget {
              owner: SharedRef::Single(owner),
              server: SharedRef::Shared(local_server.clone()),
              descriptor: descriptor.0.convert(AsSingle::default()),
            },
            Announcement { title, body, public, when: when.0 },
          )))
        } else {
          None
        }
      }
      Err(e) => Some(Err(e)),
    })
    .collect::<QueryResult<Announcements>>()?;
    if let Some(player_secret_id) = player_secret_id.as_ref() {
      use schema::calendar_cache::dsl as calendar_cache_schema;
      entries.extend(
        diesel::update(
          calendar_cache_schema::calendar_cache.filter(
            calendar_cache_schema::player
              .eq(
                player_schema::player
                  .select(player_schema::id)
                  .filter(player_schema::calendar_id.eq(player_secret_id))
                  .single_value()
                  .assume_not_null(),
              )
              .and(calendar_cache_schema::calendar_entries.is_not_null()),
          ),
        )
        .set(calendar_cache_schema::last_used.eq(Utc::now().naive_utc()))
        .returning((calendar_cache_schema::server, calendar_cache_schema::calendar_entries))
        .load_iter::<(String, AsJsonb<CalendarCacheEntries<String>>), DefaultLoadingMode>(&mut db_connection)?
        .flat_map(|row| match row {
          Ok((remote_server, cached_announcements)) => {
            let remote_server: Arc<str> = remote_server.into();
            cached_announcements
              .0
              .into_iter()
              .map(|(LocalTarget { owner, descriptor }, announcement)| {
                (
                  AbsoluteTarget {
                    owner: SharedRef::Single(owner),
                    server: SharedRef::Shared(remote_server.clone()),
                    descriptor: descriptor.convert(AsSingle::default()),
                  },
                  announcement,
                )
              })
              .collect()
          }
          Err(e) => {
            eprintln!("Ignoring corrupt row in remote calendar cache: {}", e);
            Vec::new()
          }
        }),
      )
    }
    Ok(entries)
  }
  pub fn location_announcements_fetch_for_remote(
    &self,
    locations: Vec<LocalTarget<impl AsRef<str> + Debug>>,
    player: PlayerIdentifier<&str>,
    local_server: &Arc<str>,
  ) -> QueryResult<CalendarCacheEntries<String>> {
    use schema::location::dsl as location_schema;
    use schema::location_announcement::dsl as location_announcement_schema;
    use schema::player::dsl as player_schema;
    type AnnouncementTuple = (
      String,
      AsJsonb<Descriptor<String>>,
      i32,
      String,
      String,
      AsJsonb<communication::AnnouncementTime>,
      bool,
      AsJsonb<AccessSetting<String, access::SimpleAccess>>,
    );
    let mut db_connection = self.0.get().unwrap();
    let results = location_announcement_schema::location_announcement
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
        location_schema::acl,
      ))
      .filter(
        location_scope::LocationListScope::Or(
          locations
            .into_iter()
            .map(|LocalTarget { owner, descriptor }| {
              location_scope::LocationListScope::Exact(location_scope::LocationScope { owner: PlayerReference::Name(owner), descriptor })
            })
            .collect(),
        )
        .as_expression(),
      )
      .load_iter::<AnnouncementTuple, DefaultLoadingMode>(&mut db_connection)?
      .filter_map(|result| match result {
        Ok((owner, descriptor, _, title, body, when, public, access)) => {
          if public || access.0.check(&player, local_server) == access::SimpleAccess::Allow {
            Some(Ok((LocalTarget { owner, descriptor: descriptor.0 }, Announcement { title, body, public, when: when.0 })))
          } else {
            None
          }
        }
        Err(e) => Some(Err(e)),
      })
      .collect();
    results
  }
  pub(crate) fn location_change_visibility(
    &self,
    visibility: Visibility,
    predicate: location_scope::LocationListScope<impl AsRef<str> + Debug>,
  ) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    for id in diesel::update(
      location_schema::location.filter(
        location_schema::visibility.ne(visibility as i16).and(
          player_schema::player
            .filter(location_schema::owner.eq(player_schema::id).and(predicate.as_expression()))
            .select(count_star())
            .single_value()
            .assume_not_null()
            .gt(0),
        ),
      ),
    )
    .set((location_schema::visibility.eq(visibility as i16), location_schema::visibility_changed.eq(Utc::now().naive_utc())))
    .returning(location_schema::id)
    .load_iter::<i32, DefaultLoadingMode>(&mut db_connection)?
    {
      let _ = self.1.send(id?);
    }
    Ok(())
  }
  pub fn location_chat_delete(&self, db_id: i32, from: DateTime<Utc>, to: DateTime<Utc>) -> QueryResult<()> {
    use schema::location_chat::dsl as location_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::delete(
      location_chat_schema::location_chat.filter(
        location_chat_schema::location
          .eq(db_id)
          .and(location_chat_schema::created.ge(from.naive_utc()))
          .and(location_chat_schema::created.le(to.naive_utc())),
      ),
    )
    .execute(&mut db_connection)?;
    Ok(())
  }
  pub fn location_chat_write(
    &self,
    db_id: i32,
    sender: &PlayerIdentifier<impl AsRef<str> + serde::Serialize + Debug>,
    body: &communication::MessageBody<impl AsRef<str> + serde::Serialize + Debug>,
  ) -> QueryResult<DateTime<Utc>> {
    use schema::location_chat::dsl as location_chat_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      Utc.from_utc_datetime(
        &(diesel::insert_into(location_chat_schema::location_chat)
          .values((
            location_chat_schema::body.eq(AsJsonb(body)),
            location_chat_schema::principal.eq(AsJsonb(sender)),
            location_chat_schema::created.eq(Utc::now().naive_utc()),
            location_chat_schema::location.eq(db_id),
          ))
          .returning(location_chat_schema::created)
          .get_result(&mut db_connection)?),
      ),
    )
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
        location_schema::descriptor.eq(AsJsonb(descriptor)),
        location_schema::state.eq(AsJsonb(state)),
        location_schema::acl.eq(
          player_schema::player.select(player_schema::default_location_acl).filter(player_schema::name.eq(owner)).single_value().assume_not_null(),
        ),
        location_schema::visibility.eq(Visibility::Private as i16),
      ))
      .returning(location_schema::id)
      .on_conflict((location_schema::owner, location_schema::descriptor))
      .do_update()
      .set(location_schema::updated_at.eq(excluded(location_schema::updated_at)))
      .get_result::<i32>(&mut db_connection)
  }
  pub fn location_delete(&self, db_id: i32) -> QueryResult<()> {
    let mut db_connection = self.0.get().unwrap();
    db_connection.transaction::<_, diesel::result::Error, _>(|db_connection| {
      use schema::location::dsl as location_schema;
      use schema::location_announcement::dsl as announcement_schema;
      use schema::location_calendar_subscription::dsl as calendar_schema;
      use schema::location_chat::dsl as chat_schema;
      diesel::delete(announcement_schema::location_announcement.filter(announcement_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(calendar_schema::location_calendar_subscription.filter(calendar_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(chat_schema::location_chat.filter(chat_schema::location.eq(db_id))).execute(db_connection)?;
      diesel::delete(
        chat_schema::location_chat.filter(
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
      .load_iter::<(AsJsonb<Descriptor<Arc<str>>>, String, String, NaiveDateTime, NaiveDateTime, i16), DefaultLoadingMode>(&mut db_connection)
      .and_then(|entries| {
        entries
          .map(|result| {
            result.map(|(descriptor, owner, name, updated, created, visibility)| DirectoryEntry {
              descriptor: descriptor.0,
              owner: owner.into(),
              name: name.into(),
              activity: Activity::Unknown,
              server: server_name.clone(),
              updated: Utc.from_utc_datetime(&updated),
              created: Utc.from_utc_datetime(&created),
              visibility: Visibility::try_from(visibility).unwrap_or(Visibility::Archived),
            })
          })
          .collect()
      })
  }
  pub fn location_messages(&self, db_id: i32, from: DateTime<Utc>, to: DateTime<Utc>) -> QueryResult<Vec<ChatMessage<String>>> {
    let mut db_connection = self.0.get().unwrap();
    use schema::location_chat::dsl as location_chat_schema;
    let result = location_chat_schema::location_chat
      .select((location_chat_schema::principal, location_chat_schema::created, location_chat_schema::body))
      .filter(
        location_chat_schema::location
          .eq(db_id)
          .and(location_chat_schema::created.ge(from.naive_utc()))
          .and(location_chat_schema::created.lt(to.naive_utc())),
      )
      .load_iter::<(AsJsonb<PlayerIdentifier<String>>, NaiveDateTime, AsJsonb<communication::MessageBody<String>>), DefaultLoadingMode>(
        &mut db_connection,
      )?
      .map(|r| r.map(|(sender, timestamp, body)| ChatMessage { body: body.0, sender: sender.0, timestamp: Utc.from_utc_datetime(&timestamp) }))
      .collect();
    result
  }
  pub fn location_state_read(&self, db_id: i32) -> QueryResult<serde_json::Value> {
    use schema::location::dsl as location_schema;

    let mut db_connection = self.0.get().unwrap();

    location_schema::location
      .select(location_schema::state)
      .filter(location_schema::id.eq(db_id))
      .first::<AsJsonb<serde_json::Value>>(&mut db_connection)
      .map(|state| state.0)
  }

  pub fn location_state_write(&self, db_id: i32, state: serde_json::Value) -> QueryResult<()> {
    use schema::location::dsl as location_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(location_schema::location.filter(location_schema::id.eq(db_id)))
      .set(location_schema::state.eq(AsJsonb(state)))
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
    use schema::remote_player_chat::dsl as chat_schema;
    let result = chat_schema::remote_player_chat
      .select((chat_schema::body, chat_schema::created, chat_schema::inbound))
      .filter(chat_schema::player.eq(db_id).and(chat_schema::remote_player.eq(remote_player)).and(chat_schema::remote_server.eq(remote_server)))
      .load_iter::<(AsJsonb<communication::MessageBody<String>>, NaiveDateTime, bool), DefaultLoadingMode>(&mut db_connection)?
      .map(|result| match result {
        Err(e) => Err(e),
        Ok((body, timestamp, inbound)) => Ok(communication::DirectMessage { body: body.0, timestamp: Utc.from_utc_datetime(&timestamp), inbound }),
      })
      .collect();
    result
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
    use schema::remote_player_chat::dsl as remote_player_chat_schema;
    use schema::remote_player_last_read::dsl as remote_player_last_read_schema;
    let mut db_connection = self.0.get().unwrap();
    if body.is_transient() {
      diesel::insert_into(remote_player_last_read_schema::remote_player_last_read)
        .values((
          remote_player_last_read_schema::player.eq(player.get_id(&mut db_connection)?),
          remote_player_last_read_schema::remote_player.eq(remote_player),
          remote_player_last_read_schema::remote_server.eq(remote_server),
          remote_player_last_read_schema::when.eq(timestamp.naive_utc()),
        ))
        .on_conflict((
          remote_player_last_read_schema::player,
          remote_player_last_read_schema::remote_player,
          remote_player_last_read_schema::remote_server,
        ))
        .do_update()
        .set(remote_player_last_read_schema::when.eq(excluded(remote_player_last_read_schema::when)))
        .execute(&mut db_connection)?;
    } else {
      diesel::insert_into(remote_player_chat_schema::remote_player_chat)
        .values(&(
          remote_player_chat_schema::player.eq(player.get_id(&mut db_connection)?),
          remote_player_chat_schema::remote_player.eq(remote_player),
          remote_player_chat_schema::remote_server.eq(remote_server),
          remote_player_chat_schema::body.eq(AsJsonb(&body)),
          remote_player_chat_schema::created.eq(timestamp.naive_utc()),
          remote_player_chat_schema::inbound.eq(inbound),
        ))
        .execute(&mut db_connection)?;
    }
    Ok(timestamp)
  }
  pub(crate) fn player_avatar_read(&self, db_id: i32) -> QueryResult<Avatar> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(player_schema::player.select(player_schema::avatar).filter(player_schema::id.eq(db_id)).get_result::<AsJsonb<Avatar>>(&mut db_connection)?.0)
  }
  pub(crate) fn player_avatar_write(&self, db_id: i32, avatar: &Avatar) -> QueryResult<()> {
    use schema::player::dsl as player_schema;
    let mut db_connection = self.0.get().unwrap();
    diesel::update(player_schema::player.filter(player_schema::id.eq(db_id)))
      .set(player_schema::avatar.eq(AsJsonb(avatar)))
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
        .get_result::<AsJsonb<AccessSetting<String, T::Verb>>>(&mut db_connection)
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
    diesel::update(player_schema::player.filter(player_schema::id.eq(id))).set(column.eq(AsJsonb(access_control))).execute(&mut db_connection)?;
    Ok(())
  }
  pub fn setting_read<T: setting::Setting>(&self) -> QueryResult<T::Stored> {
    use schema::server_setting::dsl as server_setting_schema;
    let mut db_connection = self.0.get().unwrap();
    Ok(
      server_setting_schema::server_setting
        .select(server_setting_schema::data)
        .filter(server_setting_schema::category.eq(std::str::from_utf8(&[T::CODE]).unwrap()))
        .get_result::<AsJsonb<T::Stored>>(&mut db_connection)
        .optional()?
        .map(|j| j.0)
        .unwrap_or_default(),
    )
  }
  pub fn setting_write<T: setting::Setting>(&self, data: &T::Stored) -> QueryResult<()> {
    use schema::server_setting::dsl as server_setting_schema;
    let mut db_connection = self.0.get().unwrap();

    diesel::insert_into(server_setting_schema::server_setting)
      .values(&(server_setting_schema::category.eq(std::str::from_utf8(&[T::CODE]).unwrap()), server_setting_schema::data.eq(AsJsonb(data))))
      .on_conflict(server_setting_schema::category)
      .do_update()
      .set(server_setting_schema::data.eq(excluded(server_setting_schema::data)))
      .execute(&mut db_connection)?;
    Ok(())
  }
}
