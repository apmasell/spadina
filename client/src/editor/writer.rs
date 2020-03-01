pub trait AssetWriter {
  type RequestId: Copy;
  fn check(&mut self, id: Self::RequestId) -> Option<Result<String, spadina_core::AssetError>>;
  fn write(&mut self, asset: crate::state::mutator::AssetUploadRequest) -> Self::RequestId;
}
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct LocalRequest(i32);
pub struct LocalWriter {
  id: i32,
  output: tokio::sync::mpsc::UnboundedSender<(i32, crate::state::mutator::AssetUploadRequest)>,
  assets: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<LocalRequest, Result<String, spadina_core::AssetError>>>>,
}
impl LocalWriter {
  pub fn new<S: spadina_core::asset_store::AsyncAssetStore + 'static>(
    author: spadina_core::player::PlayerIdentifier<impl AsRef<str>>,
    asset_store: S,
    runtime: &tokio::runtime::Runtime,
  ) -> Self {
    let author = author.to_string();
    let assets: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<LocalRequest, _>>> = Default::default();
    let (output, mut input) = tokio::sync::mpsc::unbounded_channel();
    let result = Self { assets: assets.clone(), output, id: 0 };
    runtime.spawn(async move {
      while let Some((id, crate::state::mutator::AssetUploadRequest { asset_type, name, tags, licence, compression, data })) = input.recv().await {
        use spadina_core::asset::AssetKind;
        let result = match asset_type.as_str() {
          spadina_core::asset::SimpleRealmDescription::<String, String, String, String>::KIND => {
            spadina_core::asset::verify_submission::<spadina_core::asset::SimpleRealmDescription<String, String, String, String>, _, _>(
              &asset_store,
              compression,
              &data,
            )
            .await
          }
          spadina_core::asset::PuzzleCustom::<String, String, String>::KIND => {
            spadina_core::asset::verify_submission::<spadina_core::asset::PuzzleCustom<String, String, String>, _, _>(
              &asset_store,
              compression,
              &data,
            )
            .await
          }
          <spadina_core::asset::SimpleSprayModel<spadina_core::asset::Mesh, u32, u32, u32> as spadina_core::asset::AssetKind<String>>::KIND => {
            spadina_core::asset::verify_submission::<spadina_core::asset::SimpleSprayModel<spadina_core::asset::Mesh, u32, u32, u32>, _, String>(
              &asset_store,
              compression,
              &data,
            )
            .await
          }
          _ => Err(spadina_core::AssetError::UnknownKind),
        };
        match result {
          Ok(details) => {
            let asset = spadina_core::asset::Asset {
              asset_type,
              author: author.clone(),
              capabilities: details.capabilities,
              children: details.children,
              data,
              compression,
              licence,
              name,
              tags,
              created: chrono::Utc::now(),
            };
            let principal = asset.principal_hash();
            asset_store.push(&principal, &asset).await;
            assets.lock().unwrap().insert(LocalRequest(id), Ok(principal));
          }
          Err(error) => {
            assets.lock().unwrap().insert(LocalRequest(id), Err(error));
          }
        }
      }
    });
    result
  }
}
impl AssetWriter for LocalWriter {
  type RequestId = LocalRequest;

  fn check(&mut self, id: Self::RequestId) -> Option<Result<String, spadina_core::AssetError>> {
    self.assets.lock().unwrap().remove(&id)
  }

  fn write(&mut self, asset: crate::state::mutator::AssetUploadRequest) -> Self::RequestId {
    let id = self.id;
    self.id += 1;
    self.output.send((id, asset)).unwrap();
    LocalRequest(id)
  }
}

impl<S: crate::state::location::LocationState> AssetWriter for crate::state::ServerConnection<S> {
  type RequestId = crate::state::mutator::AssetUpload;

  fn check(&mut self, id: Self::RequestId) -> Option<Result<String, spadina_core::AssetError>> {
    self.assets_upload().try_remove(id)
  }

  fn write(&mut self, asset: crate::state::mutator::AssetUploadRequest) -> Self::RequestId {
    self.assets_upload().push(asset)
  }
}
