use crate::reference_converter::{Converter, Referencer};
use std::cmp::Ordering;
use std::sync::Arc;

/// The result of parsing a player identifier
#[derive(Clone, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum PlayerIdentifier<S: AsRef<str>> {
  /// The player is an name for a player on the local server
  Local(S),
  /// The player is a name for a player on the remote server
  Remote { server: S, player: S },
}

pub enum PlayerIdentifierError {
  MisplacedAt,
  BadDomain,
  BadPlayerName,
}
/// When querying the online status and location of another player, this is the response
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum OnlineState<S: AsRef<str>> {
  /// Player state is not visible
  Unknown,
  /// Not a valid player
  Invalid,
  /// Offline
  Offline,
  /// Online, but location is unavailable
  Online,
  /// Online and not in a realm
  InTransit,
  /// Online and hosting
  Hosting,
  /// Online and hosted
  Guest { host: PlayerIdentifier<S> },
  /// Online and in a particular realm
  Location { location: crate::location::target::AbsoluteTarget<S> },
  /// The server is not yet contacted; a second response may follow
  ServerDown,
}

impl<S: AsRef<str>> PlayerIdentifier<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> PlayerIdentifier<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      PlayerIdentifier::Local(p) => PlayerIdentifier::Local(reference.convert(p)),
      PlayerIdentifier::Remote { server, player } => {
        PlayerIdentifier::Remote { server: reference.convert(server), player: reference.convert(player) }
      }
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> PlayerIdentifier<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      PlayerIdentifier::Local(n) => PlayerIdentifier::Local(converter.convert(n)),
      PlayerIdentifier::Remote { server, player } => {
        PlayerIdentifier::Remote { server: converter.convert(server), player: converter.convert(player) }
      }
    }
  }
  pub fn get_player(&self) -> &S {
    match self {
      PlayerIdentifier::Local(s) => s,
      PlayerIdentifier::Remote { player, .. } => player,
    }
  }
  pub fn get_server<'a>(&'a self, local_server: &'a str) -> &'a str {
    match self {
      PlayerIdentifier::Local(_) => local_server,
      PlayerIdentifier::Remote { server, .. } => server.as_ref(),
    }
  }
  pub fn globalize(self, server: impl Into<S>) -> Self {
    match self {
      PlayerIdentifier::Local(player) => PlayerIdentifier::Remote { server: server.into(), player },
      PlayerIdentifier::Remote { server, player } => PlayerIdentifier::Remote { server, player },
    }
  }
  pub fn into_player(self) -> S {
    match self {
      PlayerIdentifier::Local(player) => player,
      PlayerIdentifier::Remote { player, .. } => player,
    }
  }
  pub fn localize(self, local_server: &str) -> Self {
    match self {
      PlayerIdentifier::Local(player) => PlayerIdentifier::Local(player),
      PlayerIdentifier::Remote { server, player } => {
        if server.as_ref() == local_server {
          PlayerIdentifier::Local(player)
        } else {
          PlayerIdentifier::Remote { server, player }
        }
      }
    }
  }
}
impl std::str::FromStr for PlayerIdentifier<String> {
  type Err = PlayerIdentifierError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    use unicode_normalization::UnicodeNormalization;
    let parts: Vec<_> = s.rsplitn(2, '@').collect();
    match parts[..] {
      [name] => match percent_encoding::percent_decode_str(name).decode_utf8() {
        Ok(name) => {
          if name.is_empty() {
            Err(PlayerIdentifierError::BadPlayerName)
          } else {
            Ok(PlayerIdentifier::Local(name.as_ref().nfc().collect()))
          }
        }
        Err(_) => Err(PlayerIdentifierError::BadPlayerName),
      },
      [name, server_name_raw] => match percent_encoding::percent_decode_str(name).decode_utf8() {
        Err(_) => Err(PlayerIdentifierError::BadPlayerName),
        Ok(name) => {
          if name.is_empty() {
            Err(PlayerIdentifierError::BadPlayerName)
          } else {
            match idna::domain_to_unicode(server_name_raw) {
              (server_name, Ok(())) => Ok(PlayerIdentifier::Remote { server: server_name, player: name.nfc().collect() }),
              _ => Err(PlayerIdentifierError::BadDomain),
            }
          }
        }
      },
      _ => Err(PlayerIdentifierError::MisplacedAt),
    }
  }
}

