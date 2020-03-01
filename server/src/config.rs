#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ServerConfiguration {
  pub asset_store: crate::asset_store::AssetStoreConfiguration,
  pub authentication: crate::auth::AuthConfiguration,
  pub bind_address: Option<String>,
  pub certificate: Option<std::path::PathBuf>,
  pub database_url: String,
  pub default_realm: Option<String>,
  pub name: String,
  pub unix_socket: Option<String>,
}

impl ServerConfiguration {
  pub fn load() -> Self {
    let mut configuration_file: String = "spadina.config".into();
    {
      let mut ap = argparse::ArgumentParser::new();
      ap.set_description("Spadina Server");
      ap.refer(&mut configuration_file).add_option(&["-c", "--config"], argparse::Store, "Set the configuration JSON file");
      ap.parse_args_or_exit();
    }
    let mut config: ServerConfiguration = serde_json::from_reader(std::fs::File::open(&configuration_file).expect("Cannot open configuration file"))
      .expect("Cannot parse configuration file.");
    let name = spadina_core::net::parse_server_name(&config.name).expect("Invalid server name. Must be a valid DNS name.");
    config.name = name;
    config
  }
}
