/// A message to all players in a realm
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LocationMessage<S: AsRef<str>> {
  /// The contents of the message
  pub body: crate::communication::MessageBody<S>,
  /// The principal of the player that sent the message
  pub sender: crate::player::PlayerIdentifier<S>,
  /// The time the message was sent
  pub timestamp: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum LocationRequest<S: AsRef<str> + Eq + std::hash::Hash> {
  Realm(crate::realm::RealmTarget<S>),
  Guest(crate::player::PlayerIdentifier<S>),
  Host { capabilities: Vec<S>, rules: Vec<crate::access::AccessControl<crate::access::SimpleAccess>>, default: crate::access::SimpleAccess },
  NoWhere,
}
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub enum LocationResponse<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord> {
  Realm {
    /// The ID of the realm
    owner: S,
    /// The server that hosts the realm
    server: S,
    /// The friendly name for the realm
    name: S,
    /// The root asset for the realm
    asset: S,
    /// The realm is listed in the directory
    in_directory: bool,
    /// Some elements in the realm and randomised and this serves as the seed for those random choices. It is provided here, so players see a consistent version of those choices even though the selection is done client-side.
    seed: i32,
    /// Admin controllable parameters for this realm
    settings: crate::realm::RealmSettings<S>,
  },
  Hosting,
  Guest {
    host: crate::player::PlayerIdentifier<S>,
  },
  NoWhere,
  InternalError,
  Resolving,
  PermissionDenied,
  ResolutionFailed,
  MissingCapabilities {
    capabilities: Vec<String>,
  },
}
impl<S: AsRef<str>> LocationMessage<S> {
  pub fn convert_str<T: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord>(self) -> LocationMessage<T>
  where
    S: Into<T>,
  {
    LocationMessage { body: self.body.convert_str(), sender: self.sender.convert_str(), timestamp: self.timestamp }
  }
  pub fn as_owned_str(&self) -> LocationMessage<String> {
    LocationMessage { body: self.body.as_owned_str(), sender: self.sender.as_owned_str(), timestamp: self.timestamp }
  }
}
impl<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord> LocationResponse<S> {
  pub fn convert_str<T: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord>(self) -> LocationResponse<T>
  where
    S: Into<T>,
  {
    match self {
      LocationResponse::Realm { owner, server, name, asset, in_directory, seed, settings } => LocationResponse::Realm {
        owner: owner.into(),
        server: server.into(),
        name: name.into(),
        asset: asset.into(),
        in_directory,
        seed,
        settings: settings.into_iter().map(|(k, v)| (k.into(), v.convert_str())).collect(),
      },
      LocationResponse::Hosting => LocationResponse::Hosting,
      LocationResponse::Guest { host } => LocationResponse::Guest { host: host.convert_str() },
      LocationResponse::NoWhere => LocationResponse::NoWhere,
      LocationResponse::InternalError => LocationResponse::InternalError,
      LocationResponse::Resolving => LocationResponse::Resolving,
      LocationResponse::PermissionDenied => LocationResponse::PermissionDenied,
      LocationResponse::ResolutionFailed => LocationResponse::ResolutionFailed,
      LocationResponse::MissingCapabilities { capabilities } => LocationResponse::MissingCapabilities { capabilities },
    }
  }
  pub fn is_released(&self) -> bool {
    match self {
      LocationResponse::InternalError
      | LocationResponse::PermissionDenied
      | LocationResponse::ResolutionFailed
      | LocationResponse::MissingCapabilities { .. } => true,
      _ => false,
    }
  }
}
