use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use spadina_core::asset::Asset;
use spadina_core::asset_store::{AssetStore, LoadError, LoadResult};

pub struct GoogleCloudAssetStore {
  client: Client,
  bucket: String,
}

impl GoogleCloudAssetStore {
  pub fn new(bucket: String) -> Self {
    Self { client: Client::new(ClientConfig::default()), bucket }
  }
}

impl AssetStore for GoogleCloudAssetStore {
  async fn pull(&self, asset: &str) -> LoadResult {
    match self
      .client
      .download_object(
        &GetObjectRequest {
          bucket: self.bucket.clone(),
          object: asset.to_string(),
          generation: None,
          if_generation_match: None,
          if_generation_not_match: None,
          if_metageneration_match: None,
          if_metageneration_not_match: None,
          projection: None,
          encryption: None,
        },
        &Range::default(),
      )
      .await
    {
      Ok(data) => match rmp_serde::from_read(data.as_slice()) {
        Ok(asset) => Ok(asset),
        Err(e) => {
          eprintln!("Failed to decode {}: {}", asset, e);
          Err(LoadError::Corrupt)
        }
      },
      Err(e) => {
        eprintln!("Failed to fetch {} from Google Cloud Storage: {}", asset, e);
        Err(LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) {
    let data = rmp_serde::to_vec_named(value).expect("Failed to encode asset as MessagePack");
    if let Err(e) = self
      .client
      .upload_object(
        &UploadObjectRequest {
          bucket: self.bucket.clone(),
          generation: None,
          if_generation_match: None,
          if_generation_not_match: None,
          if_metageneration_match: None,
          if_metageneration_not_match: None,
          kms_key_name: None,
          predefined_acl: None,
          projection: None,
          encryption: None,
        },
        data,
        &UploadType::Simple(Media::new(asset.to_string())),
      )
      .await
    {
      println!("Failed to write asset {} to Google Cloud Storage: {}", asset, e);
    }
  }
}
