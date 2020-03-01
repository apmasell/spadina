use serde::{Deserialize, Serialize};

/// An announcement that should be visible to users
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Announcement<S: AsRef<str>> {
  /// The summary/title of the event
  pub title: S,
  /// The text that should be displayed
  pub body: S,
  /// The time when the event described will start
  pub when: AnnouncementTime,
  /// The location where the event will be held
  pub realm: Option<crate::realm::RealmTarget<S>>,
  /// The announcement is visible on the public calendar (i.e., it can be seen without logging in)
  pub public: bool,
}
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone)]
pub enum AnnouncementTime {
  /// The event has no start, but the announcement expires
  Until(chrono::DateTime<chrono::Utc>),
  /// The event starts a particular time and lasts a certain number of minutes
  Starts(chrono::DateTime<chrono::Utc>, u32),
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Bookmark<S: AsRef<str>> {
  Asset { kind: S, asset: S },
  Player { player: crate::player::PlayerIdentifier<S>, notes: S },
  Realm(crate::realm::RealmTarget<S>),
  Server(S),
}

/// Information about direct messages between this player and another
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectMessage<S: AsRef<str>> {
  /// Whether the direction was to the player or from the player
  pub inbound: bool,
  /// The contents of the message
  pub body: MessageBody<S>,
  /// The time the message was sent
  pub timestamp: chrono::DateTime<chrono::Utc>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectMessageInfo {
  pub last_received: chrono::DateTime<chrono::Utc>,
  pub last_read: Option<chrono::DateTime<chrono::Utc>>,
}
/// The status of a direct message after sending
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DirectMessageStatus {
  /// The message was written to user's inbox
  Delivered(chrono::DateTime<chrono::Utc>),
  /// The recipient is invalid
  UnknownRecipient,
  /// The message was placed in a queue to send to a remote server. More delivery information may follow.
  Queued(chrono::DateTime<chrono::Utc>),
  /// The player isn't allowed to send direct messages yet
  Forbidden,
  /// An error occurred on the server while sending the message
  InternalError,
}
/// The result of attempting to create an invitation
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum InvitationError {
  /// The user doesn't have that power
  NotAllowed,
  /// This server does not support creating invitations
  Closed,
  /// A server error occurred trying to create the realm
  InternalError,
}
#[derive(Clone, Serialize, Deserialize, Debug, Hash, Eq, PartialEq)]
pub enum MessageBody<S: AsRef<str>> {
  Announcement { title: S, body: S, when: AnnouncementTime, realm: Option<crate::realm::RealmTarget<S>> },
  Asset(S),
  Delete(chrono::DateTime<chrono::Utc>),
  Edit(chrono::DateTime<chrono::Utc>, S),
  Player(crate::player::PlayerIdentifier<S>),
  Reaction(chrono::DateTime<chrono::Utc>, char),
  Read,
  Realm(crate::realm::RealmTarget<S>),
  Reply(chrono::DateTime<chrono::Utc>, S),
  Time(chrono::DateTime<chrono::Utc>),
  Text(S),
  Typing,
}

impl AnnouncementTime {
  pub fn expires(&self) -> chrono::DateTime<chrono::Utc> {
    match self {
      AnnouncementTime::Until(t) => *t,
      AnnouncementTime::Starts(t, m) => *t + chrono::Duration::minutes(*m as i64),
    }
  }
}

impl<S: AsRef<str>> Bookmark<S> {
  pub fn as_ref(&self) -> Bookmark<&'_ str> {
    match self {
      Bookmark::Asset { kind, asset } => Bookmark::Asset { kind: kind.as_ref(), asset: asset.as_ref() },
      Bookmark::Player { player, notes } => Bookmark::Player { player: player.as_ref(), notes: notes.as_ref() },
      Bookmark::Realm(realm) => Bookmark::Realm(realm.as_ref()),
      Bookmark::Server(server) => Bookmark::Server(server.as_ref()),
    }
  }
}
impl Bookmark<String> {
  pub fn localize(self, local_server: &str) -> Option<Self> {
    match self {
      Bookmark::Asset { kind, asset } => Some(Bookmark::Asset { kind, asset }),
      Bookmark::Player { player, notes } => Some(Bookmark::Player { player: player.localize(local_server), notes }),
      Bookmark::Realm(realm) => Some(Bookmark::Realm(realm.localize(local_server))),
      Bookmark::Server(name) => match crate::net::parse_server_name(&name) {
        None => None,
        Some(name) => {
          if name.as_str() == local_server {
            None
          } else {
            Some(Bookmark::Server(name))
          }
        }
      },
    }
  }
}
impl<S: AsRef<str>> DirectMessage<S> {
  pub fn as_ref<'a>(&'a self) -> DirectMessage<&'a str> {
    DirectMessage { inbound: self.inbound, body: self.body.as_ref(), timestamp: self.timestamp }
  }
  pub fn convert_str<T: AsRef<str>>(self) -> DirectMessage<T>
  where
    S: Into<T>,
  {
    DirectMessage { inbound: self.inbound, body: self.body.convert_str(), timestamp: self.timestamp }
  }
}
impl<S: AsRef<str>> MessageBody<S> {
  pub fn as_owned_str(&self) -> MessageBody<String> {
    match self {
      MessageBody::Announcement { title, body, when, realm } => MessageBody::Announcement {
        title: title.as_ref().to_string(),
        body: body.as_ref().to_string(),
        when: when.clone(),
        realm: realm.as_ref().map(|r| r.as_owned_str()),
      },
      MessageBody::Asset(a) => MessageBody::Asset(a.as_ref().to_string()),
      MessageBody::Delete(ts) => MessageBody::Delete(*ts),
      MessageBody::Edit(ts, t) => MessageBody::Edit(*ts, t.as_ref().to_string()),
      MessageBody::Player(player) => MessageBody::Player(player.as_owned_str()),
      MessageBody::Reaction(ts, emoji) => MessageBody::Reaction(*ts, *emoji),
      MessageBody::Read => MessageBody::Read,
      MessageBody::Realm(realm) => MessageBody::Realm(realm.as_owned_str()),
      MessageBody::Reply(ts, t) => MessageBody::Reply(*ts, t.as_ref().to_string()),
      MessageBody::Time(ts) => MessageBody::Time(*ts),
      MessageBody::Text(t) => MessageBody::Text(t.as_ref().to_string()),
      MessageBody::Typing => MessageBody::Typing,
    }
  }
  pub fn as_ref<'a>(&'a self) -> MessageBody<&'a str> {
    match self {
      MessageBody::Announcement { title, body, when, realm } => {
        MessageBody::Announcement { title: title.as_ref(), body: body.as_ref(), when: when.clone(), realm: realm.as_ref().map(|r| r.as_ref()) }
      }
      MessageBody::Asset(a) => MessageBody::Asset(a.as_ref()),
      MessageBody::Delete(ts) => MessageBody::Delete(*ts),
      MessageBody::Edit(ts, t) => MessageBody::Edit(*ts, t.as_ref()),
      MessageBody::Player(player) => MessageBody::Player(player.as_ref()),
      MessageBody::Reaction(ts, emoji) => MessageBody::Reaction(*ts, *emoji),
      MessageBody::Read => MessageBody::Read,
      MessageBody::Realm(realm) => MessageBody::Realm(realm.as_ref()),
      MessageBody::Reply(ts, t) => MessageBody::Reply(*ts, t.as_ref()),
      MessageBody::Time(ts) => MessageBody::Time(*ts),
      MessageBody::Text(t) => MessageBody::Text(t.as_ref()),
      MessageBody::Typing => MessageBody::Typing,
    }
  }
  pub fn convert_str<T: AsRef<str>>(self) -> MessageBody<T>
  where
    S: Into<T>,
  {
    match self {
      MessageBody::Announcement { title, body, when, realm } => {
        MessageBody::Announcement { title: title.into(), body: body.into(), when: when, realm: realm.map(|r| r.convert_str()) }
      }
      MessageBody::Asset(a) => MessageBody::Asset(a.into()),
      MessageBody::Delete(ts) => MessageBody::Delete(ts),
      MessageBody::Edit(ts, t) => MessageBody::Edit(ts, t.into()),
      MessageBody::Player(player) => MessageBody::Player(player.convert_str()),
      MessageBody::Reaction(ts, emoji) => MessageBody::Reaction(ts, emoji),
      MessageBody::Read => MessageBody::Read,
      MessageBody::Realm(realm) => MessageBody::Realm(realm.convert_str()),
      MessageBody::Reply(ts, t) => MessageBody::Reply(ts, t.into()),
      MessageBody::Time(ts) => MessageBody::Time(ts),
      MessageBody::Text(t) => MessageBody::Text(t.into()),
      MessageBody::Typing => MessageBody::Typing,
    }
  }
  pub fn is_transient(&self) -> bool {
    match self {
      MessageBody::Read | MessageBody::Typing => true,
      _ => false,
    }
  }
  pub fn is_valid_at(&self, now: &chrono::DateTime<chrono::Utc>) -> bool {
    match self {
      MessageBody::Announcement { when, .. } => when.expires() > *now,
      MessageBody::Asset(_) => true,
      MessageBody::Delete(ts) => ts < now,
      MessageBody::Edit(ts, _) => ts < now,
      MessageBody::Player(_) => true,
      MessageBody::Reaction(ts, _) => ts < now,
      MessageBody::Read => true,
      MessageBody::Realm(_) => true,
      MessageBody::Reply(ts, _) => ts < now,
      MessageBody::Time(_) => true,
      MessageBody::Text(_) => true,
      MessageBody::Typing => true,
    }
  }
}
