use crate::reference_converter::{AsReference, Converter, Referencer};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

pub const SCHEME: &str = "spadina:";

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Resource<S: AsRef<str>> {
  Asset(S),
  Player(crate::player::PlayerIdentifier<S>),
  Location(crate::location::target::UnresolvedTarget<S>),
  Login { player: S, server: S, token: S },
  Server(S),
}

#[derive(Debug)]
pub enum ResourceParseError {
  BadScheme,
  Base64(base64::DecodeError),
  Serde(rmp_serde::decode::Error),
}
impl<S: AsRef<str>> Resource<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> Resource<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      Resource::Asset(a) => Resource::Asset(reference.convert(a)),
      Resource::Player(p) => Resource::Player(p.reference(reference)),
      Resource::Location(r) => Resource::Location(r.reference(reference)),
      Resource::Login { player, server, token } => {
        Resource::Login { player: reference.convert(player), server: reference.convert(server), token: reference.convert(token) }
      }
      Resource::Server(s) => Resource::Server(reference.convert(s)),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> Resource<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      Resource::Asset(a) => Resource::Asset(converter.convert(a)),
      Resource::Player(p) => Resource::Player(p.convert(converter)),
      Resource::Location(r) => Resource::Location(r.convert(converter)),
      Resource::Login { player, server, token } => {
        Resource::Login { player: converter.convert(player), server: converter.convert(server), token: converter.convert(token) }
      }
      Resource::Server(s) => Resource::Server(converter.convert(s)),
    }
  }
  pub fn localize(self, local_server: &str) -> Self {
    match self {
      Resource::Asset(a) => Resource::Asset(a),
      Resource::Player(p) => Resource::Player(p.localize(local_server)),
      Resource::Location(r) => Resource::Location(r),
      Resource::Login { player, server, token } => Resource::Login { player, server, token },
      Resource::Server(s) => Resource::Server(s),
    }
  }
}
impl TryFrom<&str> for Resource<String> {
  type Error = ResourceParseError;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    if value.starts_with(SCHEME) {
      Ok(rmp_serde::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&value[SCHEME.len()..])?)?)
    } else {
      Err(ResourceParseError::BadScheme)
    }
  }
}
impl<S: AsRef<str>> Display for Resource<S> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let mut result = SCHEME.to_string();
    rmp_serde::encode::write_named(
      &mut base64::write::EncoderStringWriter::from_consumer(&mut result, &base64::engine::general_purpose::URL_SAFE_NO_PAD),
      &self.reference(AsReference::<str>::default()),
    )
    .expect("Failed to encode resource as URL");
    f.write_str(&result)
  }
}

impl From<base64::DecodeError> for ResourceParseError {
  fn from(value: base64::DecodeError) -> Self {
    ResourceParseError::Base64(value)
  }
}
impl From<rmp_serde::decode::Error> for ResourceParseError {
  fn from(value: rmp_serde::decode::Error) -> Self {
    ResourceParseError::Serde(value)
  }
}

impl Display for ResourceParseError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ResourceParseError::BadScheme => f.write_str("Schema is not Spadina"),
      ResourceParseError::Base64(e) => Display::fmt(e, f),
      ResourceParseError::Serde(e) => Display::fmt(e, f),
    }
  }
}
