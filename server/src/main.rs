#![feature(map_try_insert)]
#![feature(map_many_mut)]
#![feature(try_blocks)]
extern crate core;

use crate::access::AccessManagement;
use crate::directory::Directory;
use spadina_core::net::parse_server_name;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

mod access;
mod accounts;
mod aggregating_map;
mod asset_store;
mod atomic_activity;
mod client;
mod config;
mod database;
mod directory;
mod gc_map;
mod html;
mod http_server;
mod join_request;
mod location_search;
mod metrics;
mod peer;
mod player_event;
mod player_location_update;
mod prometheus_future;
mod prometheus_locks;
mod server_controller_template;
mod socket_entity;
mod stream_map;
mod unix_socket;

/// Start the server. This is in a separate function from main because the tokio annotation mangles compile error information
async fn start() -> Result<(), Box<dyn Error + Send + Sync>> {
  let (configuration, db_path) = config::ServerConfiguration::load();
  let server_name: Arc<str> = Arc::from(parse_server_name(&configuration.name).expect("Invalid server name. It must be a valid DNS name"));
  let database = database::Database::new(db_path);
  database.player_clean()?;
  let auth = AccessManagement::new(configuration.authentication.load(&server_name, &database).await?, &database, server_name)?;
  let asset_store = configuration.asset_store.load();
  let directory = Directory::new(auth, asset_store, database.clone());

  if let Some(path) = configuration.unix_socket {
    eprintln!("Starting UNIX socket monitor on {}", &path);
    unix_socket::start(path, directory.clone());
  }
  start_cleaner_task(&directory.access_management, database.clone(), directory.clone());

  http_server::ssl::start(http_server::WebServer::new(directory, database), configuration.certificate, configuration.bind_address).await?;
  Ok(())
}
fn start_cleaner_task(auth: &AccessManagement, database: database::Database, directory: Directory) {
  let mut death = auth.give_me_death();
  tokio::spawn(async move {
    loop {
      tokio::select! {
        biased;
        _ = death.recv() => break,
        _ = sleep(Duration::from_secs(600)) => ()
      }
      if let Err(e) = database.direct_message_clean() {
        eprintln!("Failed to delete old chats: {}", e);
      }
      if let Err(e) = database.location_announcements_clean() {
        eprintln!("Failed to delete old announcements: {}", e);
      }
      match database.calender_cache_refresh() {
        Ok(updates) => directory.refresh_calendars(updates).await,
        Err(e) => {
          eprintln!("Failed to refresh calendars: {}", e);
        }
      }
    }
  });
}

// Actual main method. The tokio::main annotation causes all compile errors in the body to be on the line with the annotation, so keep this short
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
  start().await
}
