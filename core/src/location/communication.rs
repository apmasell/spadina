use crate::communication::{AnnouncementTime, MessageBody};
use crate::player::PlayerIdentifier;
use crate::reference_converter::{Converter, Referencer};
use chrono::{DateTime, Utc};

/// A realm-specific announcement that should be visible to users
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Announcement<S: AsRef<str>> {
  /// The summary/title of the event
  pub title: S,
  /// The text that should be displayed
  pub body: S,
  /// The time when the event described will start
  pub when: AnnouncementTime,
  /// The announcement is visible on the public calendar (i.e., it can be seen without logging in)
  pub public: bool,
}
/// A message to all players in a realm
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatMessage<S: AsRef<str>> {
  /// The contents of the message
  pub body: MessageBody<S>,
  /// The principal of the player that sent the message
  pub sender: PlayerIdentifier<S>,
  /// The time the message was sent
  pub timestamp: DateTime<Utc>,
}

impl<S: AsRef<str>> Announcement<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, referencer: R) -> Announcement<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    Announcement { title: referencer.convert(&self.title), body: referencer.convert(&self.body), when: self.when.clone(), public: self.public }
  }
  pub fn convert<C: Converter<S>>(self, conversion: C) -> Announcement<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    Announcement { title: conversion.convert(self.title), body: conversion.convert(self.body), when: self.when, public: self.public }
  }
}
impl<S: AsRef<str>> ChatMessage<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, referencer: R) -> ChatMessage<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    ChatMessage { body: self.body.reference(referencer), sender: self.sender.reference(referencer), timestamp: self.timestamp }
  }
  pub fn convert<C: Converter<S>>(self, conversion: C) -> ChatMessage<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    ChatMessage { body: self.body.convert(conversion), sender: self.sender.convert(conversion), timestamp: self.timestamp }
  }
}
