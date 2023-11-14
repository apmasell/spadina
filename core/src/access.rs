use crate::player::PlayerIdentifier;
use crate::reference_converter::{AsReference, Converter, Referencer};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::hash::Hash;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum AccessControl<S: AsRef<str>, T: Copy> {
  /// Match a particular player access; if a time is provided, this rule will expire after that time
  Player(PlayerIdentifier<S>, Option<DateTime<Utc>>, T),
  /// Match all users from a particular server access; if a time is provided, this rule will expire after that time
  Server(S, Option<DateTime<Utc>>, T),
  /// Match all users from any server with a domain suffix; if a time is provided, this rule will expire after that time
  Domain(S, Option<DateTime<Utc>>, T),
  /// Match all users from this server; if a time is provided, this rule will expire after that time
  Local(Option<DateTime<Utc>>, T),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AccessSetting<S: AsRef<str>, T: Copy> {
  pub default: T,
  pub rules: Vec<AccessControl<S, T>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BannedPeer<S: AsRef<str> + Hash> {
  Peer(S),
  Domain(S),
}
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LocalAccessSetting<S: AsRef<str> + Eq + Ord> {
  default: SimpleAccess,
  exceptions: BTreeSet<S>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum BulkLocationSelector<S: AsRef<str> + Eq + Ord + Hash> {
  AllMine,
  AllForOther { player: S },
  AllServer,
  MineByDescriptor { descriptors: Vec<crate::location::Descriptor<S>> },
  OtherPlayerByDescriptor { descriptors: Vec<crate::location::Descriptor<S>>, player: S },
  MineByKind { kind: crate::location::DescriptorKind<S> },
  OtherPlayerByKind { kind: crate::location::DescriptorKind<S>, player: S },
}
/// Whether to allow or deny access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum SimpleAccess {
  /// Allow access
  Allow,
  /// Deny access
  Deny,
}

/// Whether to allow or deny access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum Privilege {
  /// Can access but not make changes
  Access,
  /// Can access or modify
  Admin,
  /// No access permitted
  Deny,
}

/// Location/online access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum OnlineAccess {
  /// Allow location and online status to be viewed
  Location,
  ///Only show online/offline status
  OnlineOnly,
  /// Deny any information
  Deny,
}

/// Sets the access control being read or changed
///
/// Access controls are meant to be applied in a layered approach. A player can be attempting to do one of three things:
/// * access a realm
/// * change ACLs for a realm (administration)
/// * send a direct message to a user
///
/// In any case, first a set of server ACLs are applied. If the user is allowed, then a realm-specific (for access or administration) or user-specific (for direct messages) are applied. If still allowed, then the action is permitted.
///
/// Access to a realm implies broadcast messaging while in that realm.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AccessTarget {
  /// Access controls for a server.
  ///
  /// A player with access can may visit a realm on this server from a remote server. This is not checked for local players.
  AccessServer,
  /// Administrator controls for a server.
  ///
  /// A player with administrator controls can change access controls on any realm in a server or direct messaging on a server or change the server admin access.
  AdminServer,
  /// Whether a player can create assets or host on this server
  CreateOnServer,
  /// Sending direct messages to a user
  ///
  /// A player with direct messaging can send messages to a recipient player
  DirectMessages,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AccountLockState {
  /// The account is know to be locked and can be unlocked
  Locked,
  /// The requester is not allowed to change account locks
  NotAllowed,
  /// The account is locked and the server doesn't have a way to change that (the account locking is controlled by another system)
  PermanentlyLocked,
  /// The account is unlocked and the server doesn't have a way to change that (the account locking is controlled by another system)
  PermanentlyUnlocked,
  /// The account locking status cannot be determined due to an error
  Unknown,
  /// The account should be able to log in
  Unlocked,
}

impl<S: AsRef<str>, T: Copy> AccessControl<S, T> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> AccessControl<R::Output<'a>, T>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      AccessControl::Player(player, time, permission) => AccessControl::Player(player.reference(reference), time.clone(), *permission),
      AccessControl::Server(server, time, permission) => AccessControl::Server(reference.convert(server), time.clone(), *permission),
      AccessControl::Domain(domain, time, permission) => AccessControl::Domain(reference.convert(domain), time.clone(), *permission),
      AccessControl::Local(time, permission) => AccessControl::Local(time.clone(), *permission),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> AccessControl<C::Output, T>
  where
    C::Output: AsRef<str>,
  {
    match self {
      AccessControl::Player(player, time, permission) => AccessControl::Player(player.convert(converter), time, permission),
      AccessControl::Server(server, time, permission) => AccessControl::Server(converter.convert(server), time, permission),
      AccessControl::Domain(domain, time, permission) => AccessControl::Domain(converter.convert(domain), time, permission),
      AccessControl::Local(time, permission) => AccessControl::Local(time, permission),
    }
  }
  pub fn into_local(self, local_server: &str) -> Option<Self> {
    fn is_live(time: &Option<DateTime<Utc>>) -> bool {
      match time {
        Some(t) => *t > Utc::now(),
        None => true,
      }
    }
    match self {
      AccessControl::Player(player, time, access) => {
        if is_live(&time) {
          Some(AccessControl::Player(player, time, access))
        } else {
          None
        }
      }
      AccessControl::Server(server, time, access) => {
        if is_live(&time) {
          if server.as_ref() == local_server {
            Some(AccessControl::Local(time, access))
          } else {
            Some(AccessControl::Server(server, time, access))
          }
        } else {
          None
        }
      }
      AccessControl::Domain(suffix, time, access) => {
        if is_live(&time) {
          Some(AccessControl::Domain(suffix, time, access))
        } else {
          None
        }
      }
      AccessControl::Local(time, access) => {
        if is_live(&time) {
          Some(AccessControl::Local(time, access))
        } else {
          None
        }
      }
    }
  }
}

