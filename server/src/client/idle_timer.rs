use crate::client::location::LocationEvent;
use futures::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep_until, Instant, Sleep};

pub struct IdleTimer {
  sleep: Option<Pin<Box<Sleep>>>,
}

impl IdleTimer {
  pub fn active(&mut self, active: bool) {
    if self.sleep.is_some() != active {
      self.sleep = if active { Some(Box::pin(sleep_until(Instant::now() + Duration::from_secs(60)))) } else { None };
    }
  }
}
impl Default for IdleTimer {
  fn default() -> Self {
    IdleTimer { sleep: None }
  }
}

impl Stream for IdleTimer {
  type Item = LocationEvent;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let timer = self.get_mut();
    let expired = timer
      .sleep
      .as_mut()
      .map(|sleep| match sleep.as_mut().poll(cx) {
        Poll::Ready(_) => true,
        Poll::Pending => false,
      })
      .unwrap_or(false);
    if expired {
      timer.sleep = None;
      Poll::Ready(Some(LocationEvent::IdleTimeout))
    } else {
      Poll::Pending
    }
  }
}
