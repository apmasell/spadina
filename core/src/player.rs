/// The result of parsing a player identifier
#[derive(Clone, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
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
pub enum PlayerLocationState<S: AsRef<str>> {
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
  Realm { realm: crate::realm::RealmTarget<S> },
  /// The server is not yet contacted; a second response may follow
  ServerDown,
}

impl<S: AsRef<str>> PlayerIdentifier<S> {
  pub fn as_owned_str(&self) -> PlayerIdentifier<String> {
    match self {
      PlayerIdentifier::Local(p) => PlayerIdentifier::Local(p.as_ref().to_string()),
      PlayerIdentifier::Remote { server, player } => {
        PlayerIdentifier::Remote { server: server.as_ref().to_string(), player: player.as_ref().to_string() }
      }
    }
  }
  pub fn as_ref(&self) -> PlayerIdentifier<&str> {
    match self {
      PlayerIdentifier::Local(p) => PlayerIdentifier::Local(p.as_ref()),
      PlayerIdentifier::Remote { server, player } => PlayerIdentifier::Remote { server: server.as_ref(), player: player.as_ref() },
    }
  }
  pub fn convert_str<T: AsRef<str>>(self) -> PlayerIdentifier<T>
  where
    S: Into<T>,
  {
    match self {
      PlayerIdentifier::Local(n) => PlayerIdentifier::Local(n.into()),
      PlayerIdentifier::Remote { server, player } => PlayerIdentifier::Remote { server: server.into(), player: player.into() },
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
  pub fn to_owned(&self) -> PlayerIdentifier<String> {
    match self {
      PlayerIdentifier::Local(name) => PlayerIdentifier::Local(name.as_ref().to_string()),
      PlayerIdentifier::Remote { server, player } => {
        PlayerIdentifier::Remote { server: server.as_ref().to_string(), player: player.as_ref().to_string() }
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

impl std::fmt::Display for PlayerIdentifierError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      PlayerIdentifierError::MisplacedAt => "Multiple '@' in player identifier",
      PlayerIdentifierError::BadDomain => "Invalid server name",
      PlayerIdentifierError::BadPlayerName => "Invalid player name",
    })
  }
}
impl<S: AsRef<str>> PlayerLocationState<S> {
  pub fn as_ref<'a>(&'a self) -> PlayerLocationState<&'a str> {
    match self {
      PlayerLocationState::Unknown => PlayerLocationState::Unknown,
      PlayerLocationState::Invalid => PlayerLocationState::Invalid,
      PlayerLocationState::Offline => PlayerLocationState::Offline,
      PlayerLocationState::Online => PlayerLocationState::Online,
      PlayerLocationState::InTransit => PlayerLocationState::InTransit,
      PlayerLocationState::Hosting => PlayerLocationState::Hosting,
      PlayerLocationState::Guest { host } => PlayerLocationState::Guest { host: host.as_ref() },
      PlayerLocationState::Realm { realm } => PlayerLocationState::Realm { realm: realm.as_ref() },
      PlayerLocationState::ServerDown => PlayerLocationState::ServerDown,
    }
  }
  pub fn convert_str<T: AsRef<str>>(self) -> PlayerLocationState<T>
  where
    S: Into<T>,
  {
    match self {
      PlayerLocationState::Unknown => PlayerLocationState::Unknown,
      PlayerLocationState::Invalid => PlayerLocationState::Invalid,
      PlayerLocationState::Offline => PlayerLocationState::Offline,
      PlayerLocationState::Online => PlayerLocationState::Online,
      PlayerLocationState::InTransit => PlayerLocationState::InTransit,
      PlayerLocationState::Hosting => PlayerLocationState::Hosting,
      PlayerLocationState::Guest { host } => PlayerLocationState::Guest { host: host.convert_str() },
      PlayerLocationState::Realm { realm } => PlayerLocationState::Realm { realm: realm.convert_str() },
      PlayerLocationState::ServerDown => PlayerLocationState::ServerDown,
    }
  }
}
impl<S: AsRef<str>> Default for PlayerLocationState<S> {
  fn default() -> Self {
    PlayerLocationState::Unknown
  }
}
