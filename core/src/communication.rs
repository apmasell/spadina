use crate::location::target::UnresolvedTarget;
use crate::reference_converter::{Converter, Referencer};
use crate::resource::Resource;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An announcement that should be visible to users
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Announcement<S: AsRef<str>> {
  /// The summary/title of the event
  pub title: S,
  /// The text that should be displayed
  pub body: S,
  /// The time when the event described will start
  pub when: AnnouncementTime,
  /// The location where the event will be held
  pub location: UnresolvedTarget<S>,
  /// The announcement is visible on the public calendar (i.e., it can be seen without logging in)
  pub public: bool,
}
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone, Copy)]
pub enum AnnouncementTime {
  /// The event has no start, but the announcement expires
  Until(DateTime<Utc>),
  /// The event starts a particular time and lasts a certain number of minutes
  Starts(DateTime<Utc>, u32),
}

/// Information about direct messages between this player and another
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectMessage<S: AsRef<str>> {
  /// Whether the direction was to the player or from the player
  pub inbound: bool,
  /// The contents of the message
  pub body: MessageBody<S>,
  /// The time the message was sent
  pub timestamp: DateTime<Utc>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectMessageInfo {
  pub last_received: DateTime<Utc>,
  pub last_read: Option<DateTime<Utc>>,
}
/// The status of a direct message after sending
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectMessageStatus {
  /// The message was written to user's inbox
  Delivered(DateTime<Utc>),
  /// The recipient is invalid
  UnknownRecipient,
  /// The message was placed in a queue to send to a remote server. More delivery information may follow.
  Queued,
  /// The player isn't allowed to send direct messages yet
  Forbidden,
  /// An error occurred on the server while sending the message
  InternalError,
}
/// The result of attempting to create an invitation
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
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
  Announcement(Announcement<S>),
  Delete(DateTime<Utc>),
  Edit(DateTime<Utc>, S),
  Reaction(DateTime<Utc>, char),
  Read,
  Resource(Resource<S>),
  Reply(DateTime<Utc>, S),
  Time(DateTime<Utc>),
  Text(S),
  Typing,
}

impl AnnouncementTime {
  pub fn expires(&self) -> DateTime<Utc> {
    match self {
      AnnouncementTime::Until(t) => *t,
      AnnouncementTime::Starts(t, m) => *t + chrono::Duration::minutes(*m as i64),
    }
  }
}

impl<S: AsRef<str>> Announcement<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> Announcement<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    Announcement {
      title: reference.convert(&self.title),
      body: reference.convert(&self.body),
      when: self.when,
      location: self.location.reference(reference),
      public: self.public,
    }
  }
  pub fn convert<C: Converter<S>>(self, conversion: C) -> Announcement<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    Announcement {
      title: conversion.convert(self.title),
      body: conversion.convert(self.body),
      when: self.when,
      location: self.location.convert(conversion),
      public: self.public,
    }
  }
}
impl<S: AsRef<str>> DirectMessage<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> DirectMessage<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    DirectMessage { inbound: self.inbound, body: self.body.reference(reference), timestamp: self.timestamp }
  }
  pub fn convert<C: Converter<S>>(self, conversion: C) -> DirectMessage<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    DirectMessage { inbound: self.inbound, body: self.body.convert(conversion), timestamp: self.timestamp }
  }
}
impl<S: AsRef<str>> MessageBody<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> MessageBody<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      MessageBody::Announcement(announcement) => MessageBody::Announcement(announcement.reference(reference)),
      MessageBody::Delete(ts) => MessageBody::Delete(*ts),
      MessageBody::Edit(ts, t) => MessageBody::Edit(*ts, reference.convert(t)),
      MessageBody::Reaction(ts, emoji) => MessageBody::Reaction(*ts, *emoji),
      MessageBody::Read => MessageBody::Read,
      MessageBody::Resource(resource) => MessageBody::Resource(resource.reference(reference)),
      MessageBody::Reply(ts, t) => MessageBody::Reply(*ts, reference.convert(t)),
      MessageBody::Time(ts) => MessageBody::Time(*ts),
      MessageBody::Text(t) => MessageBody::Text(reference.convert(t)),
      MessageBody::Typing => MessageBody::Typing,
    }
  }
  pub fn convert<C: Converter<S>>(self, conversion: C) -> MessageBody<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      MessageBody::Announcement(announcement) => MessageBody::Announcement(announcement.convert(conversion)),
      MessageBody::Delete(ts) => MessageBody::Delete(ts),
      MessageBody::Edit(ts, t) => MessageBody::Edit(ts, conversion.convert(t)),
      MessageBody::Reaction(ts, emoji) => MessageBody::Reaction(ts, emoji),
      MessageBody::Read => MessageBody::Read,
      MessageBody::Resource(resource) => MessageBody::Resource(resource.convert(conversion)),
      MessageBody::Reply(ts, t) => MessageBody::Reply(ts, conversion.convert(t)),
      MessageBody::Time(ts) => MessageBody::Time(ts),
      MessageBody::Text(t) => MessageBody::Text(conversion.convert(t)),
      MessageBody::Typing => MessageBody::Typing,
    }
  }
  pub fn is_transient(&self) -> bool {
    match self {
      MessageBody::Read | MessageBody::Typing => true,
      _ => false,
    }
  }
  pub fn is_valid_at(&self, now: &DateTime<Utc>) -> bool {
    match self {
      MessageBody::Announcement(Announcement { when, .. }) => when.expires() > *now,
      MessageBody::Delete(ts) => ts < now,
      MessageBody::Edit(ts, _) => ts < now,
      MessageBody::Reaction(ts, _) => ts < now,
      MessageBody::Read => true,
      MessageBody::Resource(_) => true,
      MessageBody::Reply(ts, _) => ts < now,
      MessageBody::Time(_) => true,
      MessageBody::Text(_) => true,
      MessageBody::Typing => true,
    }
  }
}
