pub struct EmoteCache(lru::LruCache<String, Result<std::sync::Arc<Emote>, ()>>);
pub struct Emote {
  pub duration: u32,
  pub animation: (),
}
pub enum EmoteResult {
  Emote(std::sync::Arc<Emote>),
  Bad,
  Missing,
}

impl EmoteCache {
  pub fn new() -> Self {
    Self(lru::LruCache::new(std::num::NonZeroUsize::try_from(100).unwrap()))
  }
  pub async fn get(&mut self, id: &str, asset_store: impl spadina_core::asset_store::AsyncAssetStore) -> EmoteResult {
    if let Some(result) = self.0.get(id) {
      match result {
        Ok(value) => EmoteResult::Emote(value.clone()),
        Err(()) => EmoteResult::Bad,
      }
    } else {
      match asset_store.pull(id).await {
        Ok(asset) => todo!(),
        Err(spadina_core::asset_store::LoadError::Unknown) => EmoteResult::Missing,
        Err(e) => {
          eprintln!("Failed to load emote {}: {}", id, e);
          EmoteResult::Bad
        }
      }
    }
  }
}
