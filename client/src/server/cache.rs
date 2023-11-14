use crate::server::exports::Export;
use crate::server::peer::Peer;
use crate::server::updates::Update;
use crate::server::EventKind;
use chrono::{Duration, Utc};
use spadina_core::access::{AccessSetting, BannedPeer, OnlineAccess, Privilege, SimpleAccess};
use spadina_core::avatar::Avatar;
use spadina_core::communication::Announcement;
use spadina_core::location::target::LocalTarget;
use spadina_core::net::server::auth::PublicKey;
use spadina_core::net::server::{ClientRequest, DirectMessageStats};
use spadina_core::resource::Resource;
use spadina_core::tracking_map::TrackingMap;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
pub struct Cache<C: Cacheable>(Option<(chrono::DateTime<Utc>, C)>);

pub trait Cacheable: 'static {
  fn refresh() -> ClientRequest<String, Vec<u8>>;
  fn stale() -> Duration;
}

impl<C: Cacheable> Cache<C> {
  pub fn get<'a, E: Export<C>>(&'a self, export: E) -> (Option<Message>, E::Output<'a>) {
    (
      if self.0.as_ref().map(|(old, _)| Utc::now() - old > C::stale()).unwrap_or(true) { Some(C::refresh().into()) } else { None },
      export.export(self.0.as_ref().map(|(_, v)| v)),
    )
  }
  pub fn invalidate(&mut self) {
    self.0 = None;
  }
  pub fn modify<U: Update<C>>(&mut self, tracking: &mut TrackingMap<U>, update: U) -> Option<Message> {
    tracking.add(update, |id, update| update.update(id, self.0.as_mut().map(|(_, v)| v)))
  }
  pub fn refresh(&self) -> Message {
    C::refresh().into()
  }
  pub(crate) fn set(&mut self, value: C) {
    self.0 = Some((Utc::now(), value));
  }
}
impl<C: Cacheable + Default> Cache<C> {
  pub(crate) fn update(&mut self) -> &mut C {
    if self.0.is_none() {
      self.0 = Some((chrono::Utc::now(), Default::default()));
    }
    let (_, v) = self.0.as_mut().unwrap();
    v
  }
}
impl<C: Cacheable> Default for Cache<C> {
  fn default() -> Self {
    Self(None)
  }
}
impl Cacheable for AccessSetting<String, OnlineAccess> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::AccessGetOnline
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}
impl Cacheable for AccessSetting<String, Privilege> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::AccessGetDefault
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}
impl Cacheable for AccessSetting<String, SimpleAccess> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::AccessGetDirectMessage
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}
impl Cacheable for Vec<Announcement<String>> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::AnnouncementList
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}
impl Cacheable for Avatar {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::AvatarGet
  }

  fn stale() -> Duration {
    Duration::max_value()
  }
}
impl Cacheable for HashSet<Resource<String>> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::BookmarksList
  }

  fn stale() -> Duration {
    Duration::minutes(15)
  }
}

impl Cacheable for BTreeSet<LocalTarget<String>> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::CalendarLocationList
  }

  fn stale() -> Duration {
    Duration::minutes(15)
  }
}
impl Cacheable for DirectMessageStats<String> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::DirectMessageStats
  }

  fn stale() -> Duration {
    Duration::minutes(10)
  }
}
impl Cacheable for BTreeMap<String, PublicKey> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::PublicKeyList
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}

impl Cacheable for BTreeSet<Peer> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::Peers
  }

  fn stale() -> Duration {
    Duration::minutes(15)
  }
}
impl Cacheable for HashSet<BannedPeer<String>> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::PeerBanList
  }

  fn stale() -> Duration {
    Duration::minutes(30)
  }
}
impl Cacheable for Vec<u8> {
  fn refresh() -> ClientRequest<String, Vec<u8>> {
    ClientRequest::CalendarIdentifier
  }

  fn stale() -> Duration {
    Duration::max_value()
  }
}
