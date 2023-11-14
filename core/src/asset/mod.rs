pub mod extraction;
pub mod variants;

use crate::reference_converter::{Converter, Referencer};
use chrono::{DateTime, Utc};
use rmp_serde::decode::ReadSlice;
use serde::de::{DeserializeOwned, DeserializeSeed, EnumAccess, IntoDeserializer, VariantAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Read;

/// An asset header for a game asset; this is common across all assets
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Asset<S: AsRef<str>, B> {
  /// The type of the asset. This is not an enumeration to allow for new asset types to be added in the future. Program should handle the case of an unknown asset gracefully.
  pub asset_type: S,
  /// The player that uploaded this asset
  pub author: S,
  /// The server that forged this asset
  pub server: S,
  /// Any capabilities this asset or its children require
  pub capabilities: Vec<S>,
  /// The child assets this asset depends on; this is here so that a server can quickly pull all the appropriate assets and inform clients about which assets the require. This is more important for large aggregate assets (_i.e._, realms) over small distinct ones (_e.g._, meshes) and may be empty.
  pub children: Vec<S>,
  /// The compression used on the data
  pub compression: Compression,
  /// The actual asset. The format of this data is arbitrary and only interpretable based on `asset_type`. Usually a common external format (_e.g._, PNG) or a Message Pack-encoded Rust data structure.
  pub data: B,
  /// The licence terms applied to this asset. Currently, all licences provided are compatible, so this is mostly informative unless the asset is going to be exported.
  pub licence: Licence,
  /// A friendly name for the asset. This would be used if the asset is to appear in tool palettes and the like.
  pub name: S,
  /// A list of friendly tags to describe this asset to allow for searching.
  pub tags: Vec<S>,
  /// The time the asset was written
  pub created: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
  MessagePack,
  ZstdMessagePack,
}
pub enum CompressionError {
  MessagePack(rmp_serde::encode::Error),
  Io(std::io::Error),
}

pub enum DecompressionError {
  MessagePack(rmp_serde::decode::Error),
  Io(std::io::Error),
}

/// The licences associated with an asset
///
/// Players should be reminded of the asset when exporting it. Since the system requires unfettered replication of assets between servers, all licenses must permit copying unmodified. All assets should be available to other world builders, so remixing must be supported by all licences.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Licence {
  CreativeCommons(LicenceUses),
  CreativeCommonsNoDerivatives(LicenceUses),
  CreativeCommonsShareALike(LicenceUses),
  PubDom,
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum LicenceUses {
  Commercial,
  NonCommercial,
}

#[derive(Debug)]
pub struct Loaded<T, S: AsRef<str>> {
  asset: S,
  value: std::sync::Arc<T>,
}

impl<S: AsRef<str>, B: AsRef<[u8]>> Asset<S, B> {
  pub fn reference<'a, R: Referencer<S> + Referencer<B>>(
    &'a self,
    referencer: R,
  ) -> Asset<<R as Referencer<S>>::Output<'a>, <R as Referencer<B>>::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
    <R as Referencer<B>>::Output<'a>: AsRef<[u8]>,
  {
    Asset {
      asset_type: referencer.convert(&self.asset_type),
      author: referencer.convert(&self.author),
      server: referencer.convert(&self.server),
      capabilities: self.capabilities.iter().map(|s| referencer.convert(s)).collect(),
      children: self.children.iter().map(|s| referencer.convert(s)).collect(),
      compression: self.compression,
      data: referencer.convert(&self.data),
      licence: self.licence,
      name: referencer.convert(&self.name),
      tags: self.tags.iter().map(|s| referencer.convert(s)).collect(),
      created: self.created,
    }
  }
  pub fn convert<C: Converter<S> + Converter<B>>(self, converter: C) -> Asset<<C as Converter<S>>::Output, <C as Converter<B>>::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
    <C as Converter<B>>::Output: AsRef<[u8]>,
  {
    Asset {
      asset_type: converter.convert(self.asset_type),
      author: converter.convert(self.author),
      server: converter.convert(self.server),
      capabilities: self.capabilities.into_iter().map(|s| converter.convert(s)).collect(),
      children: self.children.into_iter().map(|s| converter.convert(s)).collect(),
      compression: self.compression,
      data: converter.convert(self.data),
      licence: self.licence,
      name: converter.convert(self.name),
      tags: self.tags.into_iter().map(|s| converter.convert(s)).collect(),
      created: self.created,
    }
  }
  pub fn principal_hash(&self) -> String {
    use sha3::Digest;
    let mut principal_hash = sha3::Sha3_512::new();
    principal_hash.update(self.asset_type.as_ref().as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(self.author.as_ref().as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(self.name.as_ref().as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(&self.data.as_ref());
    principal_hash.update(&[0]);
    principal_hash.update(self.licence.name().as_bytes());
    principal_hash.update(&[0]);
    for tag in &self.tags {
      principal_hash.update(tag.as_ref().as_bytes());
      principal_hash.update(&[0]);
    }
    principal_hash.update(self.created.to_rfc3339().as_bytes());
    hex::encode(principal_hash.finalize())
  }
}
impl<S: AsRef<str> + Clone, B: AsRef<[u8]>> Asset<S, B> {
  pub fn deserialize_inner<Inner: DeserializeOwned>(&self) -> Result<Inner, DecompressionError> {
    self.compression.decompress(self.asset_type.as_ref(), self.data.as_ref())
  }
}

impl Compression {
  pub fn compress<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>, CompressionError> {
    Ok(match self {
      Compression::MessagePack => rmp_serde::to_vec(data)?,
      Compression::ZstdMessagePack => {
        let mut buffer = Vec::new();
        let mut writer = zstd::stream::zio::Writer::new(&mut buffer, zstd::stream::raw::Encoder::new(20)?);
        rmp_serde::encode::write(&mut writer, data)?;
        writer.finish()?;
        buffer
      }
    })
  }
  pub fn decompress<T: DeserializeOwned>(&self, name: &str, data: &[u8]) -> Result<T, DecompressionError> {
    Ok(match self {
      Compression::MessagePack => deserialize_tagged_asset(name, data)?,
      Compression::ZstdMessagePack => {
        use read_restrict::ReadExt;
        deserialize_tagged_asset(
          name,
          zstd::stream::zio::Reader::new(std::io::Cursor::new(data), zstd::stream::raw::Decoder::new()?).restrict(15 * 1024),
        )?
      }
    })
  }
}

fn deserialize_tagged_asset<T: DeserializeOwned>(name: &str, read: impl Read) -> Result<T, DecompressionError> {
  struct TaggedDeserializer<'a, R: Read>(&'a str, &'a mut rmp_serde::Deserializer<R>);

  impl<'de, R: ReadSlice<'de>> EnumAccess<'de> for TaggedDeserializer<'de, R> {
    type Error = rmp_serde::decode::Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
      V: DeserializeSeed<'de>,
    {
      Ok((seed.deserialize::<serde::de::value::StrDeserializer<Self::Error>>(self.0.into_deserializer())?, self))
    }
  }
  impl<'de, R: ReadSlice<'de>> VariantAccess<'de> for TaggedDeserializer<'de, R> {
    type Error = rmp_serde::decode::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
      Deserialize::deserialize(self.1)
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
      T: DeserializeSeed<'de>,
    {
      seed.deserialize(self.1)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.1.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.1.deserialize_struct("", fields, visitor)
    }
  }
  impl<'de, R: ReadSlice<'de>> Deserializer<'de> for TaggedDeserializer<'de, R> {
    type Error = rmp_serde::decode::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      visitor.visit_enum(self)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_struct<V>(self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_enum<V>(self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
      V: Visitor<'de>,
    {
      self.deserialize_any(visitor)
    }
  }

  Ok(T::deserialize(TaggedDeserializer(name, &mut rmp_serde::Deserializer::new(read)))?)
}
impl std::fmt::Display for CompressionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CompressionError::MessagePack(e) => e.fmt(f),
      CompressionError::Io(e) => e.fmt(f),
    }
  }
}
impl From<std::io::Error> for CompressionError {
  fn from(value: std::io::Error) -> Self {
    CompressionError::Io(value)
  }
}
impl From<rmp_serde::encode::Error> for CompressionError {
  fn from(value: rmp_serde::encode::Error) -> Self {
    CompressionError::MessagePack(value)
  }
}

impl std::fmt::Display for DecompressionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      DecompressionError::MessagePack(e) => e.fmt(f),
      DecompressionError::Io(e) => e.fmt(f),
    }
  }
}
impl From<std::io::Error> for DecompressionError {
  fn from(value: std::io::Error) -> Self {
    DecompressionError::Io(value)
  }
}
impl From<rmp_serde::decode::Error> for DecompressionError {
  fn from(value: rmp_serde::decode::Error) -> Self {
    DecompressionError::MessagePack(value)
  }
}

