#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Configuration {
  accounts: Vec<ServerConfiguration>,
  client: String,
  private_key: String,
  public_key: String,
}
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct ServerConfiguration {
  player: String,
  server: String,
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  let error_message = match self_update::backends::github::Update::configure()
    .repo_owner("apmasell")
    .repo_name("puzzleverse")
    .bin_name("puzzleverse-client")
    .show_download_progress(true)
    .current_version(self_update::cargo_crate_version!())
    .build()
    .unwrap()
    .update()
  {
    Ok(self_update::Status::UpToDate(_)) => None,
    Ok(self_update::Status::Updated(version)) => {
      println!("Updated to {}", version);
      None
    }
    Err(e) => Some(format!("Failed to update: {}", e)),
  };
  let dirs = directories::ProjectDirs::from("", "", "puzzleverse").unwrap();
  let mut login_file = std::path::PathBuf::new();
  login_file.extend(dirs.config_dir());
  login_file.push("client.json");
  let configuration = (if std::fs::metadata(&login_file).is_ok() {
    match std::fs::OpenOptions::new().read(true).open(&login_file) {
      Ok(login_handle) => match serde_json::from_reader::<_, Configuration>(login_handle) {
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
    Configuration {
      accounts: vec![],
      client: buf.iter().map(|b| format!("{:2X}", b)).collect(),
      private_key: String::from_utf8(keys.private_key_to_pem().expect("Failed to encoding private key")).expect("OpenSSL generate invalid output"),
      public_key: String::from_utf8(keys.public_key_to_pem().expect("Failed to encoding public key")).expect("OpenSSL generate invalid output"),
    }
  });
  serde_json::to_writer(std::fs::OpenOptions::new().write(true).open(&login_file).expect("Failed to write client configuration."), &configuration)
    .expect("Failed to encode client configuration");
  let mut insecure = false;
  {
    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("Puzzleverse Client");
    ap.refer(&mut insecure).add_option(&["-i", "--insecure"], argparse::StoreTrue, "Use HTTP instead HTTPS");
    ap.parse_args_or_exit();
  }
  let mut asset_directory = dirs.cache_dir().to_path_buf();
  asset_directory.push("assets");
  todo!();
}

#[cfg(target_arch = "wasm32")]
fn main() {
  unimplemented!()
}
