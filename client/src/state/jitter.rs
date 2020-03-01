pub struct Jitter<const N: usize> {
  index: std::sync::atomic::AtomicUsize,
  buffer: [(std::sync::atomic::AtomicI64, std::sync::atomic::AtomicI64); N],
}
pub struct Iter<'a, const N: usize> {
  jitter: &'a Jitter<N>,
  current: usize,
  start: usize,
}
impl<const N: usize> Default for Jitter<N> {
  fn default() -> Self {
    Self { index: Default::default(), buffer: std::array::from_fn(|_| (std::sync::atomic::AtomicI64::new(0), std::sync::atomic::AtomicI64::new(0))) }
  }
}

impl<const N: usize> Jitter<N> {
  pub(crate) fn update(&self, remote_time: chrono::DateTime<chrono::Utc>) {
    let now = chrono::Utc::now();
    let index = self.index.fetch_update(std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed, |i| Some((i + 1) % N)).unwrap();
    self.buffer[index].0.store(now.timestamp(), std::sync::atomic::Ordering::Relaxed);
    self.buffer[index].1.store(now.signed_duration_since(remote_time).num_milliseconds(), std::sync::atomic::Ordering::Relaxed);
  }
  pub fn iter(&self) -> Iter<N> {
    let start = self.index.load(std::sync::atomic::Ordering::Relaxed);
    Iter { jitter: self, current: 0, start }
  }
}
impl<'a, const N: usize> std::iter::Iterator for Iter<'a, N> {
  type Item = (chrono::DateTime<chrono::Utc>, chrono::Duration);

  fn next(&mut self) -> Option<Self::Item> {
    if self.current + 2 > N || spadina_core::abs_difference(self.start, self.jitter.index.load(std::sync::atomic::Ordering::Relaxed)) < 2 {
      None
    } else {
      let (stamp, difference) = &self.jitter.buffer[(self.start + self.current) % N];
      self.current += 1;

      Some((
        chrono::DateTime::from_utc(
          chrono::NaiveDateTime::from_timestamp_millis(stamp.load(std::sync::atomic::Ordering::Relaxed)).unwrap(),
          chrono::Utc,
        ),
        chrono::Duration::milliseconds(difference.load(std::sync::atomic::Ordering::Relaxed)),
      ))
    }
  }
}