impl Licence {
  pub fn name(&self) -> &'static str {
    match self {
      Licence::CreativeCommons(u) => match u {
        LicenceUses::Commercial => "cc",
        LicenceUses::NonCommercial => "cc-nc",
      },
      Licence::CreativeCommonsNoDerivatives(u) => match u {
        LicenceUses::Commercial => "cc-nd",
        LicenceUses::NonCommercial => "cc-nd-nc",
      },
      Licence::CreativeCommonsShareALike(u) => match u {
        LicenceUses::Commercial => "cc-sa",
        LicenceUses::NonCommercial => "cc-sa-nc",
      },
      Licence::PubDom => "public",
    }
  }
}
impl Hash for Licence {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.name().hash(state)
  }
}
impl<T, S: AsRef<str>> Loaded<T, S> {
  pub fn new(asset: S, value: T) -> Self {
    Loaded { asset, value: std::sync::Arc::new(value) }
  }
  pub fn asset(&self) -> &S {
    &self.asset
  }
}
impl<T, S: AsRef<str> + Clone> Clone for Loaded<T, S> {
  fn clone(&self) -> Self {
    Self { asset: self.asset.clone(), value: self.value.clone() }
  }
}
impl<T, S: AsRef<str>> std::ops::Deref for Loaded<T, S> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &*self.value
  }
}
