use chrono::Utc;
use openssl::pkey::{PKey, Public};
use spadina_core::access::{AccessControl, AccessSetting, BannedPeer, OnlineAccess, Privilege, SimpleAccess};
use spadina_core::avatar::Avatar;
use spadina_core::communication::Announcement;
use spadina_core::location::target::LocalTarget;
use spadina_core::net::server::auth::{compute_fingerprint, PublicKey};
use spadina_core::net::server::ClientRequest;
use spadina_core::reference_converter::AsReference;
use spadina_core::resource::Resource;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use tokio_tungstenite::tungstenite::Message;

pub trait Update<V> {
  fn update(&self, id: u32, entry: Option<&mut V>) -> Option<Message>;
}

pub struct Add<T>(pub T);

pub struct Clear;

pub struct Remove<T>(pub T);

pub struct Set<T>(pub T);

impl Update<Vec<Announcement<String>>> for Add<&Announcement<String>> {
  fn update(&self, id: u32, announcements: Option<&mut Vec<Announcement<String>>>) -> Option<Message> {
    let message = ClientRequest::<_, &[u8]>::AnnouncementAdd { id, announcement: self.0.reference(AsReference::<str>::default()) }.into();
    if let Some(announcements) = announcements {
      announcements.push(self.0.clone())
    }
    Some(message)
  }
}

impl Update<Vec<Announcement<String>>> for Clear {
  fn update(&self, id: u32, announcements: Option<&mut Vec<Announcement<String>>>) -> Option<Message> {
    if let Some(announcements) = announcements {
      announcements.clear();
    }
    Some(ClientRequest::<String, &[u8]>::AnnouncementClear { id }.into())
  }
}

impl Update<Avatar> for Set<Avatar> {
  fn update(&self, id: u32, avatar: Option<&mut Avatar>) -> Option<Message> {
    if let Some(avatar) = avatar {
      *avatar = self.0.clone();
    }
    Some(ClientRequest::<String, &[u8]>::AvatarSet { id, avatar: self.0.clone() }.into())
  }
}

impl Update<BTreeMap<String, PublicKey>> for Clear {
  fn update(&self, id: u32, keys: Option<&mut BTreeMap<String, PublicKey>>) -> Option<Message> {
    if let Some(keys) = keys {
      keys.clear();
    }
    Some(ClientRequest::<String, &[u8]>::PublicKeyDeleteAll { id }.into())
  }
}

