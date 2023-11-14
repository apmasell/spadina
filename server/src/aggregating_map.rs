use std::collections::BTreeMap;
use std::iter;

#[derive(Clone, Default)]
pub struct AggregatingMap<K, C>(pub BTreeMap<K, C>);

impl<K: Eq + Ord + Clone, V, C: Extend<V> + Default> FromIterator<(K, V)> for AggregatingMap<K, C> {
  fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
    let mut collection = AggregatingMap(BTreeMap::new());
    collection.extend(iter);
    collection
  }
}
impl<K: Eq + Ord + Clone, V, C: Extend<V> + Default> Extend<(K, V)> for AggregatingMap<K, C> {
  fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
    for (key, value) in iter {
      self.0.entry(key).or_default().extend(iter::once(value));
    }
  }
}
