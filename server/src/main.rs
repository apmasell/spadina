#![feature(map_try_insert)]
#![feature(map_many_mut)]
mod access;
mod asset_store;
mod auth;
mod avatar;
mod client;
mod config;
mod database;
mod destination;
mod html;
mod http;
mod map;
mod metrics;
mod peer;
mod prometheus_locks;
mod realm;
mod shstr;
mod unix_socket;

/// Start the server. This is in a separate function from main because the tokio annotation mangles compile error information
async fn start() -> Result<(), Box<dyn std::error::Error>> {
  let configuration = config::ServerConfiguration::load();
  let server_name: std::sync::Arc<str> = std::sync::Arc::from(configuration.name);
  let asset_store = configuration.asset_store.load();
  if let Some(default_realm) = configuration.default_realm.as_ref() {
    match asset_store.pull(default_realm).await {
      Ok(realm_asset) => match spadina_core::asset::AssetAnyRealm::<String>::load(realm_asset, &asset_store).await {
        Ok((realm, _)) => {
          let propagation_rules: Vec<spadina_core::asset::rules::PropagationRule<spadina_core::asset::SimpleRealmPuzzleId<String>, String>> =
            match realm {
              spadina_core::asset::AssetAnyRealm::Simple(r) => r.propagation_rules,
            };
          if !propagation_rules.iter().any(|rule| match rule.propagation_match {
            spadina_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext => true,
            _ => false,
          }) {
            return Err(format!("Default realm {} is not a valid train realm", default_realm).into());
          }
        }
        Err(e) => {
          return Err(format!("Cannot load default realm {}: {}", default_realm, e).into());
        }
      },
      Err(e) => {
        return Err(format!("Cannot load default realm {}: {}", default_realm, e).into());
      }
    }
  }

  let database = std::sync::Arc::new(crate::database::Database::new(&configuration.database_url, configuration.default_realm.as_ref()));
  database.player_clean()?;
  let authnz = access::AuthNZ::new(configuration.authentication.load(&server_name).await?, &database, server_name.clone())?;
  let directory = destination::Directory::new(database.clone(), authnz.clone(), asset_store).await;

  if let Some(path) = configuration.unix_socket {
    unix_socket::start(path, std::sync::Arc::downgrade(&directory), authnz.clone(), database.clone());
  }
  start_message_cleaner_task(&database);

  http::ssl::start(std::sync::Arc::new(http::WebServer::new(authnz, directory, database)), configuration.certificate, configuration.bind_address)
    .await?;
  Ok(())
}
fn start_message_cleaner_task(database: &std::sync::Arc<crate::database::Database>) {
  let database = std::sync::Arc::downgrade(database);
  tokio::spawn(async move {
    let mut counter: u32 = 0;
    loop {
      tokio::time::sleep(std::time::Duration::from_secs(1)).await;
      match database.upgrade() {
        Some(database) => {
          counter += 1;
          if counter > 600 {
            counter = 0;
            if let Err(e) = database.direct_message_clean() {
              eprintln!("Failed to delete old chats: {}", e);
            }
            if let Err(e) = database.realm_announcements_clean() {
              eprintln!("Failed to delete old announcements: {}", e);
            }
          }
        }
        None => break,
      }
    }
  });
}

// Actual main method. The tokio::main annotation causes all compile errors in the body to be on the line with the annotation, so keep this short
#[tokio::main(worker_threads = 8)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  start().await
}
