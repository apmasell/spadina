use crate::asset_store::ServerAssetStore;
use crate::directory::Directory;
use crate::gc_map::time_use::TimeUse;
use crate::gc_map::waiting::{Communication, Waiting};
use crate::gc_map::{GarbageCollectorMap, Launcher, TrackableValue};
use crate::stream_map::{StreamableEntry, StreamsUnorderedMap};
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::{stream, FutureExt, StreamExt, TryStreamExt};
use rand::thread_rng;
use spadina_core::asset::variants::AllSupportedAssets;
use spadina_core::asset::Asset;
use spadina_core::asset_store::{AssetStore, LoadError};
use spadina_core::controller::GenericControllerTemplate;
use spadina_core::net::server::AssetError;
use spadina_core::reference_converter::{ForPacket, IntoSharedState};
use spadina_core::shared_ref::SharedRef;
use std::collections::BTreeMap;
use std::mem::swap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Duration};

pub enum AssetRequest {
  Pull(Arc<str>, oneshot::Sender<Arc<Asset<Arc<str>, Arc<[u8]>>>>, bool),
  Realm(Arc<str>, oneshot::Sender<RealmTemplate>),
  Upload(Asset<String, Vec<u8>>, oneshot::Sender<Result<(), AssetError>>),
}
pub struct AnyPlayers(Arc<AtomicBool>);
struct FindAsset<'a, Store: AssetStore>(&'a Arc<Store>, &'a Directory, bool);
struct FindRealm<'a>(&'a Directory);

pub type AssetManager = mpsc::Sender<AssetRequest>;

#[derive(Clone)]
pub enum RealmTemplate {
  Found(GenericControllerTemplate),
  NotFound(Arc<str>),
  MissingCapabilities(Vec<Arc<str>>),
  Invalid,
}
pub type WaitingAsset = Waiting<Arc<Asset<Arc<str>, Arc<[u8]>>>, BoxFuture<'static, Option<Arc<Asset<Arc<str>, Arc<[u8]>>>>>, AnyPlayers>;
pub type WaitingRealm = Waiting<RealmTemplate, BoxFuture<'static, Option<RealmTemplate>>, ()>;

pub fn start(store: ServerAssetStore, directory: Directory, mut rx: mpsc::Receiver<AssetRequest>) {
  tokio::spawn(async move {
    enum Event {
      Quit,
      Request(AssetRequest),
    }
    let mut death = directory.access_management.give_me_death();
    let store = Arc::new(store);
    let mut assets = StreamsUnorderedMap::new(GarbageCollectorMap::<Arc<str>, WaitingAsset, TimeUse>::new(500));
    let mut realms = StreamsUnorderedMap::new(GarbageCollectorMap::<Arc<str>, WaitingRealm, TimeUse>::new(100));
    loop {
      let message: Event = tokio::select! { biased;
          _ = death.recv() => Event::Quit,
          r = rx.recv() => r.map(Event::Request).unwrap_or(Event::Quit),
      };
      match message {
        Event::Quit => break,
        Event::Request(AssetRequest::Pull(id, waiter, search_peers)) => {
          assets.mutate().upsert(id, FindAsset(&store, &directory, search_peers)).add(waiter, search_peers)
        }
        Event::Request(AssetRequest::Realm(id, waiter)) => realms.mutate().upsert(id, FindRealm(&directory)).add(waiter, ()),
        Event::Request(AssetRequest::Upload(asset, output)) => match asset
          .deserialize_inner::<AllSupportedAssets<String>>()
          .map_err(|_| AssetError::DecodeFailure)
          .and_then(|a| a.validate().map_err(|_| AssetError::Invalid))
        {
          Err(e) => {
            let _ = output.send(Err(e));
          }
          Ok(()) => {
            let id = asset.principal_hash();
            let _ = output.send(Ok(()));
            if let Some(Waiting::Value(_)) = assets.get(id.as_str()) {
              continue;
            } else if store.pull(&id).await.is_err() {
              store.push(&id, &asset.reference(ForPacket)).await;
              if let Some(mut current) = assets.entry(Arc::from(id)) {
                let asset = Arc::new(asset.convert(IntoSharedState));
                let mut alternate = Waiting::Value(asset.clone());
                swap(&mut alternate, current.get_mut());
                if let Waiting::Pending(_, _, pending) = alternate {
                  for waiter in pending {
                    let _ = waiter.send(asset.clone());
                  }
                }
              }
            }
          }
        },
      }
      assets.mutate().perform_gc();
      realms.mutate().perform_gc();
    }
  });
}
async fn pull<Store: AssetStore>(
  id: Arc<str>,
  search_peers: Arc<AtomicBool>,
  store: Arc<Store>,
  directory: Directory,
) -> Option<Arc<Asset<Arc<str>, Arc<[u8]>>>> {
  use rand::seq::SliceRandom;
  match store.pull(&id).await {
    Ok(asset) => return Some(Arc::new(asset.convert(IntoSharedState))),
    Err(LoadError::Unknown) => (),
    Err(e) => {
      eprintln!("Failed to load asset {}: {}", &id, e);
      return None;
    }
  }
  for _ in 0..4 {
    if search_peers.load(Ordering::Relaxed) {
      let mut peers = directory.peers().await.ok()?.await.ok()?;
      let mut waiting = FuturesUnordered::new();
      peers.shuffle(&mut thread_rng());

      while !peers.is_empty() && !waiting.is_empty() {
        if let Some(peer) = peers.pop() {
          waiting.push(directory.pull_asset_remote(SharedRef::Shared(peer), SharedRef::Shared(id.clone())).await.ok()?);
        }
        let sleep = sleep(Duration::from_secs(15));
        tokio::pin!(sleep);

        let asset = tokio::select! {biased;
          Some(Ok(result)) = waiting.next() => result,
          _ = &mut sleep => continue
        };
        if &asset.principal_hash() == id.as_ref()
          && asset.deserialize_inner::<AllSupportedAssets<String>>().map_err(|_| ()).and_then(|a| a.validate().map_err(|_| ())).is_ok()
        {
          store.push(&id, &asset.reference(ForPacket)).await;
          return Some(Arc::new(asset.convert(IntoSharedState)));
        }
      }
    }
    sleep(Duration::from_secs(120)).await;
  }
  None
}
impl Communication for AnyPlayers {
  type Parameter = bool;

