use futures::Stream;
pub mod activity;
pub mod asset_manager;
pub mod manager;
pub mod self_hosted;

#[async_trait::async_trait]
pub(crate) trait Destination: Send + Sync + Sized + Stream + Unpin + 'static
where
  Self::Item: Send + Sync + 'static,
{
  type Identifier: std::fmt::Display + Clone + Send + Sync + Owner + 'static;
  type Request: Send + 'static;
  type Response: Clone + Sync + Send + 'static;
  fn capabilities(&self) -> &std::collections::BTreeSet<&'static str>;

  async fn consensual_emote(
    &mut self,
    requester_key: &crate::realm::puzzle::PlayerKey,
    requester: &SharedPlayerId,
    target_key: &crate::realm::puzzle::PlayerKey,
    target: &SharedPlayerId,
    emote: std::sync::Arc<str>,
  ) -> Vec<DestinationControl<Self::Response>>;
  fn delete(&mut self, requester: Option<SharedPlayerId>) -> spadina_core::UpdateResult;
  async fn follow(
    &mut self,
    requester_key: &crate::realm::puzzle::PlayerKey,
    requester: &SharedPlayerId,
    target_key: &crate::realm::puzzle::PlayerKey,
    target: &SharedPlayerId,
  ) -> Vec<DestinationControl<Self::Response>>;
  fn get_messages(
    &self,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) -> Vec<spadina_core::location::LocationMessage<String>>;
  async fn handle(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &SharedPlayerId,
    is_superuser: bool,
    request: Self::Request,
  ) -> Vec<DestinationControl<Self::Response>>;
  async fn process_events(&mut self, events: <Self as Stream>::Item) -> Vec<DestinationControl<Self::Response>>;
  fn quit(&mut self);
  async fn remove_player(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
  ) -> Vec<DestinationControl<Self::Response>>;
  async fn send_message(
    &mut self,
    key: Option<&crate::realm::puzzle::PlayerKey>,
    player: &SharedPlayerId,
    body: &spadina_core::communication::MessageBody<
      impl AsRef<str> + serde::Serialize + std::fmt::Debug + std::cmp::PartialEq + std::cmp::Eq + Sync + Into<std::sync::Arc<str>>,
    >,
  ) -> Option<chrono::DateTime<chrono::Utc>>;
  async fn try_add(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
    is_superuser: bool,
  ) -> Result<(spadina_core::location::LocationResponse<crate::shstr::ShStr>, Vec<DestinationControl<Self::Response>>), ()>;
}
pub(crate) trait Owner {
  fn owner(&self) -> &std::sync::Arc<str>;
}
pub(crate) enum DestinationControl<Message> {
  Broadcast(Message),
  Move(crate::realm::puzzle::PlayerKey, Option<spadina_core::realm::RealmTarget<std::sync::Arc<str>>>),
  MoveTrain(crate::realm::puzzle::PlayerKey, std::sync::Arc<str>, u16),
  Quit,
  Response(crate::realm::puzzle::PlayerKey, Message),
  SendMessage(spadina_core::location::LocationMessage<std::sync::Arc<str>>),
}

pub(crate) enum DestinationRequest<Request> {
  ConsensualEmoteRequest { emote: crate::shstr::ShStr, player: spadina_core::player::PlayerIdentifier<crate::shstr::ShStr> },
  ConsensualEmoteResponse { id: i32, ok: bool },
  FollowResponse(i32, bool),
  FollowRequest(spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>),
  Messages { from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc> },
  Request(Request),
  SendMessage(spadina_core::communication::MessageBody<crate::shstr::ShStr>),
}
pub(crate) enum DestinationResponse<Message> {
  ConsensualEmoteRequest(SharedPlayerId, i32, std::sync::Arc<str>),
  FollowRequest(SharedPlayerId, i32),
  Location(spadina_core::location::LocationResponse<crate::shstr::ShStr>),
  MessagePosted(spadina_core::location::LocationMessage<std::sync::Arc<str>>),
  Messages { from: chrono::DateTime<chrono::Utc>, to: chrono::DateTime<chrono::Utc>, messages: Vec<spadina_core::location::LocationMessage<String>> },
  Move(Option<spadina_core::realm::RealmTarget<std::sync::Arc<str>>>),
  MoveTrain(std::sync::Arc<str>, u16),
  Response(Message),
}

