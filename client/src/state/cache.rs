#[derive(Clone)]
pub struct Cache<C: Cacheable>(super::Shared<(Option<chrono::DateTime<chrono::Utc>>, C)>);
pub struct CacheRef<'a, C: Cacheable, S> {
  cache: &'a Cache<C>,
  client: &'a super::ServerConnection<S>,
}
pub trait Cacheable: Clone + Default {
  const OPERATION: super::InflightOperation;
  const REQUEST: spadina_core::ClientRequest<String>;
  fn stale() -> chrono::Duration;
}
pub trait Export<C: Cacheable> {
  type Error;
  type Output;
  fn export(self, value: &C) -> Result<Self::Output, Self::Error>;
}
pub trait Update<C: Cacheable> {
  fn into_request(self, id: i32, entry: &mut C, local_server: &str) -> spadina_core::ClientRequest<String>;
}
#[derive(Clone)]
pub struct KeyCache<O, V: KeyCacheable<O>>(super::Shared<std::collections::HashMap<V::Key, (chrono::DateTime<chrono::Utc>, V)>>);
pub struct KeyCacheRef<'a, R: Into<spadina_core::ClientRequest<String>>, V: KeyCacheable<R>, S> {
  cache: &'a KeyCache<R, V>,
  client: &'a super::ServerConnection<S>,
}
pub trait KeyCacheable<Request>: Default {
  type Key: Clone + std::hash::Hash + Eq;
  fn into_operation(key: &Self::Key) -> super::InflightOperation;
  fn into_request(key: Self::Key) -> Request;
  fn stale() -> chrono::Duration;
}
pub trait KeyUpdate<O, V: KeyCacheable<O>> {
  fn into_operation(self, key: V::Key, id: i32, entry: std::collections::hash_map::Entry<V::Key, (chrono::DateTime<chrono::Utc>, V)>) -> Option<O>;
}
impl<C: Cacheable> Cache<C> {
  pub(crate) fn capture<'a, S>(&'a self, client: &'a super::ServerConnection<S>) -> CacheRef<'a, C, S> {
    CacheRef { cache: self, client }
  }

  pub(crate) fn update(&self, updater: impl FnOnce(&mut C)) {
    let mut lock = self.0.lock().unwrap();
    lock.0 = Some(chrono::Utc::now());
    updater(&mut lock.1);
  }
}
impl<C: Cacheable> Default for Cache<C> {
  fn default() -> Self {
    Self(Default::default())
  }
}
impl<'a, C: Cacheable, S> CacheRef<'a, C, S> {
  pub fn export<E: Export<C>>(&self, export: E) -> Result<E::Output, E::Error> {
    let lock = self.cache.0.lock().unwrap();
    export.export(&lock.1)
  }
  pub fn get<R>(&self, process: impl FnOnce(Option<&chrono::DateTime<chrono::Utc>>, &C) -> R) -> R {
    let lock = self.cache.0.lock().unwrap();
    let result = process(lock.0.as_ref(), &lock.1);

    if lock.0.as_ref().map(|t| chrono::Utc::now() - *t > C::stale()).unwrap_or(true) {
      self.client.outbound_tx.send(crate::state::connection::ServerRequest::Deliver(C::REQUEST.clone())).unwrap();
    }
    result
  }
  pub fn force_update(&self) {
    self.client.outbound_tx.send(crate::state::connection::ServerRequest::Deliver(C::REQUEST.clone())).unwrap();
  }
  pub fn update(&self, value: impl Update<C>) {
    let id = self.client.cache_state.add_operation(C::OPERATION.clone());
    let mut lock = self.cache.0.lock().unwrap();
    let lock_server = self.client.server.lock().unwrap();
    let message = value.into_request(id, &mut lock.1, &*lock_server);
    self.client.outbound_tx.send(super::connection::ServerRequest::Deliver(message)).unwrap();
  }
}
impl<R: Into<spadina_core::ClientRequest<String>>, V: KeyCacheable<R>> KeyCache<R, V> {
  pub(crate) fn capture<'a, S>(&'a self, client: &'a super::ServerConnection<S>) -> KeyCacheRef<'a, R, V, S> {
    KeyCacheRef { cache: self, client }
  }

