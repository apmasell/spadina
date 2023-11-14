use crate::shared_ref::SharedRef;
use std::marker::PhantomData;
use std::sync::Arc;

pub trait Converter<Input>: Copy {
  type Output: Sized;
  fn convert(&self, value: Input) -> Self::Output;
}
pub trait Referencer<Input: ?Sized>: Copy {
  type Output<'a>: Sized
  where
    Input: 'a;
  fn convert<'a>(&self, value: &'a Input) -> Self::Output<'a>;
}

pub struct AsReference<'a, R: ?Sized>(PhantomData<&'a R>);
#[derive(Copy, Clone)]
pub struct AsOwned;
pub struct AsArc<'a, T: ?Sized>(PhantomData<&'a T>);
pub struct AsShared<'a, T: ?Sized>(PhantomData<&'a T>);
pub struct AsSingle<'a, T: ?Sized>(PhantomData<&'a T>);
#[derive(Copy, Clone)]
pub struct ForPacket;
#[derive(Copy, Clone)]
pub struct IntoSharedState;
pub struct ToClone<'a, R: ?Sized>(PhantomData<&'a R>);

impl<T: ?Sized> Clone for AsArc<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<T: ?Sized> Copy for AsArc<'_, T> {}

impl<T: ?Sized> Default for AsArc<'_, T> {
  fn default() -> Self {
    AsArc(Default::default())
  }
}
impl<T: ?Sized> Clone for AsReference<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<T: ?Sized> Copy for AsReference<'_, T> {}

impl<T: ?Sized> Default for AsReference<'_, T> {
  fn default() -> Self {
    AsReference(Default::default())
  }
}
impl<T: ?Sized> Clone for AsShared<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<T: ?Sized> Copy for AsShared<'_, T> {}

impl<T: ?Sized> Default for AsShared<'_, T> {
  fn default() -> Self {
    AsShared(Default::default())
  }
}
impl<T: ?Sized> Clone for AsSingle<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: ?Sized> Copy for AsSingle<'_, T> {}
impl<T: ?Sized> Default for AsSingle<'_, T> {
  fn default() -> Self {
    AsSingle(Default::default())
  }
}
impl<T: ?Sized> Clone for ToClone<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: ?Sized> Copy for ToClone<'_, T> {}
impl<T: ?Sized> Default for ToClone<'_, T> {
  fn default() -> Self {
    ToClone(Default::default())
  }
}

impl<T: AsRef<R>, R: ?Sized + 'static> Referencer<T> for AsReference<'_, R> {
  type Output<'a>
    = &'a R
  where
    T: 'a;

  fn convert<'a>(&self, value: &'a T) -> Self::Output<'a> {
    value.as_ref()
  }
}

impl<T: ToOwned + ?Sized> Referencer<T> for AsOwned
where
  T::Owned: Sized + 'static,
{
  type Output<'a>
    = T::Owned
  where
    T: 'a;

  fn convert<'a>(&self, value: &'a T) -> Self::Output<'a> {
    value.to_owned()
  }
}
impl<T> Converter<T> for () {
  type Output = T;

  fn convert(&self, value: T) -> Self::Output {
    value
  }
}
impl<T: ?Sized, S> Converter<S> for AsArc<'_, T>
where
  Arc<T>: From<S>,
{
  type Output = Arc<T>;

  fn convert(&self, value: S) -> Self::Output {
    Arc::<T>::from(value)
  }
}

impl<T: ToOwned + ?Sized> Converter<T::Owned> for AsSingle<'_, T>
where
  T::Owned: Sized,
{
  type Output = SharedRef<T>;

  fn convert(&self, value: T::Owned) -> Self::Output {
    SharedRef::Single(value)
  }
}
impl<T: ToOwned + ?Sized> Converter<Arc<T>> for AsShared<'_, T> {
  type Output = SharedRef<T>;

  fn convert(&self, value: Arc<T>) -> Self::Output {
    SharedRef::Shared(value)
  }
}
impl Referencer<String> for ForPacket {
  type Output<'a> = &'a str;

  fn convert<'a>(&self, value: &'a String) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Referencer<Arc<str>> for ForPacket {
  type Output<'a> = &'a str;

  fn convert<'a>(&self, value: &'a Arc<str>) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Referencer<SharedRef<str>> for ForPacket {
  type Output<'a> = &'a str;

  fn convert<'a>(&self, value: &'a SharedRef<str>) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Referencer<Vec<u8>> for ForPacket {
  type Output<'a> = &'a [u8];

  fn convert<'a>(&self, value: &'a Vec<u8>) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Referencer<Arc<[u8]>> for ForPacket {
  type Output<'a> = &'a [u8];

  fn convert<'a>(&self, value: &'a Arc<[u8]>) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Referencer<SharedRef<[u8]>> for ForPacket {
  type Output<'a> = &'a [u8];

  fn convert<'a>(&self, value: &'a SharedRef<[u8]>) -> Self::Output<'a> {
    value.as_ref()
  }
}
impl Converter<String> for IntoSharedState {
  type Output = Arc<str>;

  fn convert<'a>(&self, value: String) -> Self::Output {
    Arc::from(value)
  }
}
impl Converter<SharedRef<str>> for IntoSharedState {
  type Output = Arc<str>;

  fn convert<'a>(&self, value: SharedRef<str>) -> Self::Output {
    value.into_arc()
  }
}
impl Converter<Vec<u8>> for IntoSharedState {
  type Output = Arc<[u8]>;

  fn convert<'a>(&self, value: Vec<u8>) -> Self::Output {
    Arc::from(value)
  }
}
impl Converter<SharedRef<[u8]>> for IntoSharedState {
  type Output = Arc<[u8]>;

  fn convert<'a>(&self, value: SharedRef<[u8]>) -> Self::Output {
    value.into_arc()
  }
}

impl<T: ToOwned + ?Sized + 'static> Referencer<SharedRef<T>> for ToClone<'_, T>
where
  Arc<T>: From<T::Owned>,
  T::Owned: Clone,
{
  type Output<'a> = Arc<T>;

  fn convert<'a>(&self, value: &'a SharedRef<T>) -> Self::Output<'a> {
    match value {
      SharedRef::Single(v) => Arc::<T>::from(v.clone()),
      SharedRef::Shared(v) => v.clone(),
    }
  }
}
