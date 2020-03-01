use futures::FutureExt;

use crate::asset::Loaded;

/// A place where assets can be stored for later access
pub trait AssetStore: Send + Sync {
  /// Retrieve an asset from the store
  fn pull(&self, asset: &str) -> LoadResult;
  /// Store a new asset in the store
  fn push(&self, asset: &str, value: &crate::asset::Asset);
}

#[async_trait::async_trait]
pub trait AsyncAssetStore: Send + Sync {
  /// Retrieve an asset from the store
  async fn pull(&self, asset: &str) -> LoadResult;
  /// Store a new asset in the store
  async fn push(&self, asset: &str, value: &crate::asset::Asset);
}

pub struct AsyncStore<T>(pub T);

pub type LoadResult = Result<crate::asset::Asset, LoadError>;

/// The type of result when attempting to pull an asset from the store
#[derive(Debug, Clone, Copy)]
pub enum LoadError {
  /// The asset was found, but cannot be decoded
  Corrupt,
  /// Some other error occurred access the asset
  InternalError,
  /// The asset was not found in the asset store
  Unknown,
}

impl std::fmt::Display for LoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LoadError::Corrupt => f.write_str("asset is corrupt"),
      LoadError::InternalError => f.write_str("internal error"),
      LoadError::Unknown => f.write_str("unknown problem"),
    }
  }
}

/// An asset store backed by a directory on the file system
pub struct FileSystemStore<T: AsRef<std::path::Path> + Send + Sync> {
  root: T,
  splits: Vec<usize>,
}

