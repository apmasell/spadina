use crate::atomic_activity::AtomicActivity;
use crate::join_request::JoinRequest;
use crate::player_location_update::PlayerLocationUpdate;
use futures::{Stream, StreamExt};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::Activity;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, Interval};
use tokio_stream::wrappers::BroadcastStream;

pub struct LocationEndpoint {
  activity: AtomicActivity,
  join: mpsc::Sender<JoinRequest>,
}

pub struct LocationJoin {
  activity: AtomicActivity,
  death: BroadcastStream<()>,
  incoming: mpsc::Receiver<JoinRequest>,
  interval: Interval,
}

pub struct RequestStream<'a> {
  join: &'a mut LocationJoin,
  player_count: usize,
}
impl LocationEndpoint {
  pub fn activity(&self) -> Activity {
    self.activity.get()
  }
  pub fn is_closed(&self) -> bool {
    self.join.is_closed()
  }
  pub fn join(&self, join_request: JoinRequest) -> Result<(), JoinRequest> {
    match self.join.try_send(join_request) {
      Ok(()) => Ok(()),
      Err(TrySendError::Full(join_request)) => {
        let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::OverloadedError));
        Ok(())
      }
      Err(TrySendError::Closed(join_request)) => Err(join_request),
    }
  }
}

pub fn new(death: broadcast::Receiver<()>) -> (LocationEndpoint, LocationJoin) {
  let (join, incoming) = mpsc::channel(100);
  let activity = AtomicActivity::default();
  (
    LocationEndpoint { join, activity: activity.clone() },
    LocationJoin { incoming, activity, death: BroadcastStream::new(death), interval: interval(Duration::from_millis(900_000)) },
  )
}

impl LocationJoin {
  pub fn stream(&mut self, player_count: usize) -> RequestStream {
    RequestStream { join: self, player_count }
  }
  pub fn into_black_hole(mut self, reason: LocationChangeResponse<Arc<str>>) {
    tokio::spawn(async move {
      while let Some(request) = self.stream(0).next().await {
        let _ = request.tx.send(PlayerLocationUpdate::ResolveUpdate(reason.clone()));
      }
    });
  }
}

impl Drop for LocationJoin {
  fn drop(&mut self) {
    self.activity.clear();
  }
}
impl<'a> Stream for RequestStream<'a> {
  type Item = JoinRequest;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let RequestStream { join: LocationJoin { incoming, activity, interval, death, .. }, player_count } = self.get_mut();
    if let Poll::Ready(_) = interval.poll_tick(cx) {
      activity.update(*player_count)
    }
    if let Poll::Ready(_) = death.poll_next_unpin(cx) {
      return Poll::Ready(None);
    }

    incoming.poll_recv(cx)
  }
}