impl<S: AsRef<str>> std::fmt::Display for PlayerIdentifier<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Local(name) => percent_encoding::utf8_percent_encode(name.as_ref(), &percent_encoding::NON_ALPHANUMERIC).fmt(f),
      Self::Remote { server, player } => {
        percent_encoding::utf8_percent_encode(player.as_ref(), &percent_encoding::NON_ALPHANUMERIC).fmt(f)?;
        f.write_str("@")?;
        f.write_str(server.as_ref())
      }
    }
  }
}

impl<S: AsRef<str>> Eq for PlayerIdentifier<S> {}

impl<S: AsRef<str>> PartialEq<Self> for PlayerIdentifier<S> {
  fn eq(&self, other: &Self) -> bool {
    self.cmp(other) == Ordering::Equal
  }
}

impl<S: AsRef<str>> PartialOrd<Self> for PlayerIdentifier<S> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<S: AsRef<str>> Ord for PlayerIdentifier<S> {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (PlayerIdentifier::Local(s), PlayerIdentifier::Local(o)) => s.as_ref().cmp(o.as_ref()),
      (PlayerIdentifier::Remote { server, player }, PlayerIdentifier::Remote { server: other_server, player: other_player }) => {
        player.as_ref().cmp(other_player.as_ref()).then_with(|| server.as_ref().cmp(other_server.as_ref()))
      }
      (PlayerIdentifier::Local(s), PlayerIdentifier::Remote { player, .. }) => s.as_ref().cmp(player.as_ref()).then(Ordering::Less),
      (PlayerIdentifier::Remote { player, .. }, PlayerIdentifier::Local(s)) => player.as_ref().cmp(s.as_ref()).then(Ordering::Greater),
    }
  }
}

impl std::fmt::Display for PlayerIdentifierError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      PlayerIdentifierError::MisplacedAt => "Multiple '@' in player identifier",
      PlayerIdentifierError::BadDomain => "Invalid server name",
      PlayerIdentifierError::BadPlayerName => "Invalid player name",
    })
  }
}
impl<S: AsRef<str>> OnlineState<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> OnlineState<R::Output<'a>>
  where
    <R as Referencer<S>>::Output<'a>: AsRef<str>,
  {
    match self {
      OnlineState::Unknown => OnlineState::Unknown,
      OnlineState::Invalid => OnlineState::Invalid,
      OnlineState::Offline => OnlineState::Offline,
      OnlineState::Online => OnlineState::Online,
      OnlineState::InTransit => OnlineState::InTransit,
      OnlineState::Hosting => OnlineState::Hosting,
      OnlineState::Guest { host } => OnlineState::Guest { host: host.reference(reference) },
      OnlineState::Location { location } => OnlineState::Location { location: location.reference(reference) },
      OnlineState::ServerDown => OnlineState::ServerDown,
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> OnlineState<C::Output>
  where
    <C as Converter<S>>::Output: AsRef<str>,
  {
    match self {
      OnlineState::Unknown => OnlineState::Unknown,
      OnlineState::Invalid => OnlineState::Invalid,
      OnlineState::Offline => OnlineState::Offline,
      OnlineState::Online => OnlineState::Online,
      OnlineState::InTransit => OnlineState::InTransit,
      OnlineState::Hosting => OnlineState::Hosting,
      OnlineState::Guest { host } => OnlineState::Guest { host: host.convert(converter) },
      OnlineState::Location { location } => OnlineState::Location { location: location.convert(converter) },
      OnlineState::ServerDown => OnlineState::ServerDown,
    }
  }
}
impl<S: AsRef<str>> Default for OnlineState<S> {
  fn default() -> Self {
    OnlineState::Unknown
  }
}

pub type SharedPlayerIdentifier = PlayerIdentifier<Arc<str>>;