impl<S: AsRef<str>, T: Copy> AccessSetting<S, T> {
  pub fn check(&self, player: &PlayerIdentifier<impl AsRef<str>>, local_server: &str) -> T {
    let user = player.reference(AsReference::<str>::default()).globalize(local_server);
    fn time_ok(time: &Option<DateTime<Utc>>) -> bool {
      time.as_ref().map(|&t| t > Utc::now()).unwrap_or(true)
    }
    self
      .rules
      .iter()
      .filter_map(|a| match a {
        AccessControl::Player(player, time, access) => {
          if time_ok(time) && player.reference(AsReference::<str>::default()).globalize(local_server) == user {
            Some(*access)
          } else {
            None
          }
        }
        AccessControl::Server(server_id, time, access) => {
          if time_ok(time) && server_id.as_ref() == user.get_server(local_server) {
            Some(*access)
          } else {
            None
          }
        }
        AccessControl::Domain(domain, time, access) => {
          if time_ok(time) && crate::net::has_domain_suffix(domain.as_ref(), user.get_server(local_server)) {
            Some(*access)
          } else {
            None
          }
        }
        AccessControl::Local(time, access) => {
          if time_ok(time) && user.get_server(local_server) == local_server {
            Some(*access)
          } else {
            None
          }
        }
      })
      .next()
      .unwrap_or(self.default)
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> AccessSetting<C::Output, T>
  where
    C::Output: AsRef<str>,
  {
    AccessSetting { default: self.default, rules: self.rules.into_iter().map(|rule| rule.convert(converter)).collect() }
  }
}

impl<S: AsRef<str>, T: Copy + Default> Default for AccessSetting<S, T> {
  fn default() -> Self {
    Self { default: Default::default(), rules: Vec::new() }
  }
}
impl Default for AccountLockState {
  fn default() -> Self {
    AccountLockState::Unknown
  }
}
impl<S: AsRef<str> + Hash> BannedPeer<S> {
  pub fn convert<C: Converter<S>>(self, converter: C) -> BannedPeer<C::Output>
  where
    C::Output: AsRef<str> + Hash,
  {
    match self {
      BannedPeer::Peer(p) => BannedPeer::Peer(converter.convert(p)),
      BannedPeer::Domain(d) => BannedPeer::Domain(converter.convert(d)),
    }
  }
  pub fn clean(self) -> Option<BannedPeer<String>> {
    match self {
      BannedPeer::Peer(s) => crate::net::parse_server_name(s.as_ref()).map(BannedPeer::Peer),
      BannedPeer::Domain(s) => crate::net::parse_server_name(s.as_ref()).map(BannedPeer::Domain),
    }
  }
  pub fn reference<'a, R: Referencer<S>>(&'a self, referencer: R) -> BannedPeer<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str> + Hash,
  {
    match self {
      BannedPeer::Peer(p) => BannedPeer::Peer(referencer.convert(p)),
      BannedPeer::Domain(d) => BannedPeer::Domain(referencer.convert(d)),
    }
  }
}
impl<S: AsRef<str> + Eq + Ord> LocalAccessSetting<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> LocalAccessSetting<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str> + Ord + Eq,
  {
    LocalAccessSetting { default: self.default, exceptions: self.exceptions.iter().map(|e| reference.convert(e)).collect() }
  }
  pub fn allow(&mut self, player: S) -> bool {
    if self.default == SimpleAccess::Allow {
      self.exceptions.remove(&player)
    } else {
      self.exceptions.insert(player)
    }
  }
  pub fn check(&self, player: &str) -> SimpleAccess
  where
    S: Borrow<str>,
  {
    if self.exceptions.contains(player) {
      self.default.invert()
    } else {
      self.default
    }
  }
  pub fn deny(&mut self, player: S) -> bool {
    if self.default == SimpleAccess::Deny {
      self.exceptions.remove(&player)
    } else {
      self.exceptions.insert(player)
    }
  }
  pub fn reset(&mut self, default: SimpleAccess) -> bool {
    if self.default == default {
      false
    } else {
      self.default = default;
      self.exceptions.clear();
      true
    }
  }
}
impl<S: AsRef<str> + Eq + Ord> Default for LocalAccessSetting<S> {
  fn default() -> Self {
    LocalAccessSetting { default: SimpleAccess::Allow, exceptions: Default::default() }
  }
}

impl Default for OnlineAccess {
  fn default() -> Self {
    OnlineAccess::Deny
  }
}
impl Privilege {
  pub fn can_access(&self) -> bool {
    *self != Privilege::Deny
  }
}
impl SimpleAccess {
  pub fn invert(&self) -> Self {
    match self {
      SimpleAccess::Allow => SimpleAccess::Deny,
      SimpleAccess::Deny => SimpleAccess::Allow,
    }
  }
}
impl Default for Privilege {
  fn default() -> Self {
    Privilege::Deny
  }
}
impl Default for SimpleAccess {
  fn default() -> Self {
    SimpleAccess::Deny
  }
}
