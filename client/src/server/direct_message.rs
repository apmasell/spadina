use crate::server::exports::Export;
use chrono::{DateTime, Utc};
use spadina_core::communication::{DirectMessage, DirectMessageStatus, MessageBody};
use spadina_core::net::server::ClientRequest;
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::AsReference;
use spadina_core::tracking_map::TrackingMap;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
pub(crate) struct DirectMessages {
  messages: Vec<DirectMessage<String>>,
  times_requested: Vec<RangeInclusive<DateTime<Utc>>>,
}
pub struct Outstanding {
  body: MessageBody<String>,
  status: DirectMessageStatus,
  player: PlayerIdentifier<String>,
}
pub enum MessageFailure {
  UnknownRecipient,
  Forbidden,
  InternalError,
}

impl DirectMessages {
  pub fn add(&mut self, message: DirectMessage<String>) {
    self.extend_last_range(message.timestamp);
    self.messages.push(message);
    self.messages.sort_by_key(|m| m.timestamp);
  }
  pub fn add_range(&mut self, from: DateTime<Utc>, to: DateTime<Utc>, messages: Vec<DirectMessage<String>>) {
    self.messages.extend(messages);
    self.messages.sort_by_key(|m| m.timestamp);
    self.times_requested.push(from..=to);
    self.times_requested.sort_by_key(|r| *r.start());
  }
  pub fn get<'a, E: Export<[DirectMessage<String>]>>(
    &'a self,
    player: &PlayerIdentifier<&str>,
    mut from: DateTime<Utc>,
    mut to: DateTime<Utc>,
    export: E,
  ) -> (Vec<Message>, E::Output<'a>) {
    let mut ranges = Vec::new();
    for range in &self.times_requested {
      if range.contains(&from) {
        from = *range.end();
      }
      if range.contains(&to) {
        to = *range.start();
      }
      if (from..=to).contains(range.start()) && (from..=to).contains(range.end()) {
        if from != *range.start() {
          ranges.push(ClientRequest::<_, &[u8]>::DirectMessageGet { player: player.clone(), from, to: *range.start() }.into());
        }
        from = *range.end();
      }
    }
    if from != to {
      ranges.push(ClientRequest::<_, &[u8]>::DirectMessageGet { player: player.clone(), from, to }.into());
    }
    if ranges.is_empty() {
      let mut it = self.messages.iter().enumerate();
      let first = it.by_ref().skip_while(|(_, m)| m.timestamp < from).next().map(|(i, _)| i);
      let last = it.skip_while(|(_, m)| m.timestamp <= to).next().map(|(i, _)| i).unwrap_or(self.messages.len());

      (ranges, export.export(first.map(|start| self.messages.get(start..last)).flatten()))
    } else {
      (ranges, export.export(None))
    }
  }

  fn extend_last_range(&mut self, timestamp: DateTime<Utc>) {
    if self.times_requested.is_empty() {
      self.times_requested.push(timestamp..=timestamp);
    } else {
      let range = self.times_requested.last_mut().unwrap();
      let new_range = *range.start()..=timestamp;
      *range = new_range;
    }
  }
}
pub(crate) fn send(tracking: &mut TrackingMap<Outstanding>, player: PlayerIdentifier<String>, body: MessageBody<String>) -> Message {
  tracking.add(Outstanding { body, player, status: DirectMessageStatus::Queued }, |id, outstanding| {
    ClientRequest::<_, &[u8]>::DirectMessageSend {
      id,
      recipient: outstanding.player.reference(AsReference::<str>::default()),
      body: outstanding.body.reference(AsReference::<str>::default()),
    }
    .into()
  })
}
pub(crate) fn send_finish(
  tracking: &mut TrackingMap<Outstanding>,
  id: u32,
  status: DirectMessageStatus,
  messages: &mut HashMap<PlayerIdentifier<String>, DirectMessages>,
) -> Result<Option<PlayerIdentifier<String>>, (MessageFailure, PlayerIdentifier<String>, MessageBody<String>)> {
  if status == DirectMessageStatus::Queued {
    if let Some(outstanding) = tracking.get_mut(id) {
      outstanding.status = status;
      return Ok(Some(outstanding.player.clone()));
    }
    Ok(None)
  } else {
    if let Some(outstanding) = tracking.finish(id) {
      return match status {
        DirectMessageStatus::Delivered(timestamp) => {
          let direct_messages = messages.entry(outstanding.player.clone()).or_default();
          direct_messages.messages.push(DirectMessage { inbound: false, body: outstanding.body, timestamp });
          direct_messages.messages.sort_by_key(|m| m.timestamp);
          direct_messages.extend_last_range(timestamp);
          Ok(Some(outstanding.player))
        }
        DirectMessageStatus::UnknownRecipient => Err((MessageFailure::UnknownRecipient, outstanding.player, outstanding.body)),
        DirectMessageStatus::Queued => Ok(None),
        DirectMessageStatus::Forbidden => Err((MessageFailure::Forbidden, outstanding.player, outstanding.body)),
        DirectMessageStatus::InternalError => Err((MessageFailure::InternalError, outstanding.player, outstanding.body)),
      };
    }
    Ok(None)
  }
}