  pub fn update(&self, key: V::Key, updater: impl FnOnce(&mut V)) {
    let mut lock = self.0.lock().unwrap();
    let (time, value) = lock.entry(key).or_default();
    *time = chrono::Utc::now();
    updater(value);
  }
}
impl<O, K: KeyCacheable<O>> Default for KeyCache<O, K> {
  fn default() -> Self {
    Self(Default::default())
  }
}
impl<'a, R: Into<spadina_core::ClientRequest<String>>, V: KeyCacheable<R>, S> KeyCacheRef<'a, R, V, S> {
  pub fn get<T>(&self, key: V::Key, process: impl FnOnce(Option<&chrono::DateTime<chrono::Utc>>, &V) -> T) -> T {
    let (result, update) = match self.cache.0.lock().unwrap().entry(key.clone()) {
      std::collections::hash_map::Entry::Occupied(o) => {
        let (time, value) = o.get();
        (process(Some(time), value), chrono::Utc::now() - *time > V::stale())
      }
      std::collections::hash_map::Entry::Vacant(v) => {
        let (_, value) = v.insert((chrono::Utc::now(), Default::default()));
        (process(None, value), true)
      }
    };
    if update {
      self.client.outbound_tx.send(crate::state::connection::ServerRequest::Deliver(V::into_request(key).into())).unwrap();
    }
    result
  }
  pub fn force_update(&self, key: V::Key) {
    self.client.outbound_tx.send(crate::state::connection::ServerRequest::Deliver(V::into_request(key).into())).unwrap();
  }
  pub fn update(&self, key: V::Key, value: impl KeyUpdate<R, V>) {
    let id = self.client.cache_state.add_operation(V::into_operation(&key));
    if let Some(message) = value.into_operation(key.clone(), id, self.cache.0.lock().unwrap().entry(key)) {
      self.client.outbound_tx.send(super::connection::ServerRequest::Deliver(message.into())).unwrap();
    }
  }
}
impl Cacheable for (Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess) {
  const OPERATION: super::InflightOperation = super::InflightOperation::AccessChangeLocation;

  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::AccessGetLocation;

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(15)
  }
}
impl Cacheable for Vec<spadina_core::communication::Announcement<String>> {
  const OPERATION: super::InflightOperation = super::InflightOperation::Announcements;
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::AnnouncementList;

  fn stale() -> chrono::Duration {
    chrono::Duration::max_value()
  }
}
impl Cacheable for spadina_core::avatar::Avatar {
  const OPERATION: super::InflightOperation = super::InflightOperation::Avatar;
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::AvatarGet;

  fn stale() -> chrono::Duration {
    chrono::Duration::max_value()
  }
}
impl Cacheable for std::collections::HashSet<spadina_core::communication::Bookmark<String>> {
  const OPERATION: super::InflightOperation = super::InflightOperation::BookmarkList;
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::BookmarksList;
  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(15)
  }
}

impl Cacheable for super::DirectMessageStats {
  const OPERATION: super::InflightOperation = super::InflightOperation::DirectMessageStats;

  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::DirectMessageStats;

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(30)
  }
}
impl Cacheable for std::collections::HashSet<spadina_core::auth::PublicKey<String>> {
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::PublicKeyList;
  const OPERATION: super::InflightOperation = super::InflightOperation::PublicKey;

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(30)
  }
}
impl Cacheable for Vec<spadina_core::realm::RealmDirectoryEntry<String>> {
  const OPERATION: super::InflightOperation = super::InflightOperation::RealmCalendarList;

  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::CalendarRealmList;

  fn stale() -> chrono::Duration {
    chrono::Duration::max_value()
  }
}

