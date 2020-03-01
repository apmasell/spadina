use spadina_core::asset_store::AssetStore;
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

  let asset_store = spadina_core::asset_store::FileSystemStore::new(std::path::Path::new(&directory), [4, 4, 8].iter().cloned());

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

      let mut zip =
        zip::ZipArchive::new(std::fs::OpenOptions::new().read(true).open(zip_path).expect("Cannot open ZIP file")).expect("ZIP file is corrupt");
      for i in 0..zip.len() {
        let mut file = zip.by_index(i).expect("Couldn't read file");
        if !asset_store.check(file.name()) {
          let asset = rmp_serde::from_read(&mut file).expect("Failed to unpack file");
          asset_store.push(file.name(), &asset);
        }
      }
    }
  }
}
