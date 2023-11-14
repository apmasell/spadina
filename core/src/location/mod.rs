pub mod change;
pub mod communication;
pub mod directory;
pub mod protocol;
pub mod target;

use crate::reference_converter::{Converter, Referencer};
use serde::de::value::StrDeserializer;
use serde::de::{Error, IntoDeserializer, SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub enum Application {
  Editor,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Descriptor<S: AsRef<str>> {
  Asset(S),
  Application(Application, u32),
  Unsupported(S, u32),
}

impl<Str: AsRef<str>> Descriptor<Str> {
  pub fn into_kind(self) -> DescriptorKind<Str> {
    match self {
      Descriptor::Asset(a) => DescriptorKind::Asset(a),
      Descriptor::Application(a, _) => DescriptorKind::Application(a),
      Descriptor::Unsupported(n, _) => DescriptorKind::Unsupported(n),
    }
  }
  pub fn kind(&self) -> DescriptorKind<&str> {
    match self {
      Descriptor::Asset(a) => DescriptorKind::Asset(a.as_ref()),
      Descriptor::Application(a, _) => DescriptorKind::Application(*a),
      Descriptor::Unsupported(n, _) => DescriptorKind::Unsupported(n.as_ref()),
    }
  }
}
impl<Str: AsRef<str>> serde::Serialize for Descriptor<Str> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self {
      Descriptor::Asset(id) => id.as_ref().serialize(serializer),
      Descriptor::Application(application, id) => {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(application)?;
        tuple.serialize_element(id)?;
        tuple.end()
      }
      Descriptor::Unsupported(name, id) => {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(name.as_ref())?;
        tuple.serialize_element(id)?;
        tuple.end()
      }
    }
  }
}
impl<Str: AsRef<str>> Descriptor<Str> {
  pub fn reference<'a, R: Referencer<Str>>(&'a self, reference: R) -> Descriptor<R::Output<'a>>
  where
    <R as Referencer<Str>>::Output<'a>: AsRef<str>,
  {
    match self {
      Descriptor::Asset(a) => Descriptor::Asset(reference.convert(a)),
      Descriptor::Application(a, v) => Descriptor::Application(*a, *v),
      Descriptor::Unsupported(n, v) => Descriptor::Unsupported(reference.convert(n), *v),
    }
  }

  pub fn convert<C: Converter<Str>>(self, converter: C) -> Descriptor<C::Output>
  where
    <C as Converter<Str>>::Output: AsRef<str>,
  {
    match self {
      Descriptor::Asset(a) => Descriptor::Asset(converter.convert(a)),
      Descriptor::Application(a, v) => Descriptor::Application(a, v),
      Descriptor::Unsupported(n, v) => Descriptor::Unsupported(converter.convert(n), v),
    }
  }
}
impl<'de, S: AsRef<str> + serde::Deserialize<'de>> serde::Deserialize<'de> for Descriptor<S> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct DescriptorDeserializer<'de, S: AsRef<str> + serde::Deserialize<'de>>(PhantomData<&'de ()>, PhantomData<S>);
    impl<'de, S: AsRef<str> + serde::Deserialize<'de>> Visitor<'de> for DescriptorDeserializer<'de, S> {
      type Value = Descriptor<S>;

      fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("Expected string or (type, id) tuple in descriptor")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: Error,
      {
        S::deserialize(v.into_deserializer()).map(Descriptor::Asset)
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: SeqAccess<'de>,
      {
        let Some(name) = seq.next_element::<S>()? else { return Err(Error::missing_field("type")) };
        let Some(id) = seq.next_element::<u32>()? else { return Err(Error::missing_field("id")) };
        match Application::deserialize(StrDeserializer::<A::Error>::new(name.as_ref())) {
          Ok(application) => Ok(Descriptor::Application(application, id)),
          Err(_) => Ok(Descriptor::Unsupported(name, id)),
        }
      }
    }
    deserializer.deserialize_any(DescriptorDeserializer(PhantomData::default(), PhantomData::default()))
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum DescriptorKind<S: AsRef<str>> {
  Asset(S),
  Application(Application),
  Unsupported(S),
}

impl<Str: AsRef<str>> DescriptorKind<Str> {
  pub fn reference<'a, R: Referencer<Str>>(&'a self, reference: R) -> DescriptorKind<R::Output<'a>>
  where
    <R as Referencer<Str>>::Output<'a>: AsRef<str>,
  {
    match self {
      DescriptorKind::Asset(id) => DescriptorKind::Asset(reference.convert(id)),
      DescriptorKind::Application(a) => DescriptorKind::Application(*a),
      DescriptorKind::Unsupported(name) => DescriptorKind::Unsupported(reference.convert(name)),
    }
  }
  pub fn convert<C: Converter<Str>>(self, converter: C) -> DescriptorKind<C::Output>
  where
    <C as Converter<Str>>::Output: AsRef<str>,
  {
    match self {
      DescriptorKind::Asset(id) => DescriptorKind::Asset(converter.convert(id)),
      DescriptorKind::Application(a) => DescriptorKind::Application(a),
      DescriptorKind::Unsupported(name) => DescriptorKind::Unsupported(converter.convert(name)),
    }
  }
}

impl<Str: AsRef<str>> serde::Serialize for DescriptorKind<Str> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self {
      DescriptorKind::Asset(id) => {
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element("Asset")?;
        tuple.serialize_element(id.as_ref())?;
        tuple.end()
      }
      DescriptorKind::Application(application) => application.serialize(serializer),
      DescriptorKind::Unsupported(name) => name.as_ref().serialize(serializer),
    }
  }
}

impl<'de, S: AsRef<str> + serde::Deserialize<'de>> serde::Deserialize<'de> for DescriptorKind<S> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct DescriptorDeserializer<'de, S: AsRef<str> + serde::Deserialize<'de>>(PhantomData<&'de ()>, PhantomData<S>);
    impl<'de, S: AsRef<str> + serde::Deserialize<'de>> Visitor<'de> for DescriptorDeserializer<'de, S> {
      type Value = DescriptorKind<S>;

      fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("Expected string or (\"Asset\", id) tuple in descriptor")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: Error,
      {
        match Application::deserialize(StrDeserializer::<E>::new(v)) {
          Ok(application) => Ok(DescriptorKind::Application(application)),
          _ => S::deserialize(v.into_deserializer()).map(DescriptorKind::Unsupported),
        }
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: SeqAccess<'de>,
      {
        let Some(name) = seq.next_element::<&str>()? else { return Err(Error::missing_field("type")) };
        let Some(id) = seq.next_element::<S>()? else { return Err(Error::missing_field("id")) };
        match name {
          "Asset" => Ok(DescriptorKind::Asset(id)),
          _ => S::deserialize(name.into_deserializer()).map(DescriptorKind::Unsupported),
        }
      }
    }
    deserializer.deserialize_any(DescriptorDeserializer(PhantomData::default(), PhantomData::default()))
  }
}
