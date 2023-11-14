use crate::access::{AccessControl, Privilege};
use crate::avatar::Avatar;
use crate::location::communication;
use crate::player::PlayerIdentifier;
use crate::reference_converter::{Converter, Referencer};
use crate::UpdateResult;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum KickResult {
  Success,
  NotAllowed,
  NotPresent,
}

/// A request from the player to the location
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocationRequest<S: AsRef<str>, B> {
  /// Get the ACLs for a location.
  AccessGet,
  /// Set the ACLs for a location or communication settings. The client must generate a unique ID
  /// that the server will respond with if the ACLs can be updated.
  AccessSet {
    id: i32,
    rules: Vec<AccessControl<S, Privilege>>,
    default: Privilege,
  },
  /// Adds an announcement to the announcement list
  AnnouncementAdd {
    id: i32,
    announcement: communication::Announcement<S>,
  },
  /// Clears all announcements
  AnnouncementClear {
    id: i32,
  },
  /// Fetches the announcements list (though they are sent unsolicited upon change)
  AnnouncementList,
  /// Change the name. The user must have admin rights. If either of these is optional, it is not modified.
  ChangeName {
    id: i32,
    name: S,
  },

  /// Destroys the location
  Delete,

  /// Kick a player out. Requires admin privileges. It doesn't prevent them from rejoining; if that is desired, modify the access control.
  Kick {
    id: i32,
    target: PlayerIdentifier<S>,
  },
  /// Erases messages from a particular time range
  MessageClear {
    id: i32,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// A message was posted in the current realm's chat.
  MessageSend {
    body: crate::communication::MessageBody<S>,
  },
  /// A collection of messages when a time range was queried
  MessagesGet {
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  Internal(i32, B),
}

/// A message from the server about the current realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocationResponse<S: AsRef<str>, B> {
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange {
    id: i32,
    result: UpdateResult,
  },
  /// The current ACLs associated with a realm
  AccessCurrent {
    rules: Vec<AccessControl<S, Privilege>>,
    default: Privilege,
  },
  /// The realm announcements have changed
  Announcements(Vec<communication::Announcement<S>>),
  /// Whether updating the announcements (either set or clear) was successful (true) or failed (false)
  AnnouncementUpdate {
    id: i32,
    result: UpdateResult,
  },
  AvatarUpdate {
    avatars: Vec<(PlayerIdentifier<S>, Avatar)>,
  },
  Kick {
    id: i32,
    result: KickResult,
  },
  MessageClear {
    id: i32,
    result: UpdateResult,
  },
  MessagePosted(communication::ChatMessage<S>),
  Messages {
    messages: Vec<communication::ChatMessage<S>>,
    to: chrono::DateTime<chrono::Utc>,
    from: chrono::DateTime<chrono::Utc>,
  },
  NameChange {
    id: i32,
    result: UpdateResult,
  },
  /// The realm's name and/or directory listing status has been changed
  NameChanged {
    name: S,
  },
  RequestError {
    id: i32,
  },
  Internal(B),
}

impl<S: AsRef<str>, B: AsRef<[u8]>> LocationRequest<S, B> {
  pub fn convert<C: Converter<S> + Converter<B>>(self, converter: C) -> LocationRequest<<C as Converter<S>>::Output, <C as Converter<B>>::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
    <C as Converter<B>>::Output: AsRef<[u8]>,
  {
    match self {
      LocationRequest::AccessGet => LocationRequest::AccessGet,
      LocationRequest::AccessSet { id, rules, default } => {
        LocationRequest::AccessSet { id, rules: rules.into_iter().map(|rule| rule.convert(converter)).collect(), default }
      }
      LocationRequest::AnnouncementAdd { id, announcement } => LocationRequest::AnnouncementAdd { id, announcement: announcement.convert(converter) },
      LocationRequest::AnnouncementClear { id } => LocationRequest::AnnouncementClear { id },
      LocationRequest::AnnouncementList => LocationRequest::AnnouncementList,
      LocationRequest::ChangeName { id, name } => LocationRequest::ChangeName { id, name: converter.convert(name) },
      LocationRequest::Delete => LocationRequest::Delete,
      LocationRequest::Internal(id, data) => LocationRequest::Internal(id, converter.convert(data)),
      LocationRequest::Kick { id, target } => LocationRequest::Kick { id, target: target.convert(converter) },
      LocationRequest::MessageClear { id, from, to } => LocationRequest::MessageClear { id, from, to },
      LocationRequest::MessageSend { body } => LocationRequest::MessageSend { body: body.convert(converter) },
      LocationRequest::MessagesGet { from, to } => LocationRequest::MessagesGet { from, to },
    }
  }
  pub fn reference<'a, R: Referencer<S> + Referencer<B>>(
    &'a self,
    reference: R,
  ) -> LocationRequest<<R as Referencer<S>>::Output<'a>, <R as Referencer<B>>::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
    <R as Referencer<B>>::Output<'a>: AsRef<[u8]>,
  {
    match self {
      LocationRequest::AccessGet => LocationRequest::AccessGet,
      LocationRequest::AccessSet { id, rules, default } => {
        LocationRequest::AccessSet { id: *id, rules: rules.iter().map(|r| r.reference(reference)).collect(), default: *default }
      }
      LocationRequest::AnnouncementAdd { id, announcement } => {
        LocationRequest::AnnouncementAdd { id: *id, announcement: announcement.reference(reference) }
      }
      LocationRequest::AnnouncementClear { id } => LocationRequest::AnnouncementClear { id: *id },
      LocationRequest::AnnouncementList => LocationRequest::AnnouncementList,
      LocationRequest::ChangeName { id, name } => LocationRequest::ChangeName { id: *id, name: reference.convert(name) },
      LocationRequest::Delete => LocationRequest::Delete,
      LocationRequest::Internal(id, data) => LocationRequest::Internal(*id, reference.convert(data)),
      LocationRequest::Kick { id, target } => LocationRequest::Kick { id: *id, target: target.reference(reference) },
      LocationRequest::MessageClear { id, from, to } => LocationRequest::MessageClear { id: *id, from: *from, to: *to },
      LocationRequest::MessageSend { body } => LocationRequest::MessageSend { body: body.reference(reference) },
      LocationRequest::MessagesGet { from, to } => LocationRequest::MessagesGet { from: *from, to: *to },
    }
  }
}

