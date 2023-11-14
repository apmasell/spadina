use crate::location_search::LocationRecipient;
use crate::peer::message::PeerMessage;
use crate::peer::Peer;
use chrono::{DateTime, Utc};
use serde::Serialize;
use spadina_core::location::directory::DirectoryEntry;
use spadina_core::tracking_map::Expires;
use std::hash::Hash;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;

pub struct ActiveSearch {
  results: watch::Sender<Vec<DirectoryEntry<String>>>,
  max_limit: DateTime<Utc>,
}

impl ActiveSearch {
  pub fn new(results: watch::Sender<Vec<DirectoryEntry<String>>>, duration: chrono::Duration) -> Self {
    ActiveSearch { results, max_limit: Utc::now() + duration }
  }
  pub fn send(&mut self, locations: Vec<DirectoryEntry<String>>) {
    if self.results.send(locations).is_err() {
      self.max_limit = DateTime::<Utc>::MIN_UTC;
    }
  }
}

impl Expires for ActiveSearch {
  fn end_of_life(&self) -> DateTime<Utc> {
    self.max_limit
  }
}

#[derive(Copy, Clone)]
pub struct SearchRequest(pub u32);

impl LocationRecipient for SearchRequest {
  type Receiver = Peer;

  fn encode(&self, locations: Vec<DirectoryEntry<impl AsRef<str> + Eq + Hash + Ord + Serialize>>) -> Message {
    PeerMessage::<_, &[u8]>::LocationsAvailable { id: self.0, locations }.into()
  }

  fn fail(&self) -> Message {
    PeerMessage::<&str, &[u8]>::LocationsUnavailable { id: self.0 }.into()
  }
}
