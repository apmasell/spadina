use crate::state::ScreenState;

pub(crate) enum Location<S> {
  NoWhere,
  LoadingRealm {
    cache: spadina_core::asset_store::CachingResourceMapper<String>,
    messages: super::Shared<Vec<spadina_core::location::LocationMessage<String>>>,
    name: super::Shared<(String, bool)>,
    owner: String,
    players: std::collections::HashMap<spadina_core::player::PlayerIdentifier<String>, spadina_core::avatar::Avatar>,
    realm_asset: String,
    remaining: std::collections::BTreeSet<String>,
    remaining_counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    seed: i32,
    server: String,
    settings: super::Shared<spadina_core::realm::RealmSettings<String>>,
  },
  Realm {
    access: super::cache::KeyCache<
      spadina_core::realm::RealmRequest<String>,
      (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
    >,
    announcements: super::Shared<(Option<chrono::DateTime<chrono::Utc>>, Vec<spadina_core::realm::RealmAnnouncement<String>>)>,
    asset: String,
    jitter: std::sync::Arc<super::jitter::Jitter<30>>,
    messages: super::Shared<Vec<spadina_core::location::LocationMessage<String>>>,
    name: super::Shared<(String, bool)>,
    owner: String,
    paths: super::Shared<Paths>,
    players: std::collections::HashMap<spadina_core::player::PlayerIdentifier<String>, spadina_core::avatar::Avatar>,
    requests: super::Shared<Vec<super::PlayerRequest>>,
    seed: i32,
    server: String,
    settings: super::Shared<spadina_core::realm::RealmSettings<String>>,
    state: S,
  },
}
pub(crate) enum LocationEvent<S> {
  UpdateScreen(ScreenState<S, std::borrow::Cow<'static, str>>),
  PullAsset(String),
  Leave,
}
pub trait LocationState: std::marker::Unpin + Send + Sync + 'static {
  fn new_host(host: spadina_core::player::PlayerIdentifier<String>) -> Self;
  fn new_realm(host: spadina_core::player::PlayerIdentifier<String>) -> Self;
  fn handle_host(&mut self, response: spadina_core::self_hosted::GuestResponse<String>);
  fn update_realm_states(
    &mut self,
    time: chrono::DateTime<chrono::Utc>,
    player: spadina_core::realm::PlayerStates<String>,
    state: spadina_core::realm::PropertyStates<String>,
  );
  fn update_realm_setting(&mut self, name: &str, value: &spadina_core::realm::RealmSetting<String>);
}
pub(crate) type Paths = std::collections::HashMap<spadina_core::realm::Point, Vec<spadina_core::realm::Point>>;
pub trait WorldRenderer: Send + Sync + 'static {
  fn update(
    &self,
    time: chrono::DateTime<chrono::Utc>,
    player: spadina_core::realm::PlayerStates<String>,
    state: spadina_core::realm::PropertyStates<String>,
  );
  fn update_setting(&self, name: &str, value: &spadina_core::realm::RealmSetting<String>);
}
impl<S> Location<S> {
  pub(crate) fn load_realm(
    owner: String,
    server: String,
    name: String,
    asset: String,
    in_directory: bool,
    seed: i32,
    settings: spadina_core::realm::RealmSettings<String>,
  ) -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let remaining_counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));
    (
      Location::LoadingRealm {
        cache: Default::default(),
        messages: Default::default(),
        name: std::sync::Arc::new(std::sync::Mutex::new((name, in_directory))),
        owner,
        players: Default::default(),
        realm_asset: asset.clone(),
        remaining: std::iter::once(asset).collect(),
        remaining_counter: remaining_counter.clone(),
        seed,
        server,
        settings: std::sync::Arc::new(std::sync::Mutex::new(settings)),
      },
      remaining_counter,
    )
  }
  pub(crate) fn update_avatars(
    &mut self,
    update_players: std::collections::HashMap<spadina_core::player::PlayerIdentifier<String>, spadina_core::avatar::Avatar>,
  ) {
    match self {
      Location::NoWhere => (),
      Location::LoadingRealm { players, .. } => {
        *players = update_players;
      }
      Location::Realm { players, .. } => {
        *players = update_players;
      }
    }
  }
  pub fn handle_realm(&mut self, response: spadina_core::realm::RealmResponse<String>) {
    todo!();
  }
  pub fn handle_host(&mut self, response: spadina_core::self_hosted::GuestResponse<String>) {
    todo!();
  }

  pub(crate) fn handle_host_event(&mut self, event: spadina_core::self_hosted::HostEvent<String>) {
    todo!()
  }

  pub(crate) fn asset_available(&mut self, principal: &str, asset: &spadina_core::asset::Asset) -> Option<Vec<LocationEvent<S>>> {
    let (update, results) = match self {
      Location::NoWhere => (None, None),
      Location::LoadingRealm { cache, messages, name, owner, realm_asset, remaining, remaining_counter, players, seed, server, settings } => {
        match cache.install(principal.to_string(), asset.clone()) {
          Err(_) => (Some(Location::NoWhere), Some(vec![LocationEvent::Leave])),
          Ok(()) => {
            if remaining.remove(principal) {
              remaining_counter.store(remaining.len(), std::sync::atomic::Ordering::Relaxed);
              if remaining.is_empty() {
                if realm_asset.as_str() == principal {
                  (None, Some(asset.children.iter().map(|child| LocationEvent::PullAsset(child.to_string())).collect()))
                } else {
                  todo!()
                }
              } else {
                (None, None)
              }
            } else {
              (None, None)
            }
          }
        }
      }
      Location::Realm { .. } => (None, None),
    };
    if let Some(update) = update {
      *self = update;
    }
    results
  }

  pub(crate) fn request_consent(
    &self,
    id: i32,
    player: spadina_core::player::PlayerIdentifier<String>,
    emote: std::sync::Arc<super::emote_cache::Emote>,
  ) {
    todo!()
  }

  pub(crate) fn request_emote(
    &self,
    id: i32,
    emote: std::sync::Arc<super::emote_cache::Emote>,
    player: spadina_core::player::PlayerIdentifier<String>,
  ) {
    todo!()
  }

  pub(crate) fn new_guest(host: spadina_core::player::PlayerIdentifier<String>) -> (Self, S) {
    todo!()
  }

  pub(crate) fn messages(
    &self,
    messages: Vec<spadina_core::location::LocationMessage<String>>,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) {
    todo!()
  }

  pub(crate) fn message_posted(
    &self,
    sender: spadina_core::player::PlayerIdentifier<String>,
    body: spadina_core::communication::MessageBody<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
  ) {
    todo!()
  }

  pub(crate) fn request_follow(&self, id: i32, player: spadina_core::player::PlayerIdentifier<String>) {
    todo!()
  }

  pub(crate) fn new_host() -> (Location<S>, S) {
    todo!()
  }
}

impl<S: std::marker::Unpin> futures::Stream for Location<S> {
  type Item = Vec<LocationEvent<S>>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    match self.get_mut() {
      Location::NoWhere => std::task::Poll::Pending,
      Location::LoadingRealm { .. } => std::task::Poll::Pending,
      Location::Realm { .. } => std::task::Poll::Pending,
    }
  }
}
