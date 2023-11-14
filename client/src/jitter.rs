use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

pub struct Jitter<const N: usize> {
  index: AtomicUsize,
  buffer: [(AtomicI64, AtomicI64); N],
}
pub struct Iter<'a, const N: usize> {
  jitter: &'a Jitter<N>,
  current: usize,
  start: usize,
}
impl<const N: usize> Default for Jitter<N> {
  fn default() -> Self {
    Self { index: Default::default(), buffer: std::array::from_fn(|_| (AtomicI64::new(0), AtomicI64::new(0))) }
  }
}

impl<const N: usize> Jitter<N> {
  pub(crate) fn update(&self, remote_time: DateTime<Utc>) {
    let now = chrono::Utc::now();
    let index = self.index.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |i| Some((i + 1) % N)).unwrap();
    self.buffer[index].0.store(now.timestamp(), Ordering::Relaxed);
    self.buffer[index].1.store(now.signed_duration_since(remote_time).num_milliseconds(), Ordering::Relaxed);
  }
  pub fn iter(&self) -> Iter<N> {
    let start = self.index.load(Ordering::Relaxed);
    Iter { jitter: self, current: 0, start }
  }
}
impl<'a, const N: usize> Iterator for Iter<'a, N> {
  type Item = (DateTime<Utc>, Duration);

  fn next(&mut self) -> Option<Self::Item> {
    if self.current + 2 > N || spadina_core::abs_difference(self.start, self.jitter.index.load(Ordering::Relaxed)) < 2 {
      None
    } else {
      let (stamp, difference) = &self.jitter.buffer[(self.start + self.current) % N];
      self.current += 1;

      Some((
        Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_millis(stamp.load(Ordering::Relaxed)).unwrap()),
        Duration::milliseconds(difference.load(Ordering::Relaxed)),
      ))
    }
  }
}