pub(crate) struct Directory {
  asset_manager: asset_manager::AssetManager,
  authnz: std::sync::Arc<crate::access::AuthNZ>,
  database: std::sync::Arc<crate::database::Database>,
  pub hosting: dashmap::DashMap<std::sync::Arc<str>, manager::DestinationManager<self_hosted::SelfHosted>>,
  peers: dashmap::DashMap<std::sync::Arc<str>, crate::peer::Peer>,
  pub players: dashmap::DashMap<std::sync::Arc<str>, crate::client::Client>,
  realms: tokio::sync::mpsc::Sender<LaunchRequest>,
}

pub(crate) enum LaunchRequest {
  CheckActivity(spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>, tokio::sync::oneshot::Sender<spadina_core::realm::RealmActivity>),
  ClearCache,
  Delete(
    spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>,
    Option<SharedPlayerId>,
    tokio::sync::oneshot::Sender<spadina_core::UpdateResult>,
  ),
  Move(PlayerHandle<crate::realm::Realm>, LaunchTarget),
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum LaunchTarget {
  ByAsset { owner: std::sync::Arc<str>, asset: std::sync::Arc<str> },
  ByTrain { owner: std::sync::Arc<str>, train: u16 },
}

pub(crate) struct PlayerHandle<D: Destination>
where
  D::Item: Send + Sync + 'static,
{
  pub avatar: tokio::sync::watch::Receiver<spadina_core::avatar::Avatar>,
  pub capabilities: std::sync::Arc<std::collections::BTreeSet<&'static str>>,
  pub is_superuser: bool,
  pub principal: SharedPlayerId,
  pub tx: tokio::sync::mpsc::Sender<DestinationResponse<D::Response>>,
  pub rx: tokio::sync::mpsc::Receiver<DestinationRequest<D::Request>>,
}
#[derive(Debug, Hash, Eq, PartialEq)]
pub enum RealmLaunch {
  Existing { db_id: i32, owner: std::sync::Arc<str>, asset: std::sync::Arc<str> },
  New { owner: std::sync::Arc<str>, asset: std::sync::Arc<str>, train: Option<u16> },
}

pub type SharedPlayerId = spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>;
impl Directory {
  pub async fn new(
    database: std::sync::Arc<crate::database::Database>,
    authnz: std::sync::Arc<crate::access::AuthNZ>,
    asset_store: std::sync::Arc<dyn spadina_core::asset_store::AsyncAssetStore>,
  ) -> std::sync::Arc<Self> {
    let (realms, mut receiver) = tokio::sync::mpsc::channel::<LaunchRequest>(100);
    let server_name = authnz.server_name.clone();
    let result = std::sync::Arc::new(Directory {
      asset_manager: asset_manager::AssetManager::new(asset_store),
      authnz,
      database: database.clone(),
      hosting: Default::default(),
      peers: Default::default(),
      players: Default::default(),
      realms,
    });
    let directory = std::sync::Arc::downgrade(&result);
    result.asset_manager.set_directory(directory.clone()).await;
    tokio::spawn(async move {
      let mut realms = std::collections::HashMap::<_, manager::DestinationManager<crate::realm::Realm>>::new();
      let mut resolver = std::collections::HashMap::new();

      while let Some(request) = receiver.recv().await {
        if let Some(directory) = directory.upgrade() {
          match request {
            LaunchRequest::CheckActivity(realm, output) => {
              let is_err = output
                .send(match realms.get(&realm) {
                  Some(realm) => realm.activity(),
                  None => spadina_core::realm::RealmActivity::Deserted,
                })
                .is_err();
              if is_err {
                eprintln!("Failed to send activity response for realm {:?}", realm);
              }
            }
            LaunchRequest::ClearCache => resolver.clear(),
            LaunchRequest::Delete(realm, requester, output) => {
              match realms.get(&realm) {
                Some(realm) => realm.delete(requester, output).await,
                None => std::mem::drop(output.send(
                  match database.realm_find(crate::database::realm_scope::RealmScope::NamedAsset { owner: &realm.owner, asset: &realm.asset }) {
                    Ok(None) => spadina_core::UpdateResult::Success,
                    Ok(Some((db_id, _))) => match database.realm_acl_read(db_id, crate::database::schema::realm::dsl::admin_acl) {
                      Ok(acl) => {
                        if requester.map(|requester| acl.check(&requester, &server_name) == spadina_core::access::SimpleAccess::Allow).unwrap_or(true)
                        {
                          match database.realm_delete(db_id) {
                            Ok(()) => spadina_core::UpdateResult::Success,
                            Err(e) => {
                              eprintln!("Failed to delete realm {}: {}", &realm, e);
                              spadina_core::UpdateResult::InternalError
                            }
                          }
                        } else {
                          spadina_core::UpdateResult::NotAllowed
                        }
                      }
                      Err(e) => {
                        eprintln!("Failed to load realm {} for deletion: {}", &realm, e);
                        spadina_core::UpdateResult::InternalError
                      }
                    },
                    Err(e) => {
                      eprintln!("Failed to find realm {} for deletion: {}", &realm, e);
                      spadina_core::UpdateResult::InternalError
                    }
                  },
                )),
              }
              if let Some(realm) = realms.remove(&realm) {
                resolver.clear();
                realm.kill();
              }
            }
            LaunchRequest::Move(player, target) => {
              let failed_add = match resolver.entry(target.clone()) {
                std::collections::hash_map::Entry::Vacant(v) => {
                  let launch = match v.key() {
                    LaunchTarget::ByAsset { owner, asset } => {
                      match database.realm_find(crate::database::realm_scope::RealmScope::NamedAsset { owner: &owner, asset: &asset }) {
                        Ok(Some((db_id, _))) => Some(RealmLaunch::Existing { db_id, owner: owner.clone(), asset: asset.clone() }),
                        Ok(None) => Some(RealmLaunch::New { owner: owner.clone(), asset: asset.clone(), train: None }),
                        Err(e) => {
                          eprintln!("Database error trying to find realm {} for {}: {}", owner, asset, e);
                          None
                        }
                      }
                    }
                    LaunchTarget::ByTrain { owner, train } => {
                      match database.realm_find(crate::database::realm_scope::RealmScope::NamedTrain { owner: &owner, train: *train }) {
                        Ok(Some((db_id, asset))) => Some(RealmLaunch::Existing { db_id, owner: owner.clone(), asset: std::sync::Arc::from(asset) }),
                        Ok(None) => match database.realm_next_train_asset(&owner, *train) {
                          Ok(Some(asset)) => Some(RealmLaunch::New { owner: owner.clone(), asset: std::sync::Arc::from(asset), train: Some(*train) }),
                          Ok(None) => None,
                          Err(e) => {
                            eprintln!("Database error trying to find next train realm {} for {}: {}", train, owner, e);
                            None
                          }
                        },
                        Err(e) => {
                          eprintln!("Database error trying to find next train realm {} for {}: {}", train, owner, e);
                          None
                        }
                      }
                    }
                  };
                  match launch {
                    None => {
                      if let Err(_) = player.tx.send(DestinationResponse::Location(spadina_core::location::LocationResponse::ResolutionFailed)).await
                      {
                        eprintln!("Failed to resolve realm for {}", &player.principal);
                      }
                      false
                    }
                    Some(launch) => {
                      let realm = match &launch {
                        RealmLaunch::Existing { owner, asset, .. } => {
                          spadina_core::realm::LocalRealmTarget { owner: owner.clone(), asset: asset.clone() }
                        }
                        RealmLaunch::New { owner, asset, .. } => spadina_core::realm::LocalRealmTarget { owner: owner.clone(), asset: asset.clone() },
                      };
                      let failed_to_add = realms
                        .entry(realm.clone())
                        .or_insert_with(|| crate::realm::Realm::new(database.clone(), directory.clone(), server_name.clone(), launch))
                        .add(player)
                        .await;

                      if failed_to_add {
                        realms.remove(&realm);
                        true
                      } else {
                        v.insert(realm.clone());
                        false
                      }
                    }
                  }
                }
                std::collections::hash_map::Entry::Occupied(o) => {
                  let failed_add = match realms.get(o.get()) {
                    None => {
                      if let Err(_) = player.tx.send(DestinationResponse::Location(spadina_core::location::LocationResponse::ResolutionFailed)).await
                      {
                        eprintln!("Failed to find realm {} for {}", o.get(), &player.principal);
                      }
                      true
                    }
                    Some(realm) => {
                      if realm.add(player).await {
                        true
                      } else {
                        false
                      }
                    }
                  };
                  if failed_add {
                    realms.remove(o.get());
                  }
                  failed_add
                }
              };
              if failed_add {
                resolver.remove(&target);
              }
            }
          }
        }
      }
    });

    result
  }
  pub fn asset_manager(&self) -> &asset_manager::AssetManager {
    &self.asset_manager
  }
  pub fn clone_auth_and_db(&self) -> (std::sync::Arc<crate::access::AuthNZ>, std::sync::Arc<crate::database::Database>) {
    (self.authnz.clone(), self.database.clone())
  }
  pub async fn launch(&self, request: LaunchRequest) -> Result<(), ()> {
    match self.realms.send(request).await {
      Err(_) => {
        eprintln!("Failed to send launch request");
        Err(())
      }
      Ok(()) => Ok(()),
    }
  }
  pub async fn peer<'f, F: 'f, R: 'f>(self: &std::sync::Arc<Self>, name: &str, process: F) -> Option<R>
  where
    for<'a> F: FnOnce(&'a crate::peer::Peer) -> futures::future::BoxFuture<'a, R>,
  {
    let name = spadina_core::net::parse_server_name(name)?.into();
    Some(match self.peers.entry(name) {
      dashmap::mapref::entry::Entry::Occupied(o) => {
        let peer = o.get();
        process(peer).await
      }
      dashmap::mapref::entry::Entry::Vacant(v) => {
        let peer_name = v.key().clone();
        let peer = v.insert(crate::peer::Peer::new(peer_name, std::sync::Arc::downgrade(self), self.database.clone(), self.authnz.clone()).await);
        peer.value().initiate_connection().await;
        process(&*peer).await
      }
    })
  }
  pub fn peers(&self) -> Vec<std::sync::Arc<str>> {
    self.peers.iter().map(|r| r.key().clone()).collect()
  }
  pub fn clean_peer(&self, peer_name: &std::sync::Arc<str>) {
    self.peers.remove_if(peer_name, |_, old_peer| old_peer.is_dead());
  }
  pub fn apply_peer_bans(&self, bans: &std::collections::HashSet<spadina_core::access::BannedPeer<String>>) {
    self.peers.retain(|name, peer| {
      if bans.iter().any(|ban| match ban {
        spadina_core::access::BannedPeer::Peer(ban) => name.as_ref() == ban.as_str(),
        spadina_core::access::BannedPeer::Domain(ban) => spadina_core::net::has_domain_suffix(ban.as_str(), &name),
      }) {
        let peer = peer.clone();
        tokio::spawn(async move {
          peer.kill().await;
        });
        false
      } else {
        true
      }
    })
  }

  pub async fn realm_activity(&self, realm: spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>) -> spadina_core::realm::RealmActivity {
    let (tx, rx) = tokio::sync::oneshot::channel();
    match self.realms.send(LaunchRequest::CheckActivity(realm, tx)).await {
      Ok(()) => match rx.await {
        Ok(v) => v,
        Err(_) => spadina_core::realm::RealmActivity::Unknown,
      },
      Err(_) => spadina_core::realm::RealmActivity::Unknown,
    }
  }
}

impl<D: Destination> std::fmt::Debug for PlayerHandle<D>
where
  D::Item: Send + Sync + 'static,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PlayerHandle")
      .field("capabilities", &self.capabilities)
      .field("is_superuser", &self.is_superuser)
      .field("principal", &self.principal)
      .finish()
  }
}
