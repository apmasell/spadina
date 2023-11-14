use prometheus_client::encoding::{EncodeLabelValue, LabelValueEncoder};
use std::sync::Arc;

pub enum SharedRef<T: ToOwned + ?Sized>
where
  T::Owned: Sized,
{
  Single(T::Owned),
  Shared(Arc<T>),
}

impl<T: ToOwned + ?Sized> SharedRef<T>
where
  T::Owned: Sized,
  Arc<T>: From<<T as ToOwned>::Owned>,
{
  pub fn new(value: T::Owned) -> Self {
    SharedRef::Single(value)
  }
  pub fn into_inner(self) -> T::Owned {
    match self {
      SharedRef::Single(s) => s,
      SharedRef::Shared(s) => s.as_ref().to_owned(),
    }
  }
  pub fn into_arc(self) -> Arc<T> {
    match self {
      SharedRef::Single(s) => Arc::<T>::from(s),
      SharedRef::Shared(s) => s,
    }
  }
  pub fn upgrade(self) -> Self {
    match self {
      SharedRef::Single(s) => SharedRef::Shared(Arc::<T>::from(s)),
      SharedRef::Shared(s) => SharedRef::Shared(s),
    }
  }
  pub fn upgrade_in_place(&mut self) {
    replace_with::replace_with_or_abort(self, |v| v.upgrade())
  }
}
impl<T: ToOwned + ?Sized> From<Arc<T>> for SharedRef<T>
where
  T::Owned: Sized,
{
  fn from(value: Arc<T>) -> Self {
    Self::Shared(value)
  }
}
impl<T: ToOwned + ?Sized> From<&Arc<T>> for SharedRef<T>
where
  T::Owned: Sized,
{
  fn from(value: &Arc<T>) -> Self {
    Self::Shared(value.clone())
  }
}

impl<T: ToOwned + ?Sized> AsRef<T> for SharedRef<T>
where
  T::Owned: Sized,
{
  fn as_ref(&self) -> &T {
    use std::borrow::Borrow;
    match self {
      SharedRef::Single(s) => s.borrow(),
      SharedRef::Shared(s) => s.as_ref(),
    }
  }
}
impl<T: ToOwned + ?Sized> std::borrow::Borrow<T> for SharedRef<T>
where
  T::Owned: Sized,
{
  fn borrow(&self) -> &T {
    self.as_ref()
  }
}
impl<T: ToOwned + serde::Serialize + ?Sized> serde::Serialize for SharedRef<T>
where
  T::Owned: Sized,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.as_ref().serialize(serializer)
  }
}
impl<'de, T: ToOwned + ?Sized> serde::Deserialize<'de> for SharedRef<T>
where
  T::Owned: serde::de::DeserializeOwned,
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    Ok(SharedRef::Single(T::Owned::deserialize(deserializer)?))
  }
}
impl<T: ToOwned + Eq + ?Sized> Eq for SharedRef<T> where T::Owned: Sized {}
impl<T: ToOwned + Ord + ?Sized> Ord for SharedRef<T>
where
  T::Owned: Sized,
{
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.as_ref().cmp(other.as_ref())
  }
}
impl<T: ToOwned + PartialEq + ?Sized> PartialEq for SharedRef<T>
where
  T::Owned: Sized,
{
  fn eq(&self, other: &Self) -> bool {
    self.as_ref() == other.as_ref()
  }
}
impl<T: ToOwned + PartialOrd + ?Sized> PartialOrd for SharedRef<T>
where
  T::Owned: Sized,
{
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.as_ref().partial_cmp(other.as_ref())
  }
}
impl<T: ToOwned + std::hash::Hash + ?Sized> std::hash::Hash for SharedRef<T>
where
  T::Owned: Sized,
{
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.as_ref().hash(state);
  }
}
impl<T: ToOwned + std::fmt::Debug + ?Sized> std::fmt::Debug for SharedRef<T>
where
  T::Owned: Sized,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.as_ref().fmt(f)
  }
}
impl<T: ToOwned + std::fmt::Display + ?Sized> std::fmt::Display for SharedRef<T>
where
  T::Owned: Sized,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.as_ref().fmt(f)
  }
}
impl<T: ToOwned + ?Sized> EncodeLabelValue for SharedRef<T>
where
  T::Owned: Sized,
  for<'a> &'a T: EncodeLabelValue,
{
  fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
    self.as_ref().encode(encoder)
  }
}
