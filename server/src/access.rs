#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AccessSetting<T: Copy> {
  pub default: T,
  pub rules: Vec<spadina_core::access::AccessControl<T>>,
}

pub(crate) struct AuthNZ {
  pub access: crate::database::persisted::PersistedGlobal<'static, crate::access::ServerAccess, crate::access::ServerAccess>,
  pub admin: crate::database::persisted::PersistedGlobal<'static, crate::access::ServerAccess, crate::access::ServerAccess>,
  pub announcements: crate::database::persisted::PersistedWatch<crate::access::Announcements>,
  pub authentication: Box<dyn crate::auth::AuthProvider>,
  pub banned_peers: crate::database::persisted::PersistedGlobal<'static, crate::access::BannedPeers, ()>,
  pub creating: crate::database::persisted::PersistedGlobal<'static, crate::access::ServerAccess, crate::access::ServerAccess>,
  #[allow(dead_code)]
  death_rx: tokio::sync::broadcast::Receiver<()>,
  death_tx: tokio::sync::broadcast::Sender<()>,
  pub jwt_decoding_key: jsonwebtoken::DecodingKey,
  pub jwt_encoding_key: jsonwebtoken::EncodingKey,
  pub server_name: std::sync::Arc<str>,
}

impl<T: Copy> AccessSetting<T> {
  pub fn check(&self, player: &spadina_core::player::PlayerIdentifier<impl AsRef<str>>, local_server: &str) -> T {
    spadina_core::access::check_acls(&self.rules, &player.as_ref(), local_server).unwrap_or(self.default)
  }
}
impl<T: Copy + Default> Default for AccessSetting<T> {
  fn default() -> Self {
    Self { default: Default::default(), rules: Vec::new() }
  }
}
#[derive(Debug, Copy, Clone)]
pub(crate) struct Announcements;
#[derive(Debug, Copy, Clone)]
pub(crate) struct BannedPeers;
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub(crate) struct ServerAccess(&'static str, &'static str);

impl AuthNZ {
  pub fn new(
    authentication: Box<dyn crate::auth::AuthProvider>,
    database: &std::sync::Arc<crate::database::Database>,
    server_name: std::sync::Arc<str>,
  ) -> diesel::QueryResult<std::sync::Arc<Self>> {
    let access = crate::database::persisted::PersistedGlobal::new(database.clone(), ServerAccess("s", "access"), &crate::metrics::SERVER_ACL)?;
    let admin = crate::database::persisted::PersistedGlobal::new(database.clone(), ServerAccess("a", "administration"), &crate::metrics::SERVER_ACL)?;
    let creating = crate::database::persisted::PersistedGlobal::new(database.clone(), ServerAccess("c", "create"), &crate::metrics::SERVER_ACL)?;
    let announcements = crate::database::persisted::PersistedWatch::new(database.clone(), Announcements)?;
    let banned_peers = crate::database::persisted::PersistedGlobal::new(database.clone(), BannedPeers, &crate::metrics::BANNED_PEERS)?;
    let (ctrl_c, death_rx) = tokio::sync::broadcast::channel(1);
    let death_tx = ctrl_c.clone();

    tokio::spawn(async move {
      tokio::signal::ctrl_c().await.expect("Failed to handle Ctrl-C");
      ctrl_c.send(()).expect("Failed to notify of shutdown.");
    });
    let (jwt_encoding_key, jwt_decoding_key) = crate::http::jwt::create_jwt();
    Ok(std::sync::Arc::new(Self {
      access,
      admin,
      announcements,
      authentication,
      banned_peers,
      creating,
      death_rx,
      death_tx,
      jwt_encoding_key,
      jwt_decoding_key,
      server_name,
    }))
  }
  pub async fn check_access(&self, location: &'static str, player: &spadina_core::player::PlayerIdentifier<impl AsRef<str>>) -> bool {
    self.access.read(location, |acl| acl.check(player, &self.server_name)).await == spadina_core::access::SimpleAccess::Allow
  }
  pub async fn check_admin(&self, location: &'static str, player: &spadina_core::player::PlayerIdentifier<impl AsRef<str>>) -> bool {
    self.admin.read(location, |acl| acl.check(player, &self.server_name)).await == spadina_core::access::SimpleAccess::Allow
  }
  pub fn give_me_death(&self) -> tokio::sync::broadcast::Receiver<()> {
    self.death_tx.subscribe()
  }
}

impl crate::database::persisted::Persistance for BannedPeers {
  type Value = std::collections::HashSet<spadina_core::access::BannedPeer<String>>;
  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.banned_peers_list()
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.banned_peers_write(value)
  }
}
impl prometheus_client::encoding::EncodeLabelSet for ServerAccess {
  fn encode(&self, mut encoder: prometheus_client::encoding::LabelSetEncoder) -> Result<(), std::fmt::Error> {
    let mut label_encoder = encoder.encode_label();
    let mut label_key_encoder = label_encoder.encode_label_key()?;
    prometheus_client::encoding::EncodeLabelKey::encode(&"category", &mut label_key_encoder)?;
    let mut label_value_encoder = label_key_encoder.encode_label_value()?;
    prometheus_client::encoding::EncodeLabelValue::encode(&self.1, &mut label_value_encoder)?;
    label_value_encoder.finish()
  }
}
impl crate::database::persisted::Persistance for ServerAccess {
  type Value = AccessSetting<spadina_core::access::SimpleAccess>;
  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.acl_read(self.0)
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.acl_write(self.0, value)
  }
}
impl crate::database::persisted::Persistance for Announcements {
  type Value = Vec<spadina_core::communication::Announcement<std::sync::Arc<str>>>;

  fn load(&self, database: &crate::database::Database) -> diesel::result::QueryResult<Self::Value> {
    database.announcements_read()
  }

  fn store(&self, database: &crate::database::Database, value: &Self::Value) -> diesel::result::QueryResult<()> {
    database.announcements_write(value)
  }
}
impl crate::prometheus_locks::LabelledValue<()> for BannedPeers {
  fn labels(&self) -> () {}
}
