use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::error::S3Error;
use s3::Region;
use spadina_core::asset::Asset;
use spadina_core::asset_store::{AssetStore, LoadError, LoadResult};

pub struct S3AssetStore {
  bucket: Box<Bucket>,
}
impl S3AssetStore {
  pub fn new(bucket: &str, region: Region, credentials: Credentials) -> Result<Self, S3Error> {
    Ok(S3AssetStore { bucket: Bucket::new(bucket, region, credentials)? })
  }
}

impl AssetStore for S3AssetStore {
  async fn pull(&self, asset: &str) -> LoadResult {
    match self.bucket.get_object(asset).await {
      Ok(response) => {
        if response.status_code() == 200 {
          match rmp_serde::from_slice(response.bytes()) {
            Ok(asset) => Ok(asset),
            Err(e) => {
              eprintln!("Failed to decode {}: {}", asset, e);
              Err(LoadError::Corrupt)
            }
          }
        } else {
          Err(LoadError::Unknown)
        }
      }
      Err(e) => {
        eprintln!("Failed to read {} from S3: {}", asset, e);
        Err(LoadError::InternalError)
      }
    }
  }

  async fn push(&self, asset: &str, value: &Asset<&str, &[u8]>) {
    let data = rmp_serde::to_vec_named(value).expect("Failed to encode asset as MessagePak");
    if let Err(e) = self.bucket.put_object(asset, &data).await {
      println!("Failed to write asset {} to S3: {}", asset, e);
    }
  }
}
