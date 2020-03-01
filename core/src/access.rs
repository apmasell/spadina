/// An access control rule
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum AccessControl<T: Copy> {
  /// Match a particular player access; if a time is provided, this rule will expire after that time
  Player(crate::player::PlayerIdentifier<String>, Option<chrono::DateTime<chrono::Utc>>, T),
  /// Match all users from a particular server access; if a time is provided, this rule will expire after that time
  Server(String, Option<chrono::DateTime<chrono::Utc>>, T),
  /// Match all users from any server with a domain suffix; if a time is provided, this rule will expire after that time
  Domain(String, Option<chrono::DateTime<chrono::Utc>>, T),
  /// Match all users from this server; if a time is provided, this rule will expire after that time
  Local(Option<chrono::DateTime<chrono::Utc>>, T),
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum BannedPeer<S: AsRef<str> + std::hash::Hash> {
  Peer(S),
  Domain(S),
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum BulkRealmSelector<S: AsRef<str> + std::cmp::Ord> {
  AllMine,
  AllForOther { player: S },
  AllServer,
  MineByAsset { assets: std::collections::BTreeSet<S> },
  OtherPlayerByAsset { assets: std::collections::BTreeSet<S>, player: S },
}
/// Whether to allow or deny access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum SimpleAccess {
  /// Allow access
  Allow,
  /// Deny access
  Deny,
}

/// Location/online access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum InteractiveAccess {
  /// Allow access
  Allow,
  /// Deny access
  Deny,
  /// Ask host
  Prompt,
}

/// Location/online access
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum LocationAccess {
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
  /// A player with access controls can may visit a realm on a server.
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
  /// When a new realm is created, set the access permissions for that realm to a copy of these
  NewRealmDefaultAccess,
  /// When a new realm is created, set the administration permissions for that realm to a copy of these
  NewRealmDefaultAdmin,
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
/// Check if an access control rule applies
///
/// * `acls` - the access control list to use
/// * `user` - the player being checked
/// * `local_server` - the name of the local server
pub fn check_acls<T: Copy, A: std::borrow::Borrow<AccessControl<T>> + Sized>(
  acls: impl IntoIterator<Item = A>,
  user: &crate::player::PlayerIdentifier<&str>,
  local_server: &str,
) -> Option<T> {
  let user = user.clone().globalize(local_server);
  fn time_ok(time: &Option<chrono::DateTime<chrono::Utc>>) -> bool {
    time.map(|t| t > chrono::Utc::now()).unwrap_or(true)
  }
  acls
    .into_iter()
    .filter_map(|a| match a.borrow() {
      AccessControl::Player(player, time, access) => {
        if time_ok(time) && player.as_ref().globalize(local_server) == user {
          Some(*access)
        } else {
          None
        }
      }
      AccessControl::Server(server_id, time, access) => {
        if time_ok(time) && server_id == user.get_server(local_server) {
          Some(*access)
        } else {
          None
        }
      }
      AccessControl::Domain(domain, time, access) => {
        if time_ok(time) && crate::net::has_domain_suffix(domain.as_str(), user.get_server(local_server)) {
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
}
impl<T: Copy> AccessControl<T> {
  pub fn as_local(self, local_server: &str) -> Option<Self> {
    fn is_live(time: &Option<chrono::DateTime<chrono::Utc>>) -> bool {
      match time {
        Some(t) => *t > chrono::Utc::now(),
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
          if &server == local_server {
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
impl<S: AsRef<str> + std::hash::Hash> BannedPeer<S> {
  pub fn clean(self) -> Option<BannedPeer<String>> {
    match self {
      BannedPeer::Peer(s) => crate::net::parse_server_name(s.as_ref()).map(BannedPeer::Peer),
      BannedPeer::Domain(s) => crate::net::parse_server_name(s.as_ref()).map(BannedPeer::Domain),
    }
  }
}
impl Default for SimpleAccess {
  fn default() -> Self {
    SimpleAccess::Deny
  }
}
impl Default for InteractiveAccess {
  fn default() -> Self {
    InteractiveAccess::Prompt
  }
}
impl Default for LocationAccess {
  fn default() -> Self {
    LocationAccess::Deny
  }
}
impl Default for AccountLockState {
  fn default() -> Self {
    AccountLockState::Unknown
  }
}
