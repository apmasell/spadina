use cooked_waker::IntoWaker;
use futures::Stream;
use std::borrow::Borrow;
use std::collections::{btree_map, hash_map, BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Clone)]
struct MapWaker<K: Clone + Sync + Send + Ord + Eq + 'static> {
  key: K,
  ready: Arc<Mutex<BTreeSet<K>>>,
  waker: Waker,
}

pub struct StreamsUnorderedMap<Map: StreamableMap> {
  map: Map,
  ready: Arc<Mutex<BTreeSet<Map::Key>>>,
}
pub struct StreamMapGuard<'a, Map: StreamableMap> {
  owner: &'a mut StreamsUnorderedMap<Map>,
  initial: BTreeSet<Map::Key>,
}
pub trait StreamableMap: Unpin {
  type Entry<'a>: StreamableEntry<Self::Key, Self::Value> + 'a
  where
    Self: 'a,
    Self::Key: 'a,
    Self::Value: 'a;
  type IterMut<'a>: Iterator<Item = (&'a Self::Key, &'a mut Self::Value)>
  where
    Self: 'a,
    Self::Key: 'a,
    Self::Value: 'a;
  type Key: Clone + Send + Sync + Ord + Eq + 'static;
  type Value: OutputMapper<Self::Key>;
  fn get(&mut self, key: Self::Key) -> Option<Self::Entry<'_>>;

  fn iter_keys(&self) -> impl Iterator<Item = &Self::Key>;
  fn iter_mut(&mut self) -> Self::IterMut<'_>;
  fn remove<Q: ?Sized + Ord + Hash + Eq>(&mut self, key: &Q) -> Option<Self::Value>
  where
    Self::Key: Borrow<Q>;
}
pub trait StreamableEntry<K, V> {
  fn get_mut(&mut self) -> &mut V;
  fn get_key(&self) -> &K;
  fn remove(self) -> V;
}
pub trait OutputMapper<K>: Stream + Unpin {
  type Output;
  fn handle(&mut self, key: &K, value: Self::Item) -> Option<Self::Output>;
  fn end(self, key: &K) -> Option<Self::Output>;
}
impl<K: Clone + Send + Sync + Ord + Eq + 'static> cooked_waker::Wake for MapWaker<K> {}
impl<K: Clone + Send + Sync + Ord + Eq + 'static> cooked_waker::WakeRef for MapWaker<K> {
  fn wake_by_ref(&self) {
    self.ready.lock().unwrap().insert(self.key.clone());
    self.waker.wake_by_ref();
  }
}
impl<Map: StreamableMap> StreamsUnorderedMap<Map> {
  pub fn new(map: Map) -> Self {
    StreamsUnorderedMap { map, ready: Default::default() }
  }
  pub fn mutate(&mut self) -> StreamMapGuard<Map> {
    let initial = self.map.iter_keys().cloned().collect();
    StreamMapGuard { owner: self, initial }
  }
  pub fn iter_mut<'a>(&'a mut self) -> Map::IterMut<'a> {
    self.map.iter_mut()
  }
  pub fn entry(&mut self, key: Map::Key) -> Option<Map::Entry<'_>> {
    self.ready.lock().unwrap().insert(key.clone());
    self.map.get(key)
  }
  pub fn remove<Q: ?Sized + Ord + Hash + Eq>(&mut self, key: &Q) -> Option<Map::Value>
  where
    Map::Key: Borrow<Q>,
  {
    self.ready.lock().unwrap().remove(key);
    self.map.remove(key)
  }
}
impl<Map: StreamableMap + Default> Default for StreamsUnorderedMap<Map> {
  fn default() -> Self {
    Self { map: Default::default(), ready: Default::default() }
  }
}

impl<Map: StreamableMap + IntoIterator> IntoIterator for StreamsUnorderedMap<Map> {
  type Item = Map::Item;

  type IntoIter = Map::IntoIter;

  fn into_iter(self) -> Self::IntoIter {
    self.map.into_iter()
  }
}

impl<Item, Map: StreamableMap + FromIterator<Item>> FromIterator<Item> for StreamsUnorderedMap<Map> {
  fn from_iter<T: IntoIterator<Item = Item>>(iter: T) -> Self {
    Self { map: Map::from_iter(iter), ready: Default::default() }
  }
}
impl<'a, Map: StreamableMap> IntoIterator for &'a mut StreamsUnorderedMap<Map> {
  type Item = (&'a Map::Key, &'a mut Map::Value);
  type IntoIter = Map::IterMut<'a>;

  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl<Map: StreamableMap> Deref for StreamsUnorderedMap<Map> {
  type Target = Map;

  fn deref(&self) -> &Self::Target {
    &self.map
  }
}
impl<Map: StreamableMap> Deref for StreamMapGuard<'_, Map> {
  type Target = Map;

