#[cfg(not(target_arch = "wasm32"))]
pub struct Configuration {
  data: ConfigurationFile,
  path: std::path::PathBuf,
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ConfigurationFile {
  accounts: Vec<ServerConfiguration>,
  client: String,
  private_key: String,
  public_key: String,
}
pub enum ConnectionStringError {
  FileSystem(std::io::Error),
  NoSocket,
  Parse(spadina_core::player::PlayerIdentifierError),
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum ServerConfiguration {
  Remote { player: String, server: String },
  Socket(String),
}

#[cfg(not(target_arch = "wasm32"))]
impl Configuration {
  pub fn connections(&self) -> impl Iterator<Item = (usize, &'_ ServerConfiguration)> {
    self.data.accounts.iter().enumerate()
  }
  pub fn load() -> Self {
    let dirs = directories::ProjectDirs::from("", "", "spadina").unwrap();
    let mut path = std::path::PathBuf::new();
    path.extend(dirs.config_dir());
    path.push("client.json");
    let data = (if std::fs::metadata(&path).is_ok() {
      match std::fs::OpenOptions::new().read(true).open(&path) {
        Ok(login_handle) => match serde_json::from_reader::<_, ConfigurationFile>(login_handle) {
          Ok(config) => Some(config),
          Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            None
          }
        },
        Err(e) => {
          eprintln!("Failed to open configuration: {}", e);
          None
        }
      }
    } else {
      None
    })
    .unwrap_or_else(|| {
      let keys = openssl::ec::EcKey::generate(
        &openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::SECP256K1).expect("Unable to find elliptic curve group."),
      )
      .expect("Unable to generate encryption key");
      let mut buf = [0; 32];
      openssl::rand::rand_bytes(&mut buf).unwrap();
      let mut client = String::new();
      for b in buf {
        use std::fmt::Write;
        write!(&mut client, "{:2X}", b).expect("Failed to generate client ID");
      }
      ConfigurationFile {
        accounts: vec![],
        client,
        private_key: String::from_utf8(keys.private_key_to_pem().expect("Failed to encoding private key")).expect("OpenSSL generate invalid output"),
        public_key: String::from_utf8(keys.public_key_to_pem().expect("Failed to encoding public key")).expect("OpenSSL generate invalid output"),
      }
    });
    serde_json::to_writer(std::fs::OpenOptions::new().write(true).open(&path).expect("Failed to write client configuration."), &data)
      .expect("Failed to encode client configuration");
    Configuration { data, path }
  }
  pub fn process_connection_string(&mut self, connection_string: &str) -> Result<ServerConfiguration, ConnectionStringError> {
    let result = if std::fs::metadata(connection_string)?.file_type().is_file() {
      Ok(ServerConfiguration::Socket(connection_string.to_string()))
    } else {
      match connection_string.parse()? {
        spadina_core::player::PlayerIdentifier::Remote { server, player } => Ok(ServerConfiguration::Remote { player, server }),
        spadina_core::player::PlayerIdentifier::Local(_) => Err(ConnectionStringError::NoSocket),
      }
    }?;
    if !self.data.accounts.contains(&result) {
      self.data.accounts.push(result.clone());
      serde_json::to_writer(std::fs::OpenOptions::new().write(true).open(&self.path).expect("Failed to write client configuration."), &self.data)
        .expect("Failed to encode client configuration");
    }
    Ok(result)
  }
  pub fn remove(&mut self, item: usize) {
    if item < self.data.accounts.len() {
      self.data.accounts.swap_remove(item);
    }
  }
}

impl From<std::io::Error> for ConnectionStringError {
  fn from(e: std::io::Error) -> Self {
    ConnectionStringError::FileSystem(e)
  }
}
impl From<spadina_core::player::PlayerIdentifierError> for ConnectionStringError {
  fn from(e: spadina_core::player::PlayerIdentifierError) -> Self {
    ConnectionStringError::Parse(e)
  }
}
impl std::fmt::Display for ConnectionStringError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ConnectionStringError::FileSystem(e) => {
        f.write_str("failed to check socket: ")?;
        e.fmt(f)
      }
      ConnectionStringError::NoSocket => f.write_str("socket not found (maybe you meant to put an @ in your name"),
      ConnectionStringError::Parse(e) => {
        f.write_str("cannot parse login: ")?;
        e.fmt(f)
      }
    }
  }
}
