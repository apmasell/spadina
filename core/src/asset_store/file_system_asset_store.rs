use crate::asset::Asset;
use crate::asset_store::{AssetStore, LoadError, LoadResult};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// An asset store backed by a directory on the file system
pub struct FileSystemAssetStore<T: AsRef<Path> + Send + Sync> {
  root: T,
  splits: Vec<usize>,
}

impl<T: AsRef<Path> + Send + Sync> FileSystemAssetStore<T> {
  /// Create a new file store backed by a directory
  /// * `directory` - the directory root holding the asset files
  /// * `splits` - since most file systems would get angry with a directory containing many files, this creates hierarchical directory structure by breaking up an asset ID. A split of 4, 2, will transform `AAAABBCCCC` into `AAAA/BB/CCCC`
  pub fn new(directory: T, splits: impl IntoIterator<Item = usize>) -> FileSystemAssetStore<T> {
    FileSystemAssetStore { root: directory, splits: splits.into_iter().collect() }
  }
  fn get_path(&self, asset: &str) -> std::path::PathBuf {
    let mut result = self.root.as_ref().to_path_buf();
    result.extend(self.splits.iter().scan(0usize, |s, &l| match asset.get(*s..(*s + l)) {
      Some(output) => {
        *s += l;
        if output.is_empty() {
          None
        } else {
          Some(output.to_string())
        }
      }
      None => None,
    }));
    result.push(asset);
    result
  }
}

impl<T: AsRef<Path> + Send + Sync> AssetStore for FileSystemAssetStore<T> {
  async fn pull(&self, asset: &str) -> LoadResult {
    match fs::File::open(&self.get_path(asset)) {
      Ok(b) => match rmp_serde::from_read::<_, Asset<_, _>>(b) {
        Ok(result) => Ok(result),
        Err(e) => {
          eprintln!("Asset {} is corrupt: {}", asset, e);
          Err(LoadError::Corrupt)
        }
      },
      Err(e) => {
        if e.kind() == ErrorKind::NotFound {
          Err(LoadError::Unknown)
        } else {
          eprintln!("Failed to get asset {}: {}", asset, e);
          Err(LoadError::InternalError)
        }
      }
    }
  }

  async fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) {
    let path = self.get_path(asset);
    if let Some(parent) = path.parent() {
      if let Err(e) = fs::create_dir_all(parent) {
        eprintln!("Failed to create {:?}: {}", parent, e);
        return;
      }
    }
    match fs::OpenOptions::new().write(true).open(&path) {
      Err(e) => {
        eprintln!("Failed to open file for asset {:?}: {}", path, e);
      }
      Ok(writer) => {
        if let Err(e) = rmp_serde::encode::write_named(&mut std::io::BufWriter::new(writer), value) {
          eprintln!("Failed to write asset {:?}: {}", path, e);
        }
      }
    }
  }
}