  fn update(&mut self, parameter: Self::Parameter) {
    if parameter {
      self.0.store(true, Ordering::Relaxed);
    }
  }
}
impl<Store: AssetStore + 'static> Launcher<Arc<str>, WaitingAsset> for FindAsset<'_, Store> {
  fn launch(self, id: Arc<str>) -> WaitingAsset {
    let search_peers = Arc::new(AtomicBool::new(self.2));
    Waiting::Pending(pull(id, search_peers.clone(), self.0.clone(), self.1.clone()).boxed(), AnyPlayers(search_peers), Vec::new())
  }
}

async fn pull_realm(id: Arc<str>, directory: Directory) -> Option<RealmTemplate> {
  let realm = match directory.pull_asset(id.clone(), true).await {
    Ok(realm) => match realm.await {
      Ok(realm) => realm,
      Err(_) => {
        return Some(RealmTemplate::NotFound(id));
      }
    },
    Err(()) => {
      return Some(RealmTemplate::NotFound(id));
    }
  };
  let children = match stream::iter(realm.children.iter().cloned().map(Ok))
    .and_then(|id| {
      let directory = directory.clone();
      async move {
        match directory.pull_asset(id.clone(), true).await {
          Ok(rx) => match rx.await {
            Ok(asset) => Ok((asset.principal_hash(), asset)),
            Err(_) => Err(id),
          },
          Err(()) => Err(id),
        }
      }
    })
    .try_collect::<BTreeMap<String, Arc<Asset<Arc<str>, Arc<[u8]>>>>>()
    .await
  {
    Ok(v) => v,
    Err(missing) => return Some(RealmTemplate::NotFound(Arc::from(missing))),
  };
  let Ok(realm) = realm.deserialize_inner::<AllSupportedAssets<Arc<str>>>() else { return Some(RealmTemplate::Invalid) };
  Some(match realm.create_realm_template(&children) {
    Ok(template) => RealmTemplate::Found(template),
    Err(Some(capabilities)) => RealmTemplate::MissingCapabilities(capabilities),
    Err(None) => RealmTemplate::Invalid,
  })
}

impl Launcher<Arc<str>, WaitingRealm> for FindRealm<'_> {
  fn launch(self, id: Arc<str>) -> WaitingRealm {
    Waiting::Pending(pull_realm(id, self.0.clone()).boxed(), (), Vec::new())
  }
}
impl TrackableValue for RealmTemplate {
  fn is_locked(&self) -> bool {
    let RealmTemplate::Found(t) = self else {
      return false;
    };
    t.is_locked()
  }

  fn weight(&self) -> usize {
    let RealmTemplate::Found(t) = self else {
      return 0;
    };
    t.weight()
  }
}
