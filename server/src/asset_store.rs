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
  client: cloud_storage::Client,
  bucket: String,
}

impl AssetStoreConfiguration {
  pub(crate) fn load(self) -> Box<dyn puzzleverse_core::asset_store::AsyncAssetStore> {
    match self {
      AssetStoreConfiguration::FileSystem { directory } => Box::new(puzzleverse_core::asset_store::AsyncStore(
        puzzleverse_core::asset_store::FileSystemStore::new(std::path::Path::new(&directory).to_owned(), [4, 4, 8].iter().cloned()),
      )),
      AssetStoreConfiguration::GoogleCloud { bucket } => Box::new(GoogleCloud { client: cloud_storage::Client::new(), bucket }),
      AssetStoreConfiguration::S3 { bucket, region, access_key, secret_key } => Box::new(S3AssetStore {
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
impl puzzleverse_core::asset_store::AsyncAssetStore for S3AssetStore {
  async fn check(&self, asset: &str) -> bool {
    match self.bucket.head_object(asset).await {
      Ok((result, code)) => code == 200 && result.content_length.unwrap_or(0) > 0,
      Err(e) => {
        eprintln!("Failed to check {} in S3: {}", asset, e);
        false
      }
    }
  }

  async fn pull(&self, asset: &str) -> puzzleverse_core::asset_store::LoadResult {
    match self.bucket.get_object(asset).await {
      Ok(response) => {
        if response.status_code() == 200 {
          match rmp_serde::from_read(std::io::Cursor::new(response.bytes())) {
            Ok(asset) => Ok(asset),
            Err(e) => {
              eprintln!("Failed to decode {}: {}", asset, e);
              Err(puzzleverse_core::asset_store::LoadError::Corrupt)
            }
          }
        } else {
          Err(puzzleverse_core::asset_store::LoadError::Unknown)
        }
      }
      Err(e) => {
        eprintln!("Failed to read {} from S3: {}", asset, e);
        Err(puzzleverse_core::asset_store::LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &puzzleverse_core::asset::Asset) {
    let data = rmp_serde::to_vec(value).expect("Failed to encode asset as MessagePak");
    if let Err(e) = self.bucket.put_object(asset, &data).await {
      println!("Failed to write asset {} to S3: {}", asset, e);
    }
  }
}

#[async_trait::async_trait]
impl puzzleverse_core::asset_store::AsyncAssetStore for GoogleCloud {
  async fn check(&self, asset: &str) -> bool {
    match self.client.object().read(&self.bucket, asset).await {
      Ok(_) => true,
      Err(cloud_storage::Error::Google(e)) => {
        if e.errors_has_reason(&cloud_storage::Reason::NotFound) {
          true
        } else {
          eprintln!("Failed to check {} in Google Cloud Storage: {}", asset, e);
          false
        }
      }
      Err(e) => {
        eprintln!("Failed to check {} in Google Cloud Storage: {}", asset, e);
        false
      }
    }
  }

  async fn pull(&self, asset: &str) -> puzzleverse_core::asset_store::LoadResult {
    match self.client.object().download(&self.bucket, asset).await {
      Ok(data) => match rmp_serde::from_read(std::io::Cursor::new(data)) {
        Ok(asset) => Ok(asset),
        Err(e) => {
          eprintln!("Failed to decode {}: {}", asset, e);
          Err(puzzleverse_core::asset_store::LoadError::Corrupt)
        }
      },
      Err(e) => {
        eprintln!("Failed to fetch {} from Google Cloud Storage: {}", asset, e);
        Err(puzzleverse_core::asset_store::LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &puzzleverse_core::asset::Asset) {
    let data = rmp_serde::to_vec(value).expect("Failed to encode asset as MessagePak");
    if let Err(e) = self.client.object().create(&self.bucket, data, asset, "application/x-puzzleverse").await {
      println!("Failed to write asset {} to Google Cloud Storage: {}", asset, e);
    }
  }
}
