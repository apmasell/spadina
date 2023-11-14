use crate::accounts::ServerAccounts;
use crate::database::persisted::{PersistedGlobal, PersistedWatch, Persistence};
use crate::database::setting::Setting;
use crate::database::Database;
use crate::http_server::jwt;
use crate::metrics::SettingLabel;
use diesel::result::QueryResult;
use spadina_core::access::{AccessSetting, BannedPeer, SimpleAccess};
use spadina_core::communication::Announcement;
use spadina_core::player::PlayerIdentifier;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;

pub(crate) struct AccessManagement {
  pub access: PersistedGlobal<'static, ServerAccess, SettingLabel>,
  pub accounts: ServerAccounts,
  pub announcements: PersistedWatch<ServerAnnouncements>,
  pub banned_peers: PersistedGlobal<'static, BannedPeers, SettingLabel>,
  #[allow(dead_code)]
  death_rx: broadcast::Receiver<()>,
  death_tx: broadcast::Sender<()>,
  pub jwt_key: jwt::KeyPair,
  pub server_name: Arc<str>,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ServerAnnouncements;
#[derive(Debug, Copy, Clone)]
pub(crate) struct BannedPeers;
#[derive(Debug, Copy, Clone)]
pub(crate) struct ServerAccess;

impl AccessManagement {
  pub fn new(accounts: ServerAccounts, database: &Database, server_name: Arc<str>) -> QueryResult<Arc<Self>> {
    eprintln!("Setting up access management");
    let access = PersistedGlobal::new(database.clone(), ServerAccess, &crate::metrics::SETTING)?;
    eprintln!("Setting up announcements");
    let announcements = PersistedWatch::new(database.clone(), ServerAnnouncements)?;
    eprintln!("Setting up peers bans");
    let banned_peers = PersistedGlobal::new(database.clone(), BannedPeers, &crate::metrics::SETTING)?;
    eprintln!("Setting up exit handler");
    let (ctrl_c, death_rx) = broadcast::channel(1);
    let death_tx = ctrl_c.clone();

    tokio::spawn(async move {
      tokio::signal::ctrl_c().await.expect("Failed to handle Ctrl-C");
      ctrl_c.send(()).expect("Failed to notify of shutdown.");
    });
    eprintln!("Access management configured");
    Ok(Arc::new(Self { access, announcements, accounts, banned_peers, death_rx, death_tx, jwt_key: Default::default(), server_name }))
  }
  pub async fn check_access(&self, location: &'static str, player: &PlayerIdentifier<impl AsRef<str>>) -> bool {
    self.access.read(location, |acl| acl.check(player, &self.server_name)).await == SimpleAccess::Allow
  }
  pub fn give_me_death(&self) -> broadcast::Receiver<()> {
    self.death_tx.subscribe()
  }
}

impl Setting for BannedPeers {
  const CODE: u8 = b'b';
  const METRIC: &'static str = "banned_peers";
  type Stored = HashSet<BannedPeer<String>>;
}
impl Setting for ServerAccess {
  const CODE: u8 = b'a';
  const METRIC: &'static str = "server_access";
  type Stored = AccessSetting<Arc<str>, SimpleAccess>;
}
impl Persistence for ServerAnnouncements {
  type Value = Vec<Announcement<Arc<str>>>;

  fn load(&self, database: &Database) -> QueryResult<Self::Value> {
    database.announcements_read()
  }

  fn store(&self, database: &Database, value: &Self::Value) -> QueryResult<()> {
    database.announcements_write(value)
  }
}
