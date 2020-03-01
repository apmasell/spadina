#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AssetStoreConfiguration {
  FileSystem { directory: String },
  GoogleCloud { bucket: String },
  S3 { bucket: String, region: String, access_key: String, secret_key: String },
}

struct S3AssetStore {
  bucket: s3::bucket::Bucket,
}

struct GoogleCloud {
  client: google_cloud_storage::client::Client,
  bucket: String,
}

impl AssetStoreConfiguration {
  pub(crate) fn load(self) -> std::sync::Arc<dyn spadina_core::asset_store::AsyncAssetStore> {
    match self {
      AssetStoreConfiguration::FileSystem { directory } => std::sync::Arc::new(spadina_core::asset_store::AsyncStore(
        spadina_core::asset_store::FileSystemStore::new(std::path::Path::new(&directory).to_owned(), [4, 4, 8].iter().cloned()),
      )),
      AssetStoreConfiguration::GoogleCloud { bucket } => {
        std::sync::Arc::new(GoogleCloud { client: google_cloud_storage::client::Client::default(), bucket })
      }
      AssetStoreConfiguration::S3 { bucket, region, access_key, secret_key } => std::sync::Arc::new(S3AssetStore {
        bucket: s3::Bucket::new(
          &bucket,
          region.parse().expect("Invalid S3 region"),
          s3::creds::Credentials::new(Some(&access_key), Some(&secret_key), None, None, None).expect("Failed to process S3 credentials"),
        )
        .expect("Failed to connect to Amazon S3"),
      }),
    }
  }
}

#[async_trait::async_trait]
impl spadina_core::asset_store::AsyncAssetStore for S3AssetStore {
  async fn pull(&self, asset: &str) -> spadina_core::asset_store::LoadResult {
    match self.bucket.get_object(asset).await {
      Ok(response) => {
        if response.status_code() == 200 {
          match rmp_serde::from_slice(response.bytes()) {
            Ok(asset) => Ok(asset),
            Err(e) => {
              eprintln!("Failed to decode {}: {}", asset, e);
              Err(spadina_core::asset_store::LoadError::Corrupt)
            }
          }
        } else {
          Err(spadina_core::asset_store::LoadError::Unknown)
        }
      }
      Err(e) => {
        eprintln!("Failed to read {} from S3: {}", asset, e);
        Err(spadina_core::asset_store::LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &spadina_core::asset::Asset) {
    let data = rmp_serde::to_vec(value).expect("Failed to encode asset as MessagePak");
    if let Err(e) = self.bucket.put_object(asset, &data).await {
      println!("Failed to write asset {} to S3: {}", asset, e);
    }
  }
}

#[async_trait::async_trait]
impl spadina_core::asset_store::AsyncAssetStore for GoogleCloud {
  async fn pull(&self, asset: &str) -> spadina_core::asset_store::LoadResult {
    match self
      .client
      .download_object(
        &google_cloud_storage::http::objects::get::GetObjectRequest {
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
        &google_cloud_storage::http::objects::download::Range::default(),
      )
      .await
    {
      Ok(data) => match rmp_serde::from_read(data.as_slice()) {
        Ok(asset) => Ok(asset),
        Err(e) => {
          eprintln!("Failed to decode {}: {}", asset, e);
          Err(spadina_core::asset_store::LoadError::Corrupt)
        }
      },
      Err(e) => {
        eprintln!("Failed to fetch {} from Google Cloud Storage: {}", asset, e);
        Err(spadina_core::asset_store::LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &spadina_core::asset::Asset) {
    let data = rmp_serde::to_vec(value).expect("Failed to encode asset as MessagePak");
    if let Err(e) = self
      .client
      .upload_object(
        &google_cloud_storage::http::objects::upload::UploadObjectRequest {
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
        &google_cloud_storage::http::objects::upload::UploadType::Simple(google_cloud_storage::http::objects::upload::Media::new(asset.to_string())),
      )
      .await
    {
      println!("Failed to write asset {} to Google Cloud Storage: {}", asset, e);
    }
  }
}
