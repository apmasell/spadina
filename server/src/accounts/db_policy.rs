use crate::accounts::policy::{Policy, PolicyRequest};
use crate::database::persisted::PersistedGlobal;
use crate::database::setting::Setting;
use crate::database::Database;
use crate::metrics::SettingLabel;
use diesel::result::QueryResult;
use spadina_core::access::{LocalAccessSetting, SimpleAccess};
use spadina_core::UpdateResult;
use std::future::Future;

#[derive(Debug, Copy, Clone)]
struct CreateAssets;
#[derive(Debug, Copy, Clone)]
struct ServerAdministration;

pub struct DatabaseBackedPolicy {
  admin: PersistedGlobal<'static, ServerAdministration, SettingLabel>,
  creating: PersistedGlobal<'static, CreateAssets, SettingLabel>,
}

impl DatabaseBackedPolicy {
  pub fn new(database: &Database) -> QueryResult<Self> {
    Ok(Self {
      admin: PersistedGlobal::new(database.clone(), ServerAdministration, &crate::metrics::SETTING)?,
      creating: PersistedGlobal::new(database.clone(), CreateAssets, &crate::metrics::SETTING)?,
    })
  }
}

impl Policy for DatabaseBackedPolicy {
  fn can_create(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move { self.creating.read("check_create", |acl| acl.check(player) == SimpleAccess::Allow).await }
  }

  fn is_administrator(&self, player: &str) -> impl Future<Output = bool> + Send {
    async move { self.creating.read("check_is_admin", |acl| acl.check(player) == SimpleAccess::Allow).await }
  }

  fn request(&self, request: PolicyRequest) -> impl Future<Output = UpdateResult> + Send {
    async move {
      match request {
        PolicyRequest::AddAdmin(player) => self.admin.write("request", |acl| Some(acl.allow(player))).await,
        PolicyRequest::AddCreator(player) => self.creating.write("request", |acl| Some(acl.allow(player))).await,
        PolicyRequest::RemoveAdmin(player) => self.admin.write("request", |acl| Some(acl.deny(player))).await,
        PolicyRequest::RemoveCreator(player) => self.creating.write("request", |acl| Some(acl.deny(player))).await,
        PolicyRequest::SetCreator(default) => self.admin.write("request", |acl| Some(acl.reset(default))).await,
        PolicyRequest::SetAdmin(default) => self.creating.write("request", |acl| Some(acl.reset(default))).await,
      }
    }
  }
}

impl Setting for CreateAssets {
  const CODE: u8 = b'c';
  const METRIC: &'static str = "create_assets";
  type Stored = LocalAccessSetting<String>;
}

impl Setting for ServerAdministration {
  const CODE: u8 = b'A';
  const METRIC: &'static str = "server_administration";
  type Stored = LocalAccessSetting<String>;
}
