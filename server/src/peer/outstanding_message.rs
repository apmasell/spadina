use chrono::{DateTime, Utc};
use spadina_core::communication::{DirectMessageStatus, MessageBody};
use spadina_core::shared_ref::SharedRef;
use spadina_core::tracking_map::Expires;
use tokio::sync::watch;

pub struct OutstandingMessage {
  pub body: MessageBody<String>,
  pub output: watch::Sender<DirectMessageStatus>,
  pub recipient: SharedRef<str>,
  pub sender: SharedRef<str>,
  pub timeout: DateTime<Utc>,
}

impl Expires for OutstandingMessage {
  fn end_of_life(&self) -> DateTime<Utc> {
    self.timeout
  }
}