impl<T: AsRef<std::path::Path> + Send + Sync> FileSystemStore<T> {
  /// Create a new file store backed by a directory
  /// * `directory` - the directory root holding the asset files
  /// * `splits` - since most file systems would get angry with a directory containing many files, this creates hierarchical directory structure by breaking up an asset ID. A split of 4, 2, will transform `AAAABBCCCC` into `AAAA/BB/CCCC`
  pub fn new(directory: T, splits: impl IntoIterator<Item = usize>) -> FileSystemStore<T> {
    FileSystemStore { root: directory, splits: splits.into_iter().collect() }
  }
  fn get_path(&self, asset: &str) -> std::path::PathBuf {
    let mut result = self.root.as_ref().to_path_buf();
    result.extend(self.splits.iter().scan(0 as usize, |s, &l| match asset.get(*s..(*s + l)) {
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
impl<T: AsRef<std::path::Path> + Send + Sync> AssetStore for FileSystemStore<T> {
  fn pull(&self, asset: &str) -> LoadResult {
    match std::fs::File::open(&self.get_path(asset)) {
      Ok(b) => match rmp_serde::from_read::<_, crate::asset::Asset>(b) {
        Ok(result) => Ok(result),
        Err(e) => {
          eprintln!("Asset {} is corrupt: {}", asset, e);
          Err(LoadError::Corrupt)
        }
      },
      Err(e) => {
        if e.kind() == std::io::ErrorKind::NotFound {
          Err(LoadError::Unknown)
        } else {
          eprintln!("Failed to get asset {}: {}", asset, e);
          Err(LoadError::InternalError)
        }
      }
    }
  }

  fn push(&self, asset: &str, value: &crate::asset::Asset) {
    let path = self.get_path(asset);
    if let Some(parent) = path.parent() {
      if let Err(e) = std::fs::create_dir_all(parent) {
        eprintln!("Failed to create {:?}: {}", parent, e);
        return;
      }
    }
    match std::fs::OpenOptions::new().write(true).open(&path) {
      Err(e) => {
        eprintln!("Failed to open file for asset {:?}: {}", path, e);
      }
      Ok(writer) => {
        if let Err(e) = rmp_serde::encode::write(&mut std::io::BufWriter::new(writer), value) {
          eprintln!("Failed to write asset {:?}: {}", path, e);
        }
      }
    }
  }
}

#[async_trait::async_trait]
impl<'a, T: AssetStore> AsyncAssetStore for AsyncStore<T> {
  async fn pull(&self, asset: &str) -> LoadResult {
    self.0.pull(asset)
  }

  async fn push(&self, asset: &str, value: &crate::asset::Asset) {
    self.0.push(asset, value)
  }
}

impl<T: std::ops::Deref<Target = S> + Send + Sync, S: AssetStore + ?Sized> AssetStore for T {
  fn pull(&self, asset: &str) -> LoadResult {
    (**self).pull(asset)
  }

  fn push(&self, asset: &str, value: &crate::asset::Asset) {
    (**self).push(asset, value)
  }
}

#[async_trait::async_trait]
impl<T: std::ops::Deref<Target = S> + Send + Sync, S: AsyncAssetStore + ?Sized> AsyncAssetStore for T {
  async fn pull(&self, asset: &str) -> LoadResult {
    (**self).pull(asset).await
  }

  async fn push(&self, asset: &str, value: &crate::asset::Asset) {
    (**self).push(asset, value).await
  }
}
pub struct CachingResourceMapper<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + Clone> {
  audio_cache: std::collections::BTreeMap<S, Loaded<crate::asset::AssetAnyAudio, S>>,
  custom_cache: std::collections::BTreeMap<S, Loaded<crate::asset::AssetAnyCustom<S>, S>>,
  model_cache: std::collections::BTreeMap<S, Loaded<crate::asset::AssetAnyModel, S>>,
}
impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Send + Sync + Clone + 'static>
  CachingResourceMapper<S>
where
  for<'a> &'a str: Into<S>,
{
  pub fn new() -> Self {
    Self { audio_cache: Default::default(), custom_cache: Default::default(), model_cache: Default::default() }
  }
  pub fn install(&mut self, asset_id: S, asset: crate::asset::Asset) -> Result<(), crate::AssetError> {
    use crate::asset::AssetKind;
    match asset.asset_type.as_str() {
      crate::asset::PuzzleCustom::<_, _, S>::KIND => {
        let value = crate::asset::AssetAnyCustom::<S>::load(self, asset)?;
        self.custom_cache.insert(asset_id.clone(), Loaded::new(asset_id, value));
        Ok(())
      }
      <crate::asset::SimpleSprayModel<_, _, _, _> as AssetKind<S>>::KIND => {
        let value = crate::asset::AssetAnyModel::load(self, asset)?;
        self.model_cache.insert(asset_id.clone(), Loaded::new(asset_id, value));
        Ok(())
      }
      _ => Err(crate::AssetError::UnknownKind),
    }
  }

  pub fn install_from<'a, T: AsyncAssetStore>(
    &'a mut self,
    store: &'a T,
    asset_id: S,
  ) -> futures::future::BoxFuture<'a, Result<(), crate::AssetError>> {
    async {
      if self.audio_cache.contains_key(&asset_id) || self.custom_cache.contains_key(&asset_id) || self.model_cache.contains_key(&asset_id) {
        return Ok(());
      }
      match store.pull(asset_id.as_ref()).await {
        Ok(asset) => {
          for child in &asset.children {
            self.install_from(store, child.as_str().into()).await?;
          }
          self.install(asset_id, asset)
        }
        Err(e) => Err(match e {
          LoadError::Corrupt => crate::AssetError::DecodeFailure,
          LoadError::InternalError => crate::AssetError::Invalid,
          LoadError::Unknown => crate::AssetError::Missing(vec![asset_id.to_string()]),
        }),
      }
    }
    .boxed()
  }
}
impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Send + Sync + Clone + Default + 'static>
  Default for CachingResourceMapper<S>
where
  for<'a> &'a str: Into<S>,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + Clone + 'static> crate::asset::ResourceMapper<S, S, S> for CachingResourceMapper<S> {
  type Audio = crate::asset::Loaded<crate::asset::AssetAnyAudio, S>;

  type Custom = crate::asset::Loaded<crate::asset::AssetAnyCustom<S>, S>;

  type Model = crate::asset::Loaded<crate::asset::AssetAnyModel, S>;

  type Error = crate::AssetError;
  fn resolve_audio(&mut self, audio: S) -> Result<Self::Audio, Self::Error> {
    self.audio_cache.get(&audio).cloned().ok_or(crate::AssetError::Missing(vec![audio.as_ref().to_string()]))
  }
  fn resolve_custom(&mut self, custom: S) -> Result<Self::Custom, Self::Error> {
    self.custom_cache.get(&custom).cloned().ok_or(crate::AssetError::Missing(vec![custom.as_ref().to_string()]))
  }
  fn resolve_model(&mut self, model: S) -> Result<Self::Model, Self::Error> {
    self.model_cache.get(&model).cloned().ok_or(crate::AssetError::Missing(vec![model.as_ref().to_string()]))
  }
}
