use spadina_core::asset::Asset;
use spadina_core::asset_store::file_system_asset_store::FileSystemAssetStore;
use spadina_core::asset_store::AssetStore;
use spadina_core::reference_converter::ForPacket;
use std::sync::Arc;

#[derive(Debug)]
enum Command {
  Install,
  Connect,
}

impl std::str::FromStr for Command {
  type Err = ();
  fn from_str(src: &str) -> Result<Command, ()> {
    return match src {
      "install" => Ok(Command::Install),
      "connect" => Ok(Command::Connect),
      _ => Err(()),
    };
  }
}

fn main() {
  let mut subcommand = Command::Install;
  let mut args = vec![];
  {
    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("Spadina On-Disk Asset Tool");
    ap.refer(&mut subcommand).required().add_argument("command", argparse::Store, "Command to run");
    ap.refer(&mut args).add_argument("arguments", argparse::List, "Arguments for command");
    ap.stop_on_first_argument(true);
    ap.parse_args_or_exit();
  }
  args.insert(0, format!("subcommand {:?}", subcommand));

  match subcommand {
    Command::Install => {
      let mut directory = String::new();
      let mut zip_path = String::new();
      {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("Installs assets");
        ap.refer(&mut directory).add_option(&["-d", "--directory"], argparse::Store, "The directory to store assets in").required();
        ap.refer(&mut zip_path).add_option(&["-z", "--zip-file"], argparse::Store, "The ZIP file containing the assets").required();
        match ap.parse(args, &mut std::io::stdout(), &mut std::io::stderr()) {
          Ok(()) => (),
          Err(x) => {
            std::process::exit(x);
          }
        }
      }
      let asset_store = Arc::new(FileSystemAssetStore::new(directory, [4, 4, 8].iter().cloned()));

      let mut zip =
        zip::ZipArchive::new(std::fs::OpenOptions::new().read(true).open(zip_path).expect("Cannot open ZIP file")).expect("ZIP file is corrupt");
      let mut runtime = tokio::runtime::Runtime::new().expect("Failed to initialize runtime");
      for i in 0..zip.len() {
        let mut file = zip.by_index(i).expect("Couldn't read file");
        let asset = rmp_serde::from_read::<_, Asset<String, Vec<u8>>>(&mut file).expect("Failed to unpack file");
        let asset_store = asset_store.clone();
        runtime.spawn(async move { asset_store.push(&asset.principal_hash(), &asset.reference(ForPacket)).await });
      }
    }
    Command::Connect => {
      todo!()
    }
  }
}
