use futures::FutureExt;
use spadina_core::asset_store::AsyncAssetStore;
enum Request {
  Pull(String, tokio::sync::oneshot::Sender<Result<spadina_core::asset::Asset, spadina_core::asset_store::LoadError>>),
  PullAll(std::sync::Arc<str>, Vec<crate::shstr::ShStr>),
  Push(spadina_core::asset::Asset),
  PushAll(std::collections::BTreeMap<crate::shstr::ShStr, Option<spadina_core::asset::Asset>>),
  SetDirectory(std::sync::Weak<super::Directory>),
  Tick,
}

pub struct AssetManager {
  output: tokio::sync::mpsc::Sender<Request>,
}

#[derive(Default)]
struct Waiting {
  peers: Vec<std::sync::Arc<str>>,
  waiters:
    Vec<(tokio::sync::oneshot::Sender<Result<spadina_core::asset::Asset, spadina_core::asset_store::LoadError>>, chrono::DateTime<chrono::Utc>)>,
}

impl AssetManager {
  pub fn new(asset_store: std::sync::Arc<dyn spadina_core::asset_store::AsyncAssetStore>) -> Self {
    let (output, mut input) = tokio::sync::mpsc::channel(500);
    tokio::spawn(async move {
      let mut directory = std::sync::Weak::<super::Directory>::new();
      let mut missing: std::collections::BTreeMap<_, Waiting> = Default::default();
      let mut cache = lru::LruCache::<String, spadina_core::asset::Asset>::new(std::num::NonZeroUsize::new(1025).expect("Bad LRU size"));
      let mut next = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
      while let Some(message) = tokio::time::timeout_at(next, input.recv()).await.transpose().map(|r| r.unwrap_or(Request::Tick)) {
        match message {
          Request::Pull(asset, output) => match cache.get(&asset) {
            Some(value) => std::mem::drop(output.send(Ok(value.clone()))),
            None => match asset_store.pull(&asset).await {
              Ok(value) => {
                cache.put(asset, value.clone());
                std::mem::drop(output.send(Ok(value)));
              }
              Err(spadina_core::asset_store::LoadError::Unknown) => match directory.upgrade() {
                Some(directory) => {
                  match missing.entry(asset.clone()) {
                    std::collections::btree_map::Entry::Vacant(v) => {
                      use rand::seq::SliceRandom;
                      let mut peers = directory.peers();
                      peers.shuffle(&mut rand::thread_rng());
                      v.insert(Waiting { peers, waiters: Default::default() })
                    }
                    std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
                  }
                  .waiters
                  .push((output, chrono::Utc::now() + chrono::Duration::minutes(2)));
                }
                None => std::mem::drop(output.send(Err(spadina_core::asset_store::LoadError::Unknown))),
              },
              Err(e) => std::mem::drop(output.send(Err(e))),
            },
          },
          Request::PullAll(peer, ids) => {
            if let Some(directory) = directory.upgrade() {
              use spadina_core::net::ToWebMessage;
              let mut assets = std::collections::BTreeMap::new();
              for id in ids {
                let result = match cache.get(id.as_str()) {
                  Some(asset) => Some(asset.clone()),
                  None => asset_store.pull(id.as_str()).await.ok(),
                };
                assets.insert(id, result);
              }
              let message = crate::peer::message::PeerMessage::AssetsPush { assets }.as_wsm();
              directory.peer(&peer, |peer| peer.send_raw(message).boxed()).await;
            }
          }
          Request::Push(asset) => {
            let name = asset.principal_hash();
            asset_store.push(&name, &asset).await;
            cache.put(name, asset);
          }
          Request::PushAll(assets) => {
            let mut reset_timer = false;
            for (id, asset) in assets {
              match asset {
                None => {
                  if missing.contains_key(id.as_str()) {
                    reset_timer = true;
                  }
                }
                Some(asset) => {
                  let principal = asset.principal_hash();
                  if principal.as_str() == id.as_str() {
                    asset_store.push(&principal, &asset).await;
                    match missing.remove(&principal) {
                      None => {
                        eprintln!("Got unsolicited asset: {}", &principal);
                      }
                      Some(waiting) => {
                        for (output, _) in waiting.waiters {
                          std::mem::drop(output.send(Ok(asset.clone())));
                        }
                      }
                    }
                    cache.put(principal, asset);
                  }
                }
              }
            }
            if reset_timer {
              next = tokio::time::Instant::now();
            }
          }
          Request::SetDirectory(d) => {
            directory = d;
          }
          Request::Tick => {
            let now = chrono::Utc::now();
            if let Some(directory) = directory.upgrade() {
              missing.retain(|_, waiting| {
                waiting.waiters.retain(|(_, timeout)| timeout > &now);
                !waiting.peers.is_empty() && !waiting.waiters.is_empty()
              });
              let mut peer_requests = std::collections::BTreeMap::<_, Vec<_>>::new();
              for (asset, waiting) in missing.iter_mut() {
                if let Some(peer) = waiting.peers.pop() {
                  peer_requests.entry(peer).or_default().push(asset.as_str());
                }
              }
              for (peer, assets) in peer_requests {
                use spadina_core::net::ToWebMessage;
                let message = crate::peer::message::PeerMessage::AssetsPull { assets }.as_wsm();
                directory.peer(&peer, |peer| peer.send_raw(message).boxed()).await;
              }
            }
            next = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
          }
        }
      }
    });
    AssetManager { output }
  }

  pub(crate) async fn set_directory(&self, directory: std::sync::Weak<super::Directory>) -> () {
    if let Err(_) = self.output.send(Request::SetDirectory(directory)).await {
      panic!("Failed to set directory in asset manager");
    }
  }

  pub(crate) async fn pull_assets(&self, peer_name: std::sync::Arc<str>, assets: Vec<crate::shstr::ShStr>) -> () {
    if let Err(_) = self.output.send(Request::PullAll(peer_name, assets)).await {
      eprintln!("Failed to send pull request from peer to asset manager");
    }
  }
  pub(crate) async fn push_assets(&self, assets: std::collections::BTreeMap<crate::shstr::ShStr, Option<spadina_core::asset::Asset>>) -> () {
    if let Err(_) = self.output.send(Request::PushAll(assets)).await {
      eprintln!("Failed to push assets from peer to asset manager");
    }
  }
}

#[async_trait::async_trait]
impl spadina_core::asset_store::AsyncAssetStore for AssetManager {
  async fn pull(&self, asset: &str) -> spadina_core::asset_store::LoadResult {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Err(_) = self.output.send(Request::Pull(asset.to_string(), tx)).await {
      eprintln!("Failed to pull asset from asset manager");
      return Err(spadina_core::asset_store::LoadError::InternalError);
    }
    match rx.await {
      Ok(result) => result,
      Err(_) => Err(spadina_core::asset_store::LoadError::Unknown),
    }
  }
  async fn push(&self, _: &str, value: &spadina_core::asset::Asset) {
    if let Err(_) = self.output.send(Request::Push(value.clone())).await {
      eprintln!("Failed to push asset into asset manager");
    }
  }
}
