use crate::accounts::db_policy::DatabaseBackedPolicy;
use crate::accounts::ldap::{LightweightDirectory, LightweightDirectoryConfiguration};
use crate::accounts::login::openid::configuration::{OIConnectConfiguration, OpenIdRegistration};
use crate::accounts::login::openid::db_oidc::DatabaseOpenIdConnect;
use crate::accounts::login::openid::ServerOpenIdConnect;
use crate::accounts::login::password::db_otp::DatabaseOneTimePasswords;
use crate::accounts::login::password::uru::UruDatabase;
use crate::accounts::login::password::ServerPassword;
use crate::accounts::login::ServerLogin;
use crate::accounts::ServerAccounts;
use crate::database::connect::DatabaseBackend;
use crate::database::Database;
use std::collections::BTreeMap;
use std::error::Error;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AccountsConfiguration {
  DatabaseOTPs(String),
  LDAP(LightweightDirectoryConfiguration),
  OpenIdConnect { connection: String, providers: Vec<OIConnectConfiguration>, registration: Option<OpenIdRegistration> },
  OTPs(BTreeMap<String, String>),
  Passwords(BTreeMap<String, String>),
  PhpBB(String),
  Uru(String),
}

impl AccountsConfiguration {
  /// Parse the configuration string provided to the server into an authentication provider, if possible
  pub async fn load(self, server_name: &str, main_database: &Database) -> Result<ServerAccounts, Box<dyn Error + Send + Sync>> {
    Ok(match self {
      AccountsConfiguration::DatabaseOTPs(connection) => ServerAccounts::Login(
        ServerLogin::Password(ServerPassword::DatabaseOneTimePassword(DatabaseOneTimePasswords::try_from(DatabaseBackend::try_connect(
          connection,
        )?)?)),
        DatabaseBackedPolicy::new(main_database)?,
      ),
      AccountsConfiguration::LDAP(c) => ServerAccounts::LDAP(LightweightDirectory::new(c).await?),
      AccountsConfiguration::OpenIdConnect { connection, providers, registration } => {
        let mut clients = BTreeMap::new();
        for provider in providers {
          let (issuer, client) = provider.create_oidc_client(server_name).await?;
          clients.insert(issuer, client);
        }
        ServerAccounts::Login(
          ServerLogin::OpenID(ServerOpenIdConnect::Database(DatabaseOpenIdConnect::new(DatabaseBackend::try_connect(connection)?, clients)?.into())),
          DatabaseBackedPolicy::new(main_database)?,
        )
      }
      AccountsConfiguration::OTPs(users) => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::FixedOneTimePassword(users.into())), DatabaseBackedPolicy::new(main_database)?)
      }
      AccountsConfiguration::Passwords(users) => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::FixedPassword(users.into())), DatabaseBackedPolicy::new(main_database)?)
      }
      AccountsConfiguration::PhpBB(connection) => ServerAccounts::Login(
        ServerLogin::Password(ServerPassword::PhpBB(DatabaseBackend::try_connect(connection)?.into())),
        DatabaseBackedPolicy::new(main_database)?,
      ),
      AccountsConfiguration::Uru(connection) => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::Uru(UruDatabase::new(connection)?)), DatabaseBackedPolicy::new(main_database)?)
      }
    })
  }
}
