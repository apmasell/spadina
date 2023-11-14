use ::s3::creds::Credentials;
use google::GoogleCloudAssetStore;
use s3::S3AssetStore;
use spadina_core::asset::Asset;
use spadina_core::asset_store::file_system_asset_store::FileSystemAssetStore;
use spadina_core::asset_store::{AssetStore, LoadResult};
use std::path::PathBuf;

pub mod google;
pub mod manager;
pub mod s3;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssetStoreConfiguration {
  FileSystem { directory: String },
  GoogleCloud { bucket: String },
  S3 { bucket: String, region: String, access_key: String, secret_key: String },
}

pub enum ServerAssetStore {
  FileSystem(FileSystemAssetStore<PathBuf>),
  GoogleCloud(GoogleCloudAssetStore),
  S3(S3AssetStore),
}

impl AssetStoreConfiguration {
  pub(crate) fn load(self) -> ServerAssetStore {
    match self {
      AssetStoreConfiguration::FileSystem { directory } => {
        ServerAssetStore::FileSystem(FileSystemAssetStore::new(std::path::Path::new(&directory).to_owned(), [4, 4, 8].into_iter()))
      }
      AssetStoreConfiguration::GoogleCloud { bucket } => ServerAssetStore::GoogleCloud(GoogleCloudAssetStore::new(bucket)),
      AssetStoreConfiguration::S3 { bucket, region, access_key, secret_key } => ServerAssetStore::S3(
        S3AssetStore::new(
          &bucket,
          region.parse().expect("Invalid S3 region"),
          Credentials::new(Some(&access_key), Some(&secret_key), None, None, None).expect("Failed to process S3 credentials"),
        )
        .expect("Failed to connect to Amazon S3"),
      ),
    }
  }
}
impl AssetStore for ServerAssetStore {
  async fn pull(&self, asset: &str) -> LoadResult {
    match self {
      ServerAssetStore::FileSystem(f) => f.pull(asset).await,
      ServerAssetStore::GoogleCloud(g) => g.pull(asset).await,
      ServerAssetStore::S3(s) => s.pull(asset).await,
    }
  }

  async fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) {
    match self {
      ServerAssetStore::FileSystem(f) => f.push(asset, value).await,
      ServerAssetStore::GoogleCloud(g) => g.push(asset, value).await,
      ServerAssetStore::S3(s) => s.push(asset, value).await,
    }
  }
}