impl<S: AsRef<str>, B: AsRef<[u8]>> LocationResponse<S, B> {
  pub fn reference<'a, R: Referencer<S> + Referencer<B>>(
    &'a self,
    reference: R,
  ) -> LocationResponse<<R as Referencer<S>>::Output<'a>, <R as Referencer<B>>::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
    <R as Referencer<B>>::Output<'a>: AsRef<[u8]>,
  {
    match self {
      LocationResponse::AccessChange { id, result } => LocationResponse::AccessChange { id: *id, result: *result },
      LocationResponse::AccessCurrent { rules, default } => {
        LocationResponse::AccessCurrent { rules: rules.iter().map(|r| r.reference(reference)).collect(), default: *default }
      }
      LocationResponse::AnnouncementUpdate { id, result } => LocationResponse::AnnouncementUpdate { id: *id, result: *result },
      LocationResponse::Announcements(announcements) => {
        LocationResponse::Announcements(announcements.into_iter().map(|a| a.reference(reference)).collect())
      }
      LocationResponse::AvatarUpdate { avatars } => LocationResponse::AvatarUpdate {
        avatars: avatars.into_iter().map(|(player, avatar)| (player.reference(reference), avatar.clone())).collect(),
      },
      LocationResponse::Internal(data) => LocationResponse::Internal(reference.convert(data)),
      LocationResponse::Kick { id, result } => LocationResponse::Kick { id: *id, result: *result },
      LocationResponse::Messages { messages, to, from } => {
        LocationResponse::Messages { messages: messages.into_iter().map(|m| m.reference(reference)).collect(), to: *to, from: *from }
      }
      LocationResponse::MessageClear { id, result } => LocationResponse::MessageClear { id: *id, result: *result },
      LocationResponse::MessagePosted(message) => LocationResponse::MessagePosted(message.reference(reference)),
      LocationResponse::NameChange { id, result } => LocationResponse::NameChange { id: *id, result: *result },
      LocationResponse::NameChanged { name } => LocationResponse::NameChanged { name: reference.convert(name) },
      LocationResponse::RequestError { id } => LocationResponse::RequestError { id: *id },
    }
  }
  pub fn convert<C: Converter<S> + Converter<B>>(self, converter: C) -> LocationResponse<<C as Converter<S>>::Output, <C as Converter<B>>::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
    <C as Converter<B>>::Output: AsRef<[u8]>,
  {
    match self {
      LocationResponse::AccessChange { id, result } => LocationResponse::AccessChange { id, result },
      LocationResponse::AccessCurrent { rules, default } => {
        LocationResponse::AccessCurrent { rules: rules.into_iter().map(|rule| rule.convert(converter)).collect(), default }
      }
      LocationResponse::AnnouncementUpdate { id, result } => LocationResponse::AnnouncementUpdate { id, result },
      LocationResponse::Announcements(announcements) => {
        LocationResponse::Announcements(announcements.into_iter().map(|a| a.convert(converter)).collect())
      }
      LocationResponse::AvatarUpdate { avatars } => {
        LocationResponse::AvatarUpdate { avatars: avatars.into_iter().map(|(player, avatar)| (player.convert(converter), avatar)).collect() }
      }
      LocationResponse::Internal(data) => LocationResponse::Internal(converter.convert(data)),
      LocationResponse::Kick { id, result } => LocationResponse::Kick { id, result },
      LocationResponse::Messages { messages, to, from } => {
        LocationResponse::Messages { messages: messages.into_iter().map(|m| m.convert(converter)).collect(), to, from }
      }
      LocationResponse::MessageClear { id, result } => LocationResponse::MessageClear { id, result },
      LocationResponse::MessagePosted(message) => LocationResponse::MessagePosted(message.convert(converter)),
      LocationResponse::NameChange { id, result } => LocationResponse::NameChange { id, result },
      LocationResponse::NameChanged { name } => LocationResponse::NameChanged { name: converter.convert(name) },
      LocationResponse::RequestError { id } => LocationResponse::RequestError { id },
    }
  }
}
