use crate::peer::InternalEvent;
use futures::Stream;
use std::cmp::min;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{sleep_until, Instant, Sleep};

pub struct ReconnectionTimer {
  delay: u64,
  sleep: Option<Pin<Box<Sleep>>>,
  failures: u32,
}

impl ReconnectionTimer {
  pub fn active(&mut self, active: bool) {
    if self.sleep.is_some() != active {
      if active {
        self.sleep = Some(Box::pin(sleep_until(Instant::now() + Duration::from_secs(self.delay))));
      } else {
        self.delay = 5;
        self.failures = 0;
        self.sleep = None;
      };
    }
  }
  pub fn back_off(&mut self) {
    self.delay = min(self.delay * 2, 120);
    self.active(true);
    self.failures += 1;
  }
}
impl Default for ReconnectionTimer {
  fn default() -> Self {
    ReconnectionTimer { delay: 5, sleep: Some(Box::pin(sleep_until(Instant::now() + Duration::from_secs(2)))), failures: 0 }
  }
}

impl Stream for ReconnectionTimer {
  type Item = InternalEvent;

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
      Poll::Ready(Some(if timer.failures > 50 { InternalEvent::RetryExceeded } else { InternalEvent::InitiateConnection }))
    } else {
      Poll::Pending
    }
  }
}
