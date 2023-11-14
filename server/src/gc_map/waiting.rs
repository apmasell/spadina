use crate::gc_map::TrackableValue;
use crate::stream_map::OutputMapper;
use futures::{FutureExt, Stream};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::{sleep, Sleep};

pub enum Waiting<T, F, C> {
  Value(T),
  Pending(F, C, Vec<oneshot::Sender<T>>),
  Dead(Pin<Box<Sleep>>),
}

pub trait Communication: Unpin {
  type Parameter: 'static;
  fn update(&mut self, parameter: Self::Parameter);
}

impl<T: Clone, F: Unpin, C: Communication> Waiting<T, F, C> {
  pub fn add(&mut self, waiter: oneshot::Sender<T>, parameter: C::Parameter) {
    match self {
      Waiting::Value(v) => {
        let _ = waiter.send(v.clone());
      }
      Waiting::Pending(_, communication, pending) => {
        communication.update(parameter);
        pending.push(waiter);
      }
      Waiting::Dead(_) => (),
    }
  }
}
impl<T: TrackableValue, F: Unpin, C: Communication> TrackableValue for Waiting<T, F, C> {
  fn is_locked(&self) -> bool {
    match self {
      Waiting::Value(v) => v.is_locked(),
      Waiting::Pending(..) => true,
      Waiting::Dead(_) => true,
    }
  }

  fn weight(&self) -> usize {
    match self {
      Waiting::Value(v) => v.weight(),
      Waiting::Pending(_, _, pending) => pending.len(),
      Waiting::Dead(_) => 1,
    }
  }
}

impl<T: TrackableValue, F: Future<Output = Option<T>> + Unpin, C: Communication> Stream for Waiting<T, F, C> {
  type Item = T;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let waiting = self.get_mut();
    match waiting {
      Waiting::Value(_) => Poll::Pending,
      Waiting::Pending(future, _, _) => match future.poll_unpin(cx) {
        Poll::Ready(Some(value)) => Poll::Ready(Some(value)),
        Poll::Ready(None) => {
          *waiting = Waiting::Dead(Box::pin(sleep(Duration::from_secs(5 * 60))));
          Poll::Pending
        }
        Poll::Pending => Poll::Pending,
      },
      Waiting::Dead(sleep) => sleep.as_mut().poll(cx).map(|_| None),
    }
  }
}

impl<K, T: TrackableValue + Clone, F: Future<Output = Option<T>> + Unpin, C: Communication> OutputMapper<K> for Waiting<T, F, C> {
  type Output = ();

  fn handle(&mut self, _: &K, value: Self::Item) -> Option<Self::Output> {
    match self {
      Waiting::Value(_) => (),
      Waiting::Pending(_, _, pending) => {
        for waiter in pending.drain(..) {
          let _ = waiter.send(value.clone());
        }
      }
      Waiting::Dead(_) => (),
    }
    *self = Waiting::Value(value);
    None
  }

  fn end(self, _: &K) -> Option<Self::Output> {
    None
  }
}
impl Communication for () {
  type Parameter = ();

  fn update(&mut self, _parameter: Self::Parameter) {}
}
