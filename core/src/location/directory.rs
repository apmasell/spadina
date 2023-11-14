use crate::reference_converter::{Converter, Referencer};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How much activity is there in a realm
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum Activity {
  /// Activity information not available
  Unknown,
  /// No players in recent time
  Deserted,
  /// Some players with low chat volume
  Quiet,
  /// Some players with moderate chat volume
  Popular,
  /// Lots of players with moderate chat volume
  Busy,
  /// Lots of players with high chat volume
  Crowded,
}
/// A realm the player can access
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectoryEntry<S: AsRef<str>> {
  /// The location identifier
  pub descriptor: super::Descriptor<S>,
  /// The friendly name for this realm
  pub name: S,
  /// How busy the realm is
  pub activity: Activity,
  /// The player that owns this realm
  pub owner: S,
  /// The server that hosts this realm (or none of the local server)
  pub server: S,
  /// The last time the location was modified
  pub updated: DateTime<Utc>,
  /// The time when the location was created
  pub created: DateTime<Utc>,
  /// The visibility of the realm
  pub visibility: Visibility,
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash, int_enum::IntEnum)]
#[repr(i16)]
pub enum Visibility {
  Public = 0,
  Private = 1,
  Archived = 2,
  Trashed = 4,
}
/// When fetching realms from the server, what kind of realms to fetch
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Search<S: AsRef<str>> {
  /// Realms in the player's bookmark list
  Bookmarks,
  Calendar,
  /// Realms owned by the user
  Personal(Vec<Visibility>),
  PersonalSearch {
    query: SearchCriteria<S>,
    visibility: Vec<Visibility>,
    player: Option<S>,
  },
  /// Realms marked as public on the local server
  PublicLocal,
  /// Public realms on a remote server
  PublicRemote(S),
  PublicSearch {
    query: SearchCriteria<S>,
    server: Option<S>,
  },
}
/// When fetching realms from the server, what kind of realms to fetch
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SearchCriteria<S: AsRef<str>> {
  And(Vec<SearchCriteria<S>>),
  Created(TimeRange),
  Kind(super::DescriptorKind<S>),
  NameContains { text: S, case_sensitive: bool },
  Not(Box<SearchCriteria<S>>),
  Or(Vec<SearchCriteria<S>>),
  Player(S),
  Updated(TimeRange),
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimeRange {
  After(DateTime<Utc>),
  Before(DateTime<Utc>),
  In(DateTime<Utc>, DateTime<Utc>),
}
impl<S: AsRef<str>> DirectoryEntry<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> DirectoryEntry<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    DirectoryEntry {
      descriptor: self.descriptor.reference(reference),
      name: reference.convert(&self.name),
      activity: self.activity,
      owner: reference.convert(&self.owner),
      server: reference.convert(&self.server),
      created: self.created,
      updated: self.updated,
      visibility: self.visibility,
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> DirectoryEntry<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    DirectoryEntry {
      descriptor: self.descriptor.convert(converter),
      name: converter.convert(self.name),
      activity: self.activity,
      owner: converter.convert(self.owner),
      server: converter.convert(self.server),
      created: self.created,
      updated: self.updated,
      visibility: self.visibility,
    }
  }
}
impl<S: AsRef<str>> Search<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> Search<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      Search::Bookmarks => Search::Bookmarks,
      Search::Calendar => Search::Calendar,
      Search::Personal(visibility) => Search::Personal(visibility.clone()),
      Search::PersonalSearch { query, visibility, player } => Search::PersonalSearch {
        query: query.reference(reference),
        visibility: visibility.clone(),
        player: player.as_ref().map(|p| reference.convert(p)),
      },
      Search::PublicLocal => Search::PublicLocal,
      Search::PublicRemote(server) => Search::PublicRemote(reference.convert(server)),
      Search::PublicSearch { query, server } => {
        Search::PublicSearch { query: query.reference(reference), server: server.as_ref().map(|s| reference.convert(s)) }
      }
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> Search<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      Search::Bookmarks => Search::Bookmarks,
      Search::Calendar => Search::Calendar,
      Search::Personal(visibility) => Search::Personal(visibility),
      Search::PersonalSearch { query, visibility, player } => {
        Search::PersonalSearch { query: query.convert(converter), visibility, player: player.map(|p| converter.convert(p)) }
      }
      Search::PublicLocal => Search::PublicLocal,
      Search::PublicRemote(server) => Search::PublicRemote(converter.convert(server)),
      Search::PublicSearch { query, server } => {
        Search::PublicSearch { query: query.convert(converter), server: server.map(|s| converter.convert(s)) }
      }
    }
  }
}
impl<S: AsRef<str>> SearchCriteria<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> SearchCriteria<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      SearchCriteria::And(criteria) => SearchCriteria::And(criteria.iter().map(|s| s.reference(reference)).collect()),
      SearchCriteria::Created(value) => SearchCriteria::Created(value.clone()),
      SearchCriteria::Kind(kind) => SearchCriteria::Kind(kind.reference(reference)),
      SearchCriteria::NameContains { text, case_sensitive } => {
        SearchCriteria::NameContains { text: reference.convert(text), case_sensitive: *case_sensitive }
      }
      SearchCriteria::Not(value) => SearchCriteria::Not(Box::new(value.reference(reference))),
      SearchCriteria::Or(criteria) => SearchCriteria::Or(criteria.iter().map(|c| c.reference(reference)).collect()),
      SearchCriteria::Player(p) => SearchCriteria::Player(reference.convert(p)),
      SearchCriteria::Updated(value) => SearchCriteria::Updated(value.clone()),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> SearchCriteria<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      SearchCriteria::And(criteria) => SearchCriteria::And(criteria.into_iter().map(|s| s.convert(converter)).collect()),
      SearchCriteria::Created(value) => SearchCriteria::Created(value),
      SearchCriteria::Kind(kind) => SearchCriteria::Kind(kind.convert(converter)),
      SearchCriteria::NameContains { text, case_sensitive } => SearchCriteria::NameContains { text: converter.convert(text), case_sensitive },
      SearchCriteria::Not(value) => SearchCriteria::Not(Box::new(value.convert(converter))),
      SearchCriteria::Or(criteria) => SearchCriteria::Or(criteria.into_iter().map(|c| c.convert(converter)).collect()),
      SearchCriteria::Player(p) => SearchCriteria::Player(converter.convert(p)),
      SearchCriteria::Updated(value) => SearchCriteria::Updated(value),
    }
  }
}
impl Visibility {
  pub fn is_writable(&self) -> bool {
    match self {
      Visibility::Public | Visibility::Private => true,
      Visibility::Archived | Visibility::Trashed => false,
    }
  }
}
