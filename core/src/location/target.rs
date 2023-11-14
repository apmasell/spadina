use crate::net::parse_server_name;
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
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct LocalTarget<S: AsRef<str>> {
  pub descriptor: super::Descriptor<S>,
  pub owner: S,
}
/// The location that has been selected that might need additional context to resolve
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum UnresolvedTarget<S: AsRef<str>> {
  Absolute(AbsoluteTarget<S>),
  NoWhere,
  Personal { asset: S },
}

impl<S: AsRef<str>> AbsoluteTarget<S> {
  pub fn localize(self, local_server: &str) -> Option<(LocalTarget<S>, Option<String>)> {
    let server = parse_server_name(self.server.as_ref())?;
    Some((LocalTarget { descriptor: self.descriptor, owner: self.owner }, if local_server == &server { None } else { Some(server) }))
  }
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
      UnresolvedTarget::Absolute(target) => UnresolvedTarget::Absolute(target.reference(reference)),
      UnresolvedTarget::NoWhere => UnresolvedTarget::NoWhere,
      UnresolvedTarget::Personal { asset } => UnresolvedTarget::Personal { asset: reference.convert(asset) },
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> UnresolvedTarget<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      UnresolvedTarget::Absolute(target) => UnresolvedTarget::Absolute(target.convert(converter)),
      UnresolvedTarget::NoWhere => UnresolvedTarget::NoWhere,
      UnresolvedTarget::Personal { asset } => UnresolvedTarget::Personal { asset: converter.convert(asset) },
    }
  }
}
impl<S: AsRef<str>> From<AbsoluteTarget<S>> for UnresolvedTarget<S> {
  fn from(value: AbsoluteTarget<S>) -> Self {
    UnresolvedTarget::Absolute(value)
  }
}
