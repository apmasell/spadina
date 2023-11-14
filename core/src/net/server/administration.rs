use crate::{access, communication, UpdateResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AdministrationRequest<S: AsRef<str>> {
  /// Request an account be locked or unlocked
  AccountLockChange { name: S, locked: bool },
  /// Check whether an account is locked or not
  AccountLockStatus { name: S },
  /// Create an invitation for a new player
  Invite,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AdministrationResponse<S: AsRef<str>> {
  /// Result of trying to lock an account
  AccountLockChange {
    name: S,
    result: UpdateResult,
  },
  /// The current status of an account
  AccountLockStatus {
    name: S,
    status: access::AccountLockState,
  },
  /// The invitation created for a new player
  InviteSuccess {
    url: S,
  },
  /// The failed reason an invitation could not be created for a new player
  InviteFailure {
    error: communication::InvitationError,
  },
  NotAdministrator,
}
