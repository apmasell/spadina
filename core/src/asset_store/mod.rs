use crate::asset::Asset;
use std::fmt::Display;
use std::future::Future;

pub mod file_system_asset_store;

pub trait AssetStore: Send + Sync {
  /// Retrieve an asset from the store
  fn pull(&self, asset: &str) -> impl Future<Output = LoadResult> + Send;
  /// Store a new asset in the store
  fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) -> impl Future<Output = ()> + Send;
}

pub type LoadResult = Result<Asset<String, Vec<u8>>, LoadError>;

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

impl Display for LoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LoadError::Corrupt => f.write_str("asset is corrupt"),
      LoadError::InternalError => f.write_str("internal error"),
      LoadError::Unknown => f.write_str("unknown problem"),
    }
  }
}

impl<T: std::ops::Deref<Target = S> + Send + Sync, S: AssetStore + ?Sized> AssetStore for T {
  async fn pull(&self, asset: &str) -> LoadResult {
    (**self).pull(asset).await
  }

  async fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) {
    (**self).push(asset, value).await
  }
}
