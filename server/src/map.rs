use cooked_waker::IntoWaker;
use futures::Stream;

#[derive(Clone)]
struct MapWaker<K: Clone + Sync + Send + std::cmp::Ord + std::cmp::Eq + 'static> {
  key: K,
  ready: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<K>>>,
  waker: std::task::Waker,
}

pub struct StreamsUnorderedMap<K: Clone + std::cmp::Ord + std::cmp::Eq + 'static, V: Stream + Unpin> {
  map: std::collections::BTreeMap<K, V>,
  ready: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<K>>>,
}
impl<K: Clone + Send + Sync + std::cmp::Ord + std::cmp::Eq + 'static> cooked_waker::Wake for MapWaker<K> {}
impl<K: Clone + Send + Sync + std::cmp::Ord + std::cmp::Eq + 'static> cooked_waker::WakeRef for MapWaker<K> {
  fn wake_by_ref(&self) {
    self.ready.lock().unwrap().insert(self.key.clone());
    self.waker.wake_by_ref();
  }
}
impl<K: Clone + Send + Sync + std::cmp::Ord + std::cmp::Eq + 'static, V: Stream + Unpin> StreamsUnorderedMap<K, V> {
  pub fn new() -> Self {
    Default::default()
  }
  pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
  where
    K: std::borrow::Borrow<Q>,
    Q: std::cmp::Ord + std::cmp::Eq,
  {
    self.map.get(k)
  }
  pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
    self.map.iter()
  }
  pub fn iter_mut(&mut self) -> std::collections::btree_map::IterMut<'_, K, V> {
    self.map.iter_mut()
  }
  pub fn insert(&mut self, k: K, v: V) -> Option<V> {
    self.ready.lock().unwrap().insert(k.clone());
    self.map.insert(k, v)
  }
  pub fn len(&self) -> usize {
    self.map.len()
  }
  pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
  where
    K: std::borrow::Borrow<Q>,
    Q: std::cmp::Ord + std::cmp::Eq,
  {
    self.map.remove_entry(k)
  }
}
impl<K: Clone + std::cmp::Ord + std::cmp::Eq, V: Stream + Unpin> Default for StreamsUnorderedMap<K, V> {
  fn default() -> Self {
    Self { map: Default::default(), ready: Default::default() }
  }
}

impl<K: Clone + std::cmp::Ord + std::cmp::Eq + 'static, V: Stream + Unpin> IntoIterator for StreamsUnorderedMap<K, V> {
  type Item = (K, V);

  type IntoIter = std::collections::btree_map::IntoIter<K, V>;

  fn into_iter(self) -> Self::IntoIter {
    self.map.into_iter()
  }
}

impl<K: Clone + std::cmp::Ord + std::cmp::Eq + 'static, V: Stream + Unpin> FromIterator<(K, V)> for StreamsUnorderedMap<K, V> {
  fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
    Self { map: std::collections::BTreeMap::from_iter(iter), ready: Default::default() }
  }
}

impl<K: Clone + Send + Sync + std::cmp::Ord + std::cmp::Eq + 'static, V: Stream + Unpin> Stream for StreamsUnorderedMap<K, V> {
  type Item = (K, Option<V::Item>);

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    let stream_map = self.get_mut();
    while let Some(key) = stream_map.ready.lock().unwrap().pop_first() {
      if let Some(value) = stream_map.map.get_mut(&key) {
        match std::pin::Pin::new(value).poll_next(&mut std::task::Context::from_waker(
          &Box::new(MapWaker { key: key.clone(), ready: stream_map.ready.clone(), waker: cx.waker().clone() }).into_waker(),
        )) {
          std::task::Poll::Ready(output) => return std::task::Poll::Ready(Some((key.clone(), output))),
          std::task::Poll::Pending => (),
        }
      }
    }
    std::task::Poll::Pending
  }
}