impl Cacheable for std::collections::BTreeSet<super::RemoteServer> {
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::Peers;
  const OPERATION: super::InflightOperation = super::InflightOperation::RemoteServer;

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(30)
  }
}
impl Cacheable for std::collections::HashSet<spadina_core::access::BannedPeer<String>> {
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::PeerBanList;
  const OPERATION: super::InflightOperation = super::InflightOperation::RemoteServer;

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(30)
  }
}
impl Cacheable for Vec<u8> {
  const OPERATION: super::InflightOperation = super::InflightOperation::Calendar;
  const REQUEST: spadina_core::ClientRequest<String> = spadina_core::ClientRequest::CalendarIdentifier;

  fn stale() -> chrono::Duration {
    chrono::Duration::max_value()
  }
}

impl KeyCacheable<spadina_core::ClientRequest<String>>
  for (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess)
{
  type Key = spadina_core::access::AccessTarget;

  fn into_request(target: Self::Key) -> spadina_core::ClientRequest<String> {
    spadina_core::ClientRequest::AccessGet { target }
  }

  fn into_operation(target: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::AccessChange(target.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(15)
  }
}
impl KeyCacheable<spadina_core::realm::RealmRequest<String>>
  for (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess)
{
  type Key = spadina_core::realm::RealmAccessTarget;

  fn into_request(target: Self::Key) -> spadina_core::realm::RealmRequest<String> {
    spadina_core::realm::RealmRequest::AccessGet { target }
  }

  fn into_operation(target: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::RealmAccessChange(target.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(1)
  }
}
impl KeyCacheable<spadina_core::ClientRequest<String>> for spadina_core::access::AccountLockState {
  type Key = String;

  fn into_request(name: Self::Key) -> spadina_core::ClientRequest<String> {
    spadina_core::ClientRequest::AccountLockStatus { name }
  }

  fn into_operation(name: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::AccountLock(name.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(15)
  }
}
impl KeyCacheable<spadina_core::ClientRequest<String>> for Vec<spadina_core::realm::RealmDirectoryEntry<String>> {
  type Key = spadina_core::realm::RealmSource<String>;

  fn into_request(source: Self::Key) -> spadina_core::ClientRequest<String> {
    spadina_core::ClientRequest::RealmsList { source }
  }

  fn into_operation(source: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::RealmList(source.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(5)
  }
}
impl KeyCacheable<spadina_core::ClientRequest<String>> for Vec<super::DirectMessage> {
  type Key = spadina_core::player::PlayerIdentifier<String>;

  fn into_request(player: Self::Key) -> spadina_core::ClientRequest<String> {
    let now = chrono::Utc::now();
    spadina_core::ClientRequest::DirectMessageGet { player, from: now - chrono::Duration::minutes(15), to: now }
  }

  fn into_operation(player: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::DirectMessageSend(player.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(1)
  }
}
impl KeyCacheable<spadina_core::ClientRequest<String>> for spadina_core::player::PlayerLocationState<String> {
  type Key = spadina_core::player::PlayerIdentifier<String>;

  fn into_request(player: Self::Key) -> spadina_core::ClientRequest<String> {
    spadina_core::ClientRequest::PlayerCheck { player }
  }

  fn into_operation(player: &Self::Key) -> super::InflightOperation {
    super::InflightOperation::PlayerLocation(player.clone())
  }

  fn stale() -> chrono::Duration {
    chrono::Duration::minutes(1)
  }
}

pub struct Add<T>(pub T);
pub struct Clear;
pub struct Remove<T>(pub T);
pub struct Set<T>(pub T);

impl Update<Vec<spadina_core::communication::Announcement<String>>> for Add<spadina_core::communication::Announcement<String>> {
  fn into_request(self, id: i32, entry: &mut Vec<spadina_core::communication::Announcement<String>>, _: &str) -> spadina_core::ClientRequest<String> {
    entry.push(self.0.clone());
    spadina_core::ClientRequest::AnnouncementAdd { id, announcement: self.0 }
  }
}
impl Update<Vec<spadina_core::communication::Announcement<String>>> for Clear {
  fn into_request(self, id: i32, entry: &mut Vec<spadina_core::communication::Announcement<String>>, _: &str) -> spadina_core::ClientRequest<String> {
    entry.clear();
    spadina_core::ClientRequest::AnnouncementClear { id }
  }
}
impl Update<spadina_core::avatar::Avatar> for Set<spadina_core::avatar::Avatar> {
  fn into_request(self, id: i32, entry: &mut spadina_core::avatar::Avatar, _: &str) -> spadina_core::ClientRequest<String> {
    *entry = self.0.clone();
    spadina_core::ClientRequest::AvatarSet { id, avatar: self.0 }
  }
}
impl Update<std::collections::HashSet<spadina_core::auth::PublicKey<String>>> for Clear {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::auth::PublicKey<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.clear();
    spadina_core::ClientRequest::PublicKeyDeleteAll { id }
  }
}
impl Update<std::collections::HashSet<spadina_core::auth::PublicKey<String>>> for Remove<String> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::auth::PublicKey<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.retain(|p| p.fingerprint == self.0);
    spadina_core::ClientRequest::PublicKeyDelete { id, name: self.0 }
  }
}
impl Update<std::collections::HashSet<spadina_core::auth::PublicKey<String>>> for Remove<spadina_core::auth::PublicKey<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::auth::PublicKey<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.retain(|p| p.fingerprint == self.0.fingerprint);
    spadina_core::ClientRequest::PublicKeyDelete { id, name: self.0.fingerprint }
  }
}
impl Update<std::collections::HashSet<spadina_core::auth::PublicKey<String>>> for Add<Vec<u8>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::auth::PublicKey<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.insert(spadina_core::auth::PublicKey {
      created: chrono::Utc::now(),
      last_used: None,
      fingerprint: spadina_core::auth::compute_fingerprint(&self.0),
    });
    spadina_core::ClientRequest::PublicKeyAdd { id, der: self.0 }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<String> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.insert(spadina_core::access::BannedPeer::Peer(self.0.clone()));
    spadina_core::ClientRequest::PeerBanSet { id, bans: vec![spadina_core::access::BannedPeer::Peer(self.0)] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<super::RemoteServer> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.insert(spadina_core::access::BannedPeer::Peer(self.0 .0.clone()));
    spadina_core::ClientRequest::PeerBanSet { id, bans: vec![spadina_core::access::BannedPeer::Peer(self.0 .0)] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<spadina_core::access::BannedPeer<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.insert(self.0.clone());
    spadina_core::ClientRequest::PeerBanSet { id, bans: vec![self.0] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<Vec<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.extend(self.0.iter().map(|s| spadina_core::access::BannedPeer::Peer(s.clone())));
    spadina_core::ClientRequest::PeerBanSet { id, bans: self.0.into_iter().map(|s| spadina_core::access::BannedPeer::Peer(s)).collect() }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<Vec<super::RemoteServer>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    let result =
      spadina_core::ClientRequest::PeerBanSet { id, bans: self.0.iter().map(|s| spadina_core::access::BannedPeer::Peer(s.0.clone())).collect() };
    entry.extend(self.0.into_iter().map(|s| spadina_core::access::BannedPeer::Peer(s.0)));
    result
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Add<Vec<spadina_core::access::BannedPeer<String>>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    let result = spadina_core::ClientRequest::PeerBanSet { id, bans: self.0.iter().map(|s| s.clone()).collect() };
    entry.extend(self.0);
    result
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<String> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.remove(&spadina_core::access::BannedPeer::Peer(self.0.clone()));
    spadina_core::ClientRequest::PeerBanClear { id, bans: vec![spadina_core::access::BannedPeer::Peer(self.0)] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<super::RemoteServer> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.remove(&spadina_core::access::BannedPeer::Peer(self.0 .0.clone()));
    spadina_core::ClientRequest::PeerBanClear { id, bans: vec![spadina_core::access::BannedPeer::Peer(self.0 .0)] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<spadina_core::access::BannedPeer<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.remove(&self.0);
    spadina_core::ClientRequest::PeerBanClear { id, bans: vec![self.0] }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<Vec<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    for ban in &self.0 {
      entry.remove(&spadina_core::access::BannedPeer::Peer(ban.clone()));
    }
    spadina_core::ClientRequest::PeerBanClear { id, bans: self.0.into_iter().map(|s| spadina_core::access::BannedPeer::Peer(s)).collect() }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<Vec<super::RemoteServer>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    for ban in &self.0 {
      entry.remove(&spadina_core::access::BannedPeer::Peer(ban.0.clone()));
    }
    spadina_core::ClientRequest::PeerBanClear { id, bans: self.0.into_iter().map(|s| spadina_core::access::BannedPeer::Peer(s.0)).collect() }
  }
}
impl Update<std::collections::HashSet<spadina_core::access::BannedPeer<String>>> for Remove<Vec<spadina_core::access::BannedPeer<String>>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::access::BannedPeer<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    for ban in &self.0 {
      entry.remove(ban);
    }
    spadina_core::ClientRequest::PeerBanClear { id, bans: self.0 }
  }
}
impl Update<Vec<spadina_core::realm::RealmDirectoryEntry<String>>> for Add<spadina_core::realm::LocalRealmTarget<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut Vec<spadina_core::realm::RealmDirectoryEntry<String>>,
    local_server: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.push(spadina_core::realm::RealmDirectoryEntry {
      asset: self.0.asset.clone(),
      owner: self.0.owner.clone(),
      server: local_server.to_string(),
      name: "Unknown".to_string(),
      activity: spadina_core::realm::RealmActivity::Unknown,
      train: None,
    });
    spadina_core::ClientRequest::CalendarRealmAdd { id, realm: self.0 }
  }
}
impl Update<Vec<spadina_core::realm::RealmDirectoryEntry<String>>> for Clear {
  fn into_request(self, id: i32, entry: &mut Vec<spadina_core::realm::RealmDirectoryEntry<String>>, _: &str) -> spadina_core::ClientRequest<String> {
    entry.clear();
    spadina_core::ClientRequest::CalendarRealmClear { id }
  }
}
impl Update<Vec<spadina_core::realm::RealmDirectoryEntry<String>>> for Remove<spadina_core::realm::LocalRealmTarget<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut Vec<spadina_core::realm::RealmDirectoryEntry<String>>,
    local_server: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.retain_mut(|r| r.asset != self.0.asset && r.owner != self.0.owner && &r.server != local_server);
    spadina_core::ClientRequest::CalendarRealmRemove { id, realm: self.0 }
  }
}
impl Update<Vec<u8>> for () {
  fn into_request(self, id: i32, entry: &mut Vec<u8>, _: &str) -> spadina_core::ClientRequest<String> {
    entry.clear();
    spadina_core::ClientRequest::CalendarReset { id, player: None }
  }
}
impl Update<(Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess)>
  for Add<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>
{
  fn into_request(
    self,
    id: i32,
    entry: &mut (Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess),
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.0.insert(0, self.0);
    spadina_core::ClientRequest::AccessLocationSet { id, rules: entry.0.clone(), default: entry.1.clone() }
  }
}
impl Update<(Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess)>
  for Set<(Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess)>
{
  fn into_request(
    self,
    id: i32,
    entry: &mut (Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess),
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    *entry = self.0;
    spadina_core::ClientRequest::AccessLocationSet { id, rules: entry.0.clone(), default: entry.1.clone() }
  }
}
impl Update<(Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess)>
  for Remove<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>
{
  fn into_request(
    self,
    id: i32,
    entry: &mut (Vec<spadina_core::access::AccessControl<spadina_core::access::LocationAccess>>, spadina_core::access::LocationAccess),
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.0.retain(|r| r != &self.0);
    spadina_core::ClientRequest::AccessLocationSet { id, rules: entry.0.clone(), default: entry.1.clone() }
  }
}
impl
  KeyUpdate<
    spadina_core::realm::RealmRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Add<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>
{
  fn into_operation(
    self,
    key: spadina_core::realm::RealmAccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::realm::RealmAccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::realm::RealmRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        acls.insert(0, self.0.clone());
        *time = chrono::Utc::now();
        Some(spadina_core::realm::RealmRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::realm::RealmRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Remove<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>
{
  fn into_operation(
    self,
    key: spadina_core::realm::RealmAccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::realm::RealmAccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::realm::RealmRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        let len = acls.len();
        acls.retain(|a| *a != self.0);
        if len == acls.len() {
          None
        } else {
          *time = chrono::Utc::now();
          Some(spadina_core::realm::RealmRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
        }
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::realm::RealmRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Set<spadina_core::access::SimpleAccess>
{
  fn into_operation(
    self,
    key: spadina_core::realm::RealmAccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::realm::RealmAccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::realm::RealmRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        if *default == self.0 {
          None
        } else {
          *time = chrono::Utc::now();
          Some(spadina_core::realm::RealmRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
        }
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Set<(Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess)>
{
  fn into_operation(
    self,
    key: spadina_core::access::AccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::access::AccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    let (new_acls, new_default) = self.0;
    let (time, (acls, default)) = entry.or_default();
    *time = chrono::Utc::now();
    *acls = new_acls;
    *default = new_default;
    Some(spadina_core::ClientRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
  }
}
impl
  KeyUpdate<
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Add<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>
{
  fn into_operation(
    self,
    key: spadina_core::access::AccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::access::AccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        acls.insert(0, self.0.clone());
        *time = chrono::Utc::now();
        Some(spadina_core::ClientRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Remove<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>
{
  fn into_operation(
    self,
    key: spadina_core::access::AccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::access::AccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        let len = acls.len();
        acls.retain(|a| *a != self.0);
        if len == acls.len() {
          None
        } else {
          *time = chrono::Utc::now();
          Some(spadina_core::ClientRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
        }
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::ClientRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Set<spadina_core::access::SimpleAccess>
{
  fn into_operation(
    self,
    key: spadina_core::access::AccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::access::AccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    // If we haven't fetched the ACLs, we don't want to destructively overwrite them
    match entry {
      std::collections::hash_map::Entry::Occupied(mut o) => {
        let (time, (acls, default)) = o.get_mut();
        if *default == self.0 {
          None
        } else {
          *time = chrono::Utc::now();
          Some(spadina_core::ClientRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
        }
      }
      std::collections::hash_map::Entry::Vacant(_) => None,
    }
  }
}
impl
  KeyUpdate<
    spadina_core::realm::RealmRequest<String>,
    (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
  > for Set<(Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess)>
{
  fn into_operation(
    self,
    key: spadina_core::realm::RealmAccessTarget,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::realm::RealmAccessTarget,
      (
        chrono::DateTime<chrono::Utc>,
        (Vec<spadina_core::access::AccessControl<spadina_core::access::SimpleAccess>>, spadina_core::access::SimpleAccess),
      ),
    >,
  ) -> Option<spadina_core::realm::RealmRequest<String>> {
    let (new_acls, new_default) = self.0;
    let (time, (acls, default)) = entry.or_default();
    *time = chrono::Utc::now();
    *acls = new_acls;
    *default = new_default;
    Some(spadina_core::realm::RealmRequest::AccessSet { id, target: key, rules: acls.clone(), default: *default })
  }
}
impl KeyUpdate<spadina_core::ClientRequest<String>, spadina_core::access::AccountLockState> for bool {
  fn into_operation(
    self,
    name: <spadina_core::access::AccountLockState as KeyCacheable<spadina_core::ClientRequest<String>>>::Key,
    id: i32,
    entry: std::collections::hash_map::Entry<
      <spadina_core::access::AccountLockState as KeyCacheable<spadina_core::ClientRequest<String>>>::Key,
      (chrono::DateTime<chrono::Utc>, spadina_core::access::AccountLockState),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    *entry.or_default() =
      (chrono::Utc::now(), if self { spadina_core::access::AccountLockState::Locked } else { spadina_core::access::AccountLockState::Unlocked });
    Some(spadina_core::ClientRequest::AccountLockChange { id, name, locked: self })
  }
}
impl Update<std::collections::HashSet<spadina_core::communication::Bookmark<String>>> for Add<spadina_core::communication::Bookmark<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::communication::Bookmark<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.insert(self.0.clone());
    spadina_core::ClientRequest::BookmarkAdd { id, bookmark: self.0 }
  }
}
impl Update<std::collections::HashSet<spadina_core::communication::Bookmark<String>>> for Remove<spadina_core::communication::Bookmark<String>> {
  fn into_request(
    self,
    id: i32,
    entry: &mut std::collections::HashSet<spadina_core::communication::Bookmark<String>>,
    _: &str,
  ) -> spadina_core::ClientRequest<String> {
    entry.remove(&self.0);
    spadina_core::ClientRequest::BookmarkRemove { id, bookmark: self.0 }
  }
}
impl KeyUpdate<spadina_core::ClientRequest<String>, Vec<super::DirectMessage>> for String {
  fn into_operation(
    self,
    key: spadina_core::player::PlayerIdentifier<String>,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::player::PlayerIdentifier<String>,
      (chrono::DateTime<chrono::Utc>, Vec<super::DirectMessage>),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    let (_, messages) = entry.or_default();
    messages.push(super::DirectMessage::Pending(id, spadina_core::communication::MessageBody::Text(self.clone())));
    Some(spadina_core::ClientRequest::DirectMessageSend { id, recipient: key, body: spadina_core::communication::MessageBody::Text(self) })
  }
}
impl KeyUpdate<spadina_core::ClientRequest<String>, Vec<super::DirectMessage>> for spadina_core::communication::MessageBody<String> {
  fn into_operation(
    self,
    key: spadina_core::player::PlayerIdentifier<String>,
    id: i32,
    entry: std::collections::hash_map::Entry<
      spadina_core::player::PlayerIdentifier<String>,
      (chrono::DateTime<chrono::Utc>, Vec<super::DirectMessage>),
    >,
  ) -> Option<spadina_core::ClientRequest<String>> {
    let (_, messages) = entry.or_default();
    messages.push(super::DirectMessage::Pending(id, self.clone()));
    Some(spadina_core::ClientRequest::DirectMessageSend { id, recipient: key, body: self })
  }
}
pub struct ToJsonFile<P: AsRef<std::path::Path>>(pub P);
pub enum ToJsonError {
  Json(serde_json::Error),
  Io(std::io::Error),
}
impl<C: Cacheable + serde::ser::Serialize, P: AsRef<std::path::Path>> Export<C> for ToJsonFile<P> {
  type Error = ToJsonError;
  type Output = ();
  fn export(self, value: &C) -> Result<Self::Output, Self::Error> {
    serde_json::to_writer_pretty(std::fs::File::create(self.0)?, value)?;
    Ok(())
  }
}
impl std::fmt::Display for ToJsonError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ToJsonError::Json(e) => e.fmt(f),
      ToJsonError::Io(e) => e.fmt(f),
    }
  }
}
impl From<serde_json::Error> for ToJsonError {
  fn from(value: serde_json::Error) -> Self {
    ToJsonError::Json(value)
  }
}
impl From<std::io::Error> for ToJsonError {
  fn from(value: std::io::Error) -> Self {
    ToJsonError::Io(value)
  }
}
