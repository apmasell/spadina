use crate::reference_converter::{Converter, Referencer};
use serde::{Deserialize, Serialize};

/// The full address of a location
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct AbsoluteTarget<S: AsRef<str>> {
  pub descriptor: super::Descriptor<S>,
  pub owner: S,
  pub server: S,
}

/// The address of a location without the server
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct LocalTarget<S: AsRef<str>> {
  pub descriptor: super::Descriptor<S>,
  pub owner: S,
}
/// The location that has been selected that might need additional context to resolve
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum UnresolvedTarget<S: AsRef<str>> {
  Absolute { descriptor: super::Descriptor<S>, owner: S, server: S },
  NoWhere,
  Personal { asset: S },
}

impl<S: AsRef<str>> AbsoluteTarget<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> AbsoluteTarget<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    AbsoluteTarget {
      descriptor: self.descriptor.reference(reference),
      owner: reference.convert(&self.owner),
      server: reference.convert(&self.server),
    }
  }
  pub fn as_local<'a, R: Referencer<S>>(&'a self, reference: R) -> LocalTarget<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    LocalTarget { descriptor: self.descriptor.reference(reference), owner: reference.convert(&self.owner) }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> AbsoluteTarget<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    let AbsoluteTarget { descriptor, owner, server } = self;
    AbsoluteTarget { descriptor: descriptor.convert(converter), owner: converter.convert(owner), server: converter.convert(server) }
  }
  pub fn into_local(self) -> (LocalTarget<S>, S) {
    (LocalTarget { descriptor: self.descriptor, owner: self.owner }, self.server)
  }
}
impl<S: AsRef<str>> LocalTarget<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> LocalTarget<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    LocalTarget { descriptor: self.descriptor.reference(reference), owner: reference.convert(&self.owner) }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> LocalTarget<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    let LocalTarget { descriptor, owner } = self;
    LocalTarget { descriptor: descriptor.convert(converter), owner: converter.convert(owner) }
  }
  pub fn into_absolute(self, server: S) -> AbsoluteTarget<S> {
    AbsoluteTarget { descriptor: self.descriptor, owner: self.owner, server }
  }
}
impl<S: AsRef<str>> UnresolvedTarget<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> UnresolvedTarget<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      UnresolvedTarget::Absolute { descriptor, owner, server } => {
        UnresolvedTarget::Absolute { descriptor: descriptor.reference(reference), owner: reference.convert(owner), server: reference.convert(server) }
      }
      UnresolvedTarget::NoWhere => UnresolvedTarget::NoWhere,
      UnresolvedTarget::Personal { asset } => UnresolvedTarget::Personal { asset: reference.convert(asset) },
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> UnresolvedTarget<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      UnresolvedTarget::Absolute { descriptor, owner, server } => {
        UnresolvedTarget::Absolute { descriptor: descriptor.convert(converter), owner: converter.convert(owner), server: converter.convert(server) }
      }
      UnresolvedTarget::NoWhere => UnresolvedTarget::NoWhere,
      UnresolvedTarget::Personal { asset } => UnresolvedTarget::Personal { asset: converter.convert(asset) },
    }
  }
}
impl<S: AsRef<str>> From<AbsoluteTarget<S>> for UnresolvedTarget<S> {
  fn from(value: AbsoluteTarget<S>) -> Self {
    let AbsoluteTarget { descriptor, owner, server } = value;
    UnresolvedTarget::Absolute { descriptor, owner, server }
  }
}