  fn deref(&self) -> &Self::Target {
    &self.owner.map
  }
}
impl<Map: StreamableMap> DerefMut for StreamMapGuard<'_, Map> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.owner.map
  }
}

impl<Map: StreamableMap> Drop for StreamMapGuard<'_, Map> {
  fn drop(&mut self) {
    let current: BTreeSet<_> = self.owner.map.iter_keys().cloned().collect();
    let Ok(mut ready) = self.owner.ready.lock() else { return };
    ready.extend(current.difference(&self.initial).cloned());
    ready.retain(|v| !current.contains(v));
  }
}

impl<Map: StreamableMap> Stream for StreamsUnorderedMap<Map> {
  type Item = <Map::Value as OutputMapper<Map::Key>>::Output;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    let stream_map = self.get_mut();
    while let Some(key) = stream_map.ready.lock().unwrap().pop_first() {
      if let Some(mut value) = stream_map.map.get(key.clone()) {
        let result = Pin::new(value.get_mut()).poll_next(&mut Context::from_waker(
          &Box::new(MapWaker { key: key.clone(), ready: stream_map.ready.clone(), waker: cx.waker().clone() }).into_waker(),
        ));
        match result {
          Poll::Ready(Some(output)) => {
            if let Some(output) = value.get_mut().handle(&key, output) {
              return Poll::Ready(Some(output));
            }
          }
          Poll::Ready(None) => {
            if let Some(output) = value.remove().end(&key) {
              return Poll::Ready(Some(output));
            }
          }
          Poll::Pending => (),
        }
      }
    }
    Poll::Pending
  }
}

impl<K: Clone + Send + Sync + Ord + Eq + Unpin + 'static, V: OutputMapper<K>> StreamableMap for BTreeMap<K, V> {
  type Entry<'a>
    = btree_map::OccupiedEntry<'a, K, V>
  where
    Self: 'a,
    K: 'a,
    V: 'a;
  type IterMut<'a>
    = btree_map::IterMut<'a, K, V>
  where
    Self: 'a,
    K: 'a,
    V: 'a;
  type Key = K;
  type Value = V;

  fn get(&mut self, key: Self::Key) -> Option<Self::Entry<'_>> {
    match self.entry(key) {
      btree_map::Entry::Vacant(_) => None,
      btree_map::Entry::Occupied(entry) => Some(entry),
    }
  }

  fn iter_keys(&self) -> impl Iterator<Item = &Self::Key> {
    self.keys()
  }

  fn iter_mut(&mut self) -> Self::IterMut<'_> {
    self.iter_mut()
  }
  fn remove<Q: ?Sized + Ord + Hash + Eq>(&mut self, key: &Q) -> Option<Self::Value>
  where
    Self::Key: Borrow<Q>,
  {
    self.remove(key)
  }
}
impl<'a, K: Ord, V> StreamableEntry<K, V> for btree_map::OccupiedEntry<'a, K, V> {
  fn get_mut(&mut self) -> &mut V {
    btree_map::OccupiedEntry::get_mut(self)
  }

  fn get_key(&self) -> &K {
    self.key()
  }

  fn remove(self) -> V {
    btree_map::OccupiedEntry::remove(self)
  }
}

impl<K: Clone + Send + Sync + Ord + Eq + Hash + Unpin + 'static, V: OutputMapper<K>> StreamableMap for HashMap<K, V> {
  type Entry<'a>
    = hash_map::OccupiedEntry<'a, K, V>
  where
    Self: 'a,
    K: 'a,
    V: 'a;
  type IterMut<'a>
    = hash_map::IterMut<'a, K, V>
  where
    Self: 'a,
    K: 'a,
    V: 'a;
  type Key = K;
  type Value = V;

  fn get(&mut self, key: Self::Key) -> Option<Self::Entry<'_>> {
    match self.entry(key) {
      hash_map::Entry::Vacant(_) => None,
      hash_map::Entry::Occupied(entry) => Some(entry),
    }
  }

  fn iter_keys(&self) -> impl Iterator<Item = &Self::Key> {
    self.keys()
  }

  fn iter_mut(&mut self) -> Self::IterMut<'_> {
    self.iter_mut()
  }
  fn remove<Q: ?Sized + Ord + Hash + Eq>(&mut self, key: &Q) -> Option<Self::Value>
  where
    Self::Key: Borrow<Q>,
  {
    self.remove(key)
  }
}
impl<'a, K, V> StreamableEntry<K, V> for hash_map::OccupiedEntry<'a, K, V> {
  fn get_mut(&mut self) -> &mut V {
    self.get_mut()
  }

  fn get_key(&self) -> &K {
    self.key()
  }

  fn remove(self) -> V {
    self.remove()
  }
}
