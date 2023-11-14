use crate::client::Client;
use crate::database::location_scope::LocationListScope;
use crate::database::player_reference::PlayerReference;
use crate::location_search::LocationRecipient;
use crate::peer::message::PeerLocationSearch;
use serde::Serialize;
use spadina_core::location::directory::{DirectoryEntry, Search, Visibility};
use spadina_core::net::server::ClientResponse;
use std::hash::Hash;
use tokio_tungstenite::tungstenite::Message;

pub enum ReifiedSearch {
  Bookmarks,
  Calendar,
  Database(LocationListScope<String>, bool),
  Remote(String, PeerLocationSearch<String>),
}

impl ReifiedSearch {
  pub fn convert(search: Search<String>, player_name: &str, db_id: i32, local_server: &str) -> Self {
    match search {
      Search::Personal(visibility) => ReifiedSearch::Database(
        LocationListScope::And(vec![LocationListScope::Owner(PlayerReference::Id(db_id)), LocationListScope::Visibility(visibility)]),
        false,
      ),
      Search::Bookmarks => ReifiedSearch::Bookmarks,
      Search::Calendar => ReifiedSearch::Calendar,
      Search::PublicLocal => ReifiedSearch::Database(LocationListScope::Visibility(vec![Visibility::Public]), false),
      Search::PublicRemote(server) => {
        if &*server == local_server {
          ReifiedSearch::Database(LocationListScope::Visibility(vec![Visibility::Public]), false)
        } else {
          ReifiedSearch::Remote(server, PeerLocationSearch::Public)
        }
      }
      Search::PersonalSearch { query, visibility, player } => match player.filter(|s| &*s != player_name) {
        None => ReifiedSearch::Database(
          LocationListScope::And(vec![LocationListScope::Owner(PlayerReference::Id(db_id)), LocationListScope::Visibility(visibility), query.into()]),
          false,
        ),
        Some(player) => ReifiedSearch::Database(
          LocationListScope::And(vec![
            LocationListScope::Owner(PlayerReference::Name(player)),
            LocationListScope::Visibility(visibility),
            query.into(),
          ]),
          true,
        ),
      },
      Search::PublicSearch { query, server } => match server.filter(|s| &*s != local_server) {
        None => ReifiedSearch::Database(LocationListScope::And(vec![LocationListScope::Visibility(vec![Visibility::Public]), query.into()]), false),
        Some(server) => ReifiedSearch::Remote(server, PeerLocationSearch::Search { query }),
      },
    }
  }
}
#[derive(Copy, Clone)]
pub struct SearchRequest(pub u32);
impl LocationRecipient for SearchRequest {
  type Receiver = Client;

  fn encode(&self, locations: Vec<DirectoryEntry<impl AsRef<str> + Eq + Hash + Ord + Serialize>>) -> Message {
    ClientResponse::<_, &[u8]>::LocationsAvailable { id: self.0, locations }.into()
  }

  fn fail(&self) -> Message {
    ClientResponse::<&str, &[u8]>::LocationsUnavailable { id: self.0, server: None }.into()
  }
}
