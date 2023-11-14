use crate::accounts::configuration::AccountsConfiguration;
use crate::asset_store::AssetStoreConfiguration;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ServerConfiguration {
  pub asset_store: AssetStoreConfiguration,
  pub authentication: AccountsConfiguration,
  pub bind_address: Option<String>,
  pub certificate: Option<PathBuf>,
  pub name: String,
  pub unix_socket: Option<String>,
}

impl ServerConfiguration {
  pub fn load() -> (Self, PathBuf) {
    let mut configuration_file: String = "spadina.config".into();
    {
      let mut ap = argparse::ArgumentParser::new();
      ap.set_description("Spadina Server");
      ap.refer(&mut configuration_file).add_option(&["-c", "--config"], argparse::Store, "Set the configuration JSON file");
      ap.parse_args_or_exit();
    }
    let mut configuration_file = PathBuf::try_from(configuration_file).expect("Invalid configuration path");
    let mut config: ServerConfiguration = toml::from_str(&std::fs::read_to_string(&configuration_file).expect("Cannot open configuration file"))
      .expect("Cannot parse configuration file.");
    let name = spadina_core::net::parse_server_name(&config.name).expect("Invalid server name. Must be a valid DNS name.");
    config.name = name;
    configuration_file.set_extension("db");
    (config, configuration_file)
  }
}
