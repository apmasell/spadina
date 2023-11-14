use crate::asset::Asset;
use crate::location::directory::Activity;
use crate::player::OnlineState;
use crate::tracking_map::Expires;
use chrono::{DateTime, Duration, Utc};
use tokio::sync::oneshot;

pub struct OneshotTimeout<T> {
  sender: oneshot::Sender<T>,
  give_up: DateTime<Utc>,
}

pub trait WaitTime {
  const DURATION: Duration;
}

impl<T> OneshotTimeout<T> {
  pub fn send(self, value: T) -> Result<(), T> {
    self.sender.send(value)
  }
}
impl<T> Expires for OneshotTimeout<T> {
  fn end_of_life(&self) -> DateTime<Utc> {
    self.give_up
  }
}

impl<T: WaitTime> From<oneshot::Sender<T>> for OneshotTimeout<T> {
  fn from(sender: oneshot::Sender<T>) -> Self {
    OneshotTimeout { sender, give_up: Utc::now() + T::DURATION }
  }
}

impl WaitTime for Activity {
  const DURATION: Duration = Duration::milliseconds(60_000);
}
impl WaitTime for Asset<String, Vec<u8>> {
  const DURATION: Duration = Duration::milliseconds(5 * 60_000);
}

impl<S: AsRef<str>> WaitTime for OnlineState<S> {
  const DURATION: Duration = Duration::milliseconds(60_000);
}