impl<S: AsRef<str>> Update<BTreeMap<String, PublicKey>> for Remove<S> {
  fn update(&self, id: u32, keys: Option<&mut BTreeMap<String, PublicKey>>) -> Option<Message> {
    if let Some(keys) = keys {
      if keys.remove(self.0.as_ref()).is_none() {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::PublicKeyDelete { id, name: self.0.as_ref() }.into())
  }
}

impl Update<BTreeMap<String, PublicKey>> for Add<&[u8]> {
  fn update(&self, id: u32, keys: Option<&mut BTreeMap<String, PublicKey>>) -> Option<Message> {
    if let Some(keys) = keys {
      let fingerprint = compute_fingerprint(self.0);
      if keys.contains_key(fingerprint.as_str()) {
        return None;
      }
      keys.insert(fingerprint, PublicKey { created: Utc::now(), last_used: None });
    }
    Some(ClientRequest::<&str, _>::PublicKeyAdd { id, der: self.0 }.into())
  }
}
impl Update<BTreeMap<String, PublicKey>> for Add<&PKey<Public>> {
  fn update(&self, id: u32, keys: Option<&mut BTreeMap<String, PublicKey>>) -> Option<Message> {
    Add(self.0.public_key_to_der().expect("Failed to encode public key").as_slice()).update(id, keys)
  }
}

impl Update<HashSet<BannedPeer<String>>> for Add<&BannedPeer<String>> {
  fn update(&self, id: u32, bans: Option<&mut HashSet<BannedPeer<String>>>) -> Option<Message> {
    if let Some(bans) = bans {
      if !bans.insert(self.0.clone()) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::PeerBanAdd { id, ban: self.0.reference(AsReference::<str>::default()) }.into())
  }
}

impl Update<HashSet<BannedPeer<String>>> for Remove<&BannedPeer<String>> {
  fn update(&self, id: u32, bans: Option<&mut HashSet<BannedPeer<String>>>) -> Option<Message> {
    if let Some(bans) = bans {
      if !bans.remove(self.0) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::PeerBanRemove { id, ban: self.0.reference(AsReference::<str>::default()) }.into())
  }
}

impl Update<BTreeSet<LocalTarget<String>>> for Add<LocalTarget<String>> {
  fn update(&self, id: u32, subscriptions: Option<&mut BTreeSet<LocalTarget<String>>>) -> Option<Message> {
    if let Some(subscriptions) = subscriptions {
      if !subscriptions.insert(self.0.clone()) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::CalendarLocationAdd { id, location: self.0.reference(AsReference::<str>::default()) }.into())
  }
}

impl Update<BTreeSet<LocalTarget<String>>> for Clear {
  fn update(&self, id: u32, subscriptions: Option<&mut BTreeSet<LocalTarget<String>>>) -> Option<Message> {
    if let Some(subscriptions) = subscriptions {
      subscriptions.clear()
    }
    Some(ClientRequest::<String, &[u8]>::CalendarLocationClear { id }.into())
  }
}

impl Update<BTreeSet<LocalTarget<String>>> for Remove<&LocalTarget<String>> {
  fn update(&self, id: u32, subscriptions: Option<&mut BTreeSet<LocalTarget<String>>>) -> Option<Message> {
    if let Some(subscriptions) = subscriptions {
      if !subscriptions.remove(self.0) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::CalendarLocationRemove { id, location: self.0.reference(AsReference::<str>::default()) }.into())
  }
}

trait AccessSettingRequest: Copy {
  fn request(id: u32, rules: Vec<AccessControl<&str, Self>>, default: Self) -> ClientRequest<&str, &[u8]>;
}

impl<T: AccessSettingRequest> Update<AccessSetting<String, T>> for Set<&AccessSetting<String, T>> {
  fn update(&self, id: u32, access: Option<&mut AccessSetting<String, T>>) -> Option<Message> {
    if let Some(access) = access {
      *access = self.0.clone();
    }
    Some(T::request(id, self.0.rules.iter().map(|rule| rule.reference(AsReference::<str>::default())).collect(), self.0.default).into())
  }
}

impl AccessSettingRequest for OnlineAccess {
  fn request(id: u32, rules: Vec<AccessControl<&str, Self>>, default: Self) -> ClientRequest<&str, &[u8]> {
    ClientRequest::<_, &[u8]>::AccessSetOnline { id, rules, default }
  }
}
impl AccessSettingRequest for SimpleAccess {
  fn request(id: u32, rules: Vec<AccessControl<&str, Self>>, default: Self) -> ClientRequest<&str, &[u8]> {
    ClientRequest::<_, &[u8]>::AccessSetDirectMessage { id, rules, default }
  }
}
impl AccessSettingRequest for Privilege {
  fn request(id: u32, rules: Vec<AccessControl<&str, Self>>, default: Self) -> ClientRequest<&str, &[u8]> {
    ClientRequest::<_, &[u8]>::AccessSetDefault { id, rules, default }
  }
}

impl Update<HashSet<Resource<String>>> for Add<&Resource<String>> {
  fn update(&self, id: u32, bookmarks: Option<&mut HashSet<Resource<String>>>) -> Option<Message> {
    if let Some(bookmarks) = bookmarks {
      if !bookmarks.insert(self.0.clone()) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::BookmarkAdd { id, bookmark: self.0.reference(AsReference::<str>::default()) }.into())
  }
}

impl Update<HashSet<Resource<String>>> for Remove<&Resource<String>> {
  fn update(&self, id: u32, bookmarks: Option<&mut HashSet<Resource<String>>>) -> Option<Message> {
    if let Some(bookmarks) = bookmarks {
      if !bookmarks.remove(self.0) {
        return None;
      }
    }
    Some(ClientRequest::<_, &[u8]>::BookmarkRemove { id, bookmark: self.0.reference(AsReference::<str>::default()) }.into())
  }
}
