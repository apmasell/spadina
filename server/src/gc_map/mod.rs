pub mod time_use;
pub mod waiting;

use crate::stream_map::{OutputMapper, StreamableEntry, StreamableMap};
use std::borrow::Borrow;
use std::collections::hash_map::{Entry, IterMut};
use std::collections::{hash_map, HashMap};
use std::hash::Hash;
use std::sync::Arc;

pub struct GarbageCollectorMap<K, V, S> {
  data: HashMap<K, (V, S)>,
  desired_cap: usize,
}
pub trait CollectionStrategy: Default + Unpin + 'static {
  type Pressure: Copy + Ord;
  fn collection_pressure(&self) -> Self::Pressure;
  fn notify_used(&mut self, active: usize);
}
pub trait TrackableValue: Unpin {
  fn is_locked(&self) -> bool;
  fn weight(&self) -> usize;
}

pub struct GarbageIterMut<'a, K, V, S> {
  iter: IterMut<'a, K, (V, S)>,
}

pub trait Launcher<K, V>
where
  V: TrackableValue,
{
  fn launch(self, key: K) -> V;
}

impl<K: Clone + Send + Sync + Ord + Eq + Hash, V: TrackableValue + OutputMapper<K>, S: CollectionStrategy> GarbageCollectorMap<K, V, S> {
  pub fn new(desired_cap: usize) -> Self {
    GarbageCollectorMap { data: Default::default(), desired_cap }
  }
  pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<&V>
  where
    K: Borrow<Q>,
  {
    let (value, _) = self.data.get(key)?;
    Some(value)
  }
  pub fn perform_gc(&mut self) {
    for (value, tracking) in self.data.values_mut() {
      tracking.notify_used(value.weight())
    }
    if self.data.len() > self.desired_cap {
      let mut pressure: Vec<_> =
        self.data.values_mut().flat_map(|(value, tracking)| if value.is_locked() { None } else { Some(tracking.collection_pressure()) }).collect();
      let overrun = self.data.len() - self.desired_cap;
      if overrun >= pressure.len() {
        self.data.retain(|_, (value, _)| value.is_locked());
      } else {
        pressure.sort();
        self.data.retain(|_, (value, tracking)| value.is_locked() || tracking.collection_pressure() <= pressure[overrun]);
      }
    }
  }
  pub fn upsert<C: Launcher<K, V>>(&mut self, key: K, launcher: C) -> &mut V {
    let (value, _) = self.data.entry(key).or_insert_with_key(|key| (launcher.launch(key.clone()), Default::default()));
    value
  }
  pub fn remove<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<V>
  where
    K: Borrow<Q>,
  {
    let (value, _) = self.data.remove(key)?;
    Some(value)
  }
}
impl<K: Clone + Send + Sync + Ord + Hash + Eq + Unpin + 'static, V: TrackableValue + OutputMapper<K>, S: CollectionStrategy> StreamableMap
  for GarbageCollectorMap<K, V, S>
{
  type Entry<'a>
    = GarbageCollectorEntry<'a, K, V, S>
  where
    Self: 'a,
    Self::Key: 'a,
    Self::Value: 'a;
  type IterMut<'a>
    = GarbageIterMut<'a, K, V, S>
  where
    Self: 'a,
    Self::Key: 'a,
    Self::Value: 'a;
  type Key = K;
  type Value = V;

  fn get(&mut self, key: Self::Key) -> Option<Self::Entry<'_>> {
    match self.data.entry(key) {
      Entry::Occupied(entry) => Some(GarbageCollectorEntry { entry }),
      Entry::Vacant(_) => None,
    }
  }

  fn iter_keys(&self) -> impl Iterator<Item = &Self::Key> {
    self.data.keys()
  }

  fn iter_mut(&mut self) -> Self::IterMut<'_> {
    GarbageIterMut { iter: self.data.iter_mut() }
  }
  fn remove<Q: ?Sized + Ord + Hash + Eq>(&mut self, key: &Q) -> Option<Self::Value>
  where
    Self::Key: Borrow<Q>,
  {
    let (value, _) = self.data.remove(key)?;
    Some(value)
  }
}
pub struct GarbageCollectorEntry<'a, K: Clone, V: TrackableValue, S: CollectionStrategy> {
  entry: hash_map::OccupiedEntry<'a, K, (V, S)>,
}
impl<'a, K: Clone, V: TrackableValue, S: CollectionStrategy> StreamableEntry<K, V> for GarbageCollectorEntry<'a, K, V, S> {
  fn get_mut(&mut self) -> &mut V {
    let (value, _) = self.entry.get_mut();
    value
  }

  fn get_key(&self) -> &K {
    self.entry.key()
  }

  fn remove(self) -> V {
    let (value, _) = self.entry.remove();
    value
  }
}
impl<T: ?Sized> TrackableValue for Arc<T> {
  fn is_locked(&self) -> bool {
    Arc::strong_count(self) > 1
  }

  fn weight(&self) -> usize {
    Arc::strong_count(self) - 1
  }
}
impl<'a, K, V, S> Iterator for GarbageIterMut<'a, K, V, S> {
  type Item = (&'a K, &'a mut V);

  fn next(&mut self) -> Option<Self::Item> {
    let (key, (value, _)) = self.iter.next()?;
    Some((key, value))
  }
}
