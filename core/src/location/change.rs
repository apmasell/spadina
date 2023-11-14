use crate::access::{AccessControl, Privilege};
use crate::location;
use crate::location::target::AbsoluteTarget;
use crate::location::DescriptorKind;
use crate::player::{OnlineState, PlayerIdentifier};
use crate::reference_converter::{Converter, Referencer};
use location::Descriptor;

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum LocationChangeRequest<S: AsRef<str>> {
  Guest(PlayerIdentifier<S>),
  Host { descriptor: DescriptorKind<S>, rules: Vec<AccessControl<S, Privilege>>, default: Privilege },
  Location(AbsoluteTarget<S>),
  New(DescriptorKind<S>),
  NoWhere,
}
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum LocationChangeResponse<S: AsRef<str>> {
  Guest {
    host: PlayerIdentifier<S>,
    descriptor: DescriptorKind<S>,
    name: S,
  },
  Hosting,
  Location {
    /// The ID of the realm
    owner: S,
    /// The server that hosts the realm
    server: S,
    /// The friendly name for the realm
    name: S,
    /// The root asset for the realm
    descriptor: Descriptor<S>,
  },
  NoWhere,
  Resolving,
  WaitingForAsset,
  WaitingForPeer,
  InternalError,
  MissingAssetError {
    assets: Vec<S>,
  },
  MissingCapabilitiesError {
    capabilities: Vec<S>,
  },
  OverloadedError,
  PermissionError,
  ResolutionError,
  UnsupportedError,
}

impl<S: AsRef<str>> LocationChangeResponse<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> LocationChangeResponse<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      LocationChangeResponse::Guest { host, descriptor, name } => {
        LocationChangeResponse::Guest { host: host.reference(reference), descriptor: descriptor.reference(reference), name: reference.convert(name) }
      }
      LocationChangeResponse::Hosting => LocationChangeResponse::Hosting,
      LocationChangeResponse::Location { owner, server, name, descriptor } => LocationChangeResponse::Location {
        owner: reference.convert(owner),
        server: reference.convert(server),
        name: reference.convert(name),
        descriptor: descriptor.reference(reference),
      },
      LocationChangeResponse::NoWhere => LocationChangeResponse::NoWhere,
      LocationChangeResponse::Resolving => LocationChangeResponse::Resolving,
      LocationChangeResponse::WaitingForAsset => LocationChangeResponse::WaitingForAsset,
      LocationChangeResponse::WaitingForPeer => LocationChangeResponse::WaitingForPeer,
      LocationChangeResponse::InternalError => LocationChangeResponse::InternalError,
      LocationChangeResponse::MissingAssetError { assets } => {
        LocationChangeResponse::MissingAssetError { assets: assets.iter().map(|asset| reference.convert(asset)).collect() }
      }
      LocationChangeResponse::MissingCapabilitiesError { capabilities } => {
        LocationChangeResponse::MissingCapabilitiesError { capabilities: capabilities.iter().map(|cap| reference.convert(cap)).collect() }
      }
      LocationChangeResponse::OverloadedError => LocationChangeResponse::OverloadedError,
      LocationChangeResponse::PermissionError => LocationChangeResponse::PermissionError,
      LocationChangeResponse::ResolutionError => LocationChangeResponse::ResolutionError,
      LocationChangeResponse::UnsupportedError => LocationChangeResponse::UnsupportedError,
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> LocationChangeResponse<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      LocationChangeResponse::Guest { host, descriptor, name } => {
        LocationChangeResponse::Guest { host: host.convert(converter), descriptor: descriptor.convert(converter), name: converter.convert(name) }
      }
      LocationChangeResponse::Hosting => LocationChangeResponse::Hosting,
      LocationChangeResponse::Location { owner, server, name, descriptor } => LocationChangeResponse::Location {
        owner: converter.convert(owner),
        server: converter.convert(server),
        name: converter.convert(name),
        descriptor: descriptor.convert(converter),
      },
      LocationChangeResponse::NoWhere => LocationChangeResponse::NoWhere,
      LocationChangeResponse::Resolving => LocationChangeResponse::Resolving,
      LocationChangeResponse::WaitingForAsset => LocationChangeResponse::WaitingForAsset,
      LocationChangeResponse::WaitingForPeer => LocationChangeResponse::WaitingForPeer,
      LocationChangeResponse::InternalError => LocationChangeResponse::InternalError,
      LocationChangeResponse::MissingAssetError { assets } => {
        LocationChangeResponse::MissingAssetError { assets: assets.into_iter().map(|asset| converter.convert(asset)).collect() }
      }
      LocationChangeResponse::MissingCapabilitiesError { capabilities } => {
        LocationChangeResponse::MissingCapabilitiesError { capabilities: capabilities.into_iter().map(|cap| converter.convert(cap)).collect() }
      }
      LocationChangeResponse::OverloadedError => LocationChangeResponse::OverloadedError,
      LocationChangeResponse::PermissionError => LocationChangeResponse::PermissionError,
      LocationChangeResponse::ResolutionError => LocationChangeResponse::ResolutionError,
      LocationChangeResponse::UnsupportedError => LocationChangeResponse::UnsupportedError,
    }
  }
  pub fn is_released(&self) -> bool {
    match self {
      LocationChangeResponse::NoWhere
      | LocationChangeResponse::InternalError
      | LocationChangeResponse::OverloadedError
      | LocationChangeResponse::PermissionError
      | LocationChangeResponse::ResolutionError
      | LocationChangeResponse::UnsupportedError
      | LocationChangeResponse::MissingAssetError { .. } => true,
      LocationChangeResponse::MissingCapabilitiesError { .. } => true,
      LocationChangeResponse::Guest { .. }
      | LocationChangeResponse::Hosting
      | LocationChangeResponse::Location { .. }
      | LocationChangeResponse::Resolving
      | LocationChangeResponse::WaitingForAsset
      | LocationChangeResponse::WaitingForPeer => false,
    }
  }
  pub fn into_location_state(self) -> OnlineState<S> {
    match self {
      LocationChangeResponse::Location { descriptor, owner, server, .. } => {
        OnlineState::Location { location: AbsoluteTarget { descriptor, owner, server } }
      }
      LocationChangeResponse::Hosting => OnlineState::Hosting,
      LocationChangeResponse::Guest { host, .. } => OnlineState::Guest { host },
      LocationChangeResponse::Resolving | LocationChangeResponse::WaitingForAsset | LocationChangeResponse::WaitingForPeer => OnlineState::InTransit,
      LocationChangeResponse::NoWhere
      | LocationChangeResponse::InternalError
      | LocationChangeResponse::OverloadedError
      | LocationChangeResponse::PermissionError
      | LocationChangeResponse::ResolutionError
      | LocationChangeResponse::UnsupportedError
      | LocationChangeResponse::MissingAssetError { .. }
      | LocationChangeResponse::MissingCapabilitiesError { .. } => OnlineState::Online,
    }
  }
}
