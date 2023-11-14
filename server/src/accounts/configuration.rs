use crate::accounts::db_policy::DatabaseBackedPolicy;
use crate::accounts::ldap::{LightweightDirectory, LightweightDirectoryConfiguration};
use crate::accounts::login::openid::configuration::{OIConnectConfiguration, OpenIdRegistration};
use crate::accounts::login::openid::db_oidc::DatabaseOpenIdConnect;
use crate::accounts::login::openid::ServerOpenIdConnect;
use crate::accounts::login::password::db_otp::DatabaseOneTimePasswords;
use crate::accounts::login::password::php_bb::PhpBB;
use crate::accounts::login::password::uru::UruDatabase;
use crate::accounts::login::password::ServerPassword;
use crate::accounts::login::ServerLogin;
use crate::accounts::ServerAccounts;
use crate::database::Database;
use std::collections::BTreeMap;
use std::error::Error;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum AccountsConfiguration {
  DatabaseOTPs { connection: String, database: DatabaseProvider },
  LDAP(LightweightDirectoryConfiguration),
  OpenIdConnect { connection: String, database: DatabaseProvider, providers: Vec<OIConnectConfiguration>, registration: OpenIdRegistration },
  OTPs { users: BTreeMap<String, String> },
  Passwords { users: BTreeMap<String, String> },
  PhpBB { connection: String, database: DatabaseProvider },
  Uru { connection: String },
}
#[derive(serde::Serialize, serde::Deserialize)]
pub enum DatabaseProvider {
  PostgreSQL,
  MySQL,
}
impl AccountsConfiguration {
  /// Parse the configuration string provided to the server into an authentication provider, if possible
  pub async fn load(self, server_name: &str, main_database: &Database) -> Result<ServerAccounts, Box<dyn Error + Send + Sync>> {
    Ok(match self {
      AccountsConfiguration::DatabaseOTPs { connection, database } => match database {
        DatabaseProvider::PostgreSQL => ServerAccounts::Login(
          ServerLogin::Password(ServerPassword::DatabaseOneTimePassword(DatabaseOneTimePasswords::new_postgres(connection)?)),
          DatabaseBackedPolicy::new(main_database)?,
        ),
        #[cfg(feature = "mysql")]
        DatabaseProvider::MySQL => ServerAccounts::Login(
          ServerLogin::Password(ServerPassword::DatabaseOneTimePassword(DatabaseOneTimePasswords::new_mysql(connection)?)),
          DatabaseBackedPolicy::new(main_database)?,
        ),
        #[cfg(not(feature = "mysql"))]
        DatabaseProvider::MySQL => Err("MySQL support not enabled".into()),
      },
      AccountsConfiguration::LDAP(c) => ServerAccounts::LDAP(LightweightDirectory::new(c).await?),
      AccountsConfiguration::OpenIdConnect { connection, database, providers, registration } => {
        let mut clients = BTreeMap::new();
        for provider in providers {
          let (issuer, client) = provider.create_oidc_client(server_name).await?;
          clients.insert(issuer, client);
        }
        match database {
          DatabaseProvider::PostgreSQL => ServerAccounts::Login(
            ServerLogin::OpenID(ServerOpenIdConnect::Database(DatabaseOpenIdConnect::new_postgres(connection, clients)?.into())),
            DatabaseBackedPolicy::new(main_database)?,
          ),
          #[cfg(feature = "mysql")]
          DatabaseProvider::MySQL => ServerAccounts::Login(
            ServerLogin::OpenID(ServerOpenIdConnect::Database(DatabaseOpenIdConnect::new_mysql(connection, clients)?.into())),
            DatabaseBackedPolicy::new(main_database)?,
          ),
          #[cfg(not(feature = "mysql"))]
          DatabaseProvider::MySQL => Err("MySQL support not enabled".into()),
        }
      }
      AccountsConfiguration::OTPs { users } => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::FixedOneTimePassword(users.into())), DatabaseBackedPolicy::new(main_database)?)
      }
      AccountsConfiguration::Passwords { users } => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::FixedPassword(users.into())), DatabaseBackedPolicy::new(main_database)?)
      }
      AccountsConfiguration::PhpBB { connection, database } => match database {
        DatabaseProvider::PostgreSQL => ServerAccounts::Login(
          ServerLogin::Password(ServerPassword::PhpBB(PhpBB::new_postgres(connection)?)),
          DatabaseBackedPolicy::new(main_database)?,
        ),
        #[cfg(feature = "mysql")]
        DatabaseProvider::MySQL => ServerAccounts::Login(
          ServerLogin::Password(ServerPassword::PhpBB(PhpBB::new_mysql(connection)?)),
          DatabaseBackedPolicy::new(main_database)?,
        ),
        #[cfg(not(feature = "mysql"))]
        DatabaseProvider::MySQL => Err("MySQL support not enabled".into()),
      },
      AccountsConfiguration::Uru { connection } => {
        ServerAccounts::Login(ServerLogin::Password(ServerPassword::Uru(UruDatabase::new(connection)?)), DatabaseBackedPolicy::new(main_database)?)
      }
    })
  }
}
