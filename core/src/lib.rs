use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

pub mod access;
pub mod asset;
pub mod asset_store;
pub mod auth;
pub mod avatar;
pub mod capabilities;
pub mod communication;
pub mod location;
pub mod net;
pub mod physics;
pub mod player;
pub mod puzzle;
pub mod realm;
pub mod self_hosted;
pub mod user_interface;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum AssetError {
  ///The asset is not recognized by the server
  UnknownKind,
  /// The asset couldn't be decoded
  DecodeFailure,
  /// The data failed validation.
  Invalid,
  InternalError,
  /// The data provided references unknown assets.
  Missing(Vec<String>),
  /// The user doesn't have the rights to upload assets to this server
  PermissionError,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientRequest<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord> {
  /// Request an account be locked or unlocked
  AccountLockChange {
    id: i32,
    name: S,
    locked: bool,
  },
  /// Check whether an account is locked or not
  AccountLockStatus {
    name: S,
  },
  /// Request the contents of a particular asset, using its hash ID
  AssetPull {
    principal: S,
  },
  /// Uploads a new asset to the server. The client must generate a unique ID that the server
  /// will respond with
  AssetCreate {
    id: i32,
    asset_type: S,
    name: S,
    tags: Vec<S>,
    licence: asset::Licence,
    compression: asset::Compression,
    data: Vec<u8>,
  },
  /// Get the ACLs for a realm or communication.
  AccessGet {
    target: access::AccessTarget,
  },
  /// Get the ACLs for a location and online status sharing.
  AccessGetLocation,
  /// Set the ACLs for a realm or communication settings. The client must generate a unique ID
  /// that the server will respond with if the ACLs can be updated. If the realm is missing, the home realm is used.
  AccessSet {
    id: i32,
    target: access::AccessTarget,
    rules: Vec<access::AccessControl<access::SimpleAccess>>,
    default: access::SimpleAccess,
  },
  AccessSetBulk {
    id: i32,
    targets: std::collections::HashMap<realm::RealmAccessTarget, access::AccessControl<access::SimpleAccess>>,
    realms: access::BulkRealmSelector<S>,
  },
  AccessLocationSet {
    id: i32,
    rules: Vec<access::AccessControl<access::LocationAccess>>,
    default: access::LocationAccess,
  },
  /// Adds an announcement to the global announcement list
  AnnouncementAdd {
    id: i32,
    announcement: communication::Announcement<S>,
  },
  /// Clears all global anouncements
  AnnouncementClear {
    id: i32,
  },
  /// Fetches the global announcements list (though they are sent unsolicted upon change)
  AnnouncementList,
  /// Gets the player's current avatar
  ///
  /// If the player has not ever created an avatar, a default will be provided.
  AvatarGet,
  /// Sets the player's current avatar, replacing any existing avatar
  AvatarSet {
    id: i32,
    avatar: avatar::Avatar,
  },
  /// Add a new asset to the book mark list; if this book mark is already there, no action is taken.
  BookmarkAdd {
    id: i32,
    bookmark: communication::Bookmark<S>,
  },
  /// Retrieve all bookmarks
  BookmarksList,
  /// Remove an asset from the book mark list. If not present, this is a no-op.
  BookmarkRemove {
    id: i32,
    bookmark: communication::Bookmark<S>,
  },
  CalendarIdentifier,
  CalendarReset {
    id: i32,
    player: Option<S>,
  },
  CalendarRealmAdd {
    id: i32,
    realm: realm::LocalRealmTarget<S>,
  },
  CalendarRealmClear {
    id: i32,
  },
  CalendarRealmList,
  CalendarRealmRemove {
    id: i32,
    realm: realm::LocalRealmTarget<S>,
  },
  ConsensualEmoteRequest {
    emote: S,
    player: crate::player::PlayerIdentifier<S>,
  },
  ConsensualEmoteResponse {
    id: i32,
    ok: bool,
  },

  /// Retrieve direct messages between this player and another
  DirectMessageGet {
    player: player::PlayerIdentifier<S>,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// Send a message to a player on this or any server.
  DirectMessageSend {
    id: i32,
    recipient: player::PlayerIdentifier<S>,
    body: communication::MessageBody<S>,
  },
  /// Get a summary of when the last direct message received for this player
  DirectMessageStats,
  /// Create an invitation for a new player
  Invite {
    id: i32,
  },
  FollowRequest {
    player: crate::player::PlayerIdentifier<S>,
  },
  FollowResponse {
    id: i32,
    ok: bool,
  },
  /// Request that we are moved to a new location
  LocationChange {
    location: location::LocationRequest<S>,
  },
  /// Send a message to the group chat associated with the location we are currently logged into. If
  /// not in a location, the server should discard the message.
  LocationMessageSend {
    body: crate::communication::MessageBody<S>,
  },
  /// Retrieve older messages for a location
  LocationMessagesGet {
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// Do nothing. Used for keep alive/testing connection
  NoOperation,
  /// Tell the server that we no longer want to play. This should remove our avatar from the
  /// current realm and terminate the connection.
  Quit,
  /// List any servers that are in contact with the local server
  Peers,
  /// Removes servers from the banned list
  PeerBanClear {
    id: i32,
    bans: Vec<access::BannedPeer<S>>,
  },
  /// List servers that are banned
  PeerBanList,
  /// Adds servers to the ban list
  PeerBanSet {
    id: i32,
    bans: Vec<access::BannedPeer<S>>,
  },
  /// Try to get the online status and location of another player
  PlayerCheck {
    player: player::PlayerIdentifier<S>,
  },
  /// Erases a player from the server on next restart
  ///
  /// All of their realms and chats will be deleted. This does *not* prevent them from logging in again. They must also be removed from the authentication provider.
  ///
  /// Only administrators can perform this command.
  PlayerReset {
    id: i32,
    reset: bool,
    player: S,
  },

  /// Adds a new public key for this player to login with. If the name is already used, this key will replace it.
  PublicKeyAdd {
    id: i32,
    der: Vec<u8>,
  },
  /// Removes a public key for this player to login with.
  PublicKeyDelete {
    id: i32,
    name: S,
  },
  /// Removes all public keys for this player to login with
  PublicKeyDeleteAll {
    id: i32,
  },
  /// List the names of all the keys a player can log in with
  PublicKeyList,
  /// List the realms that we can access.
  RealmsList {
    source: realm::RealmSource<S>,
  },
  RealmDelete {
    id: i32,
    asset: S,
    owner: Option<S>,
  },
  /// Add a realm asset identifier to the train
  ///
  /// The asset will be check to see if it is train-compatible. If it is not, the request will be ignored.
  TrainAdd {
    id: i32,
    asset: S,
    allow_first: bool,
  },
  /// Request from player, who is the guest, to the host
  ToHost {
    request: self_hosted::GuestRequest<S>,
  },
  /// Send a response from the player, who is the host, in self-hosted mode
  FromHost {
    request: self_hosted::HostCommand<S>,
  },
  /// Requests that are tied to being in a particular realm. This mostly matters when a player is accessing a realm on another server as these requests will be proxied to the remote server without modification. When a player is first sent to a realm, it is told to download the assets. Once a player has all the assets, it should send any in-realm command to be actually moved into the realm. This will trigger the server to send the current realm state.
  InRealm {
    request: realm::RealmRequest<S>,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientResponse<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord> {
  /// Result of trying to lock an account
  AccountLockChange {
    id: i32,
    name: S,
    result: UpdateResult,
  },
  /// The current status of an account
  AccountLockStatus {
    name: S,
    status: access::AccountLockState,
  },
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange {
    id: i32,
    response: UpdateResult,
  },
  /// The server announcements have changed
  Announcements {
    announcements: Vec<communication::Announcement<S>>,
  },
  /// Whether updating the announcements (either set or clear) was successful (true) or failed (false)
  AnnouncementUpdate {
    id: i32,
    result: UpdateResult,
  },
  /// The asset was created successfully. The hash for the asset is provided.
  AssetCreationSucceeded {
    id: i32,
    hash: S,
  },
  /// The asset could not be created
  AssetCreationFailed {
    id: i32,
    error: AssetError,
  },
  /// An asset the client requested is available.
  Asset {
    principal: S,
    asset: asset::Asset,
  },
  /// An asset requested by the client is corrupt or cannot be loaded due to an internal error
  AssetUnavailable {
    principal: S,
  },
  /// The player's current avatar
  AvatarCurrent {
    avatar: avatar::Avatar,
  },
  /// Whether updating the avatar was successful (true) or failed (false)
  AvatarUpdate {
    id: i32,
    success: bool,
  },
  /// The book marks that are currently stored on the server
  Bookmarks {
    bookmarks: std::collections::HashSet<communication::Bookmark<S>>,
  },
  /// Whether updating the bookmark (add or remove) was successful (true) or failed (false)
  BookmarkUpdate {
    id: i32,
    success: bool,
  },
  /// The server is about to terminate the connection. This maybe in a response to a request or
  /// unsolicited.
  Disconnect,
  // The current calendar ID for a player
  Calendar {
    id: Vec<u8>,
  },
  CalendarUpdate {
    id: i32,
    result: UpdateResult,
  },
  CalendarRealmList {
    realms: Vec<realm::RealmDirectoryEntry<S>>,
  },
  CalendarRealmChange {
    id: i32,
    success: bool,
  },
  ConsensualEmoteRequest {
    id: i32,
    emote: S,
    player: crate::player::PlayerIdentifier<S>,
  },
  /// The current ACLs associated with a realm
  CurrentAccess {
    target: access::AccessTarget,
    rules: Vec<access::AccessControl<access::SimpleAccess>>,
    default: access::SimpleAccess,
  },
  CurrentAccessLocation {
    rules: Vec<access::AccessControl<access::LocationAccess>>,
    default: access::LocationAccess,
  },
  /// The status of a direct message that was sent
  DirectMessageReceipt {
    id: i32,
    status: communication::DirectMessageStatus,
  },
  /// The last message receipt time for all players this player has received messages from
  DirectMessageStats {
    stats: std::collections::HashMap<player::PlayerIdentifier<S>, communication::DirectMessageInfo>,
    last_login: chrono::DateTime<chrono::Utc>,
  },
  /// All the direct messages in a particular time frame requested
  DirectMessages {
    player: player::PlayerIdentifier<S>,
    messages: Vec<communication::DirectMessage<S>>,
  },
  FollowRequest {
    id: i32,
    player: crate::player::PlayerIdentifier<S>,
  },
  LocationAvatars {
    players: std::collections::HashMap<crate::player::PlayerIdentifier<S>, crate::avatar::Avatar>,
  },
  /// Where the player is currently located
  LocationChange {
    location: location::LocationResponse<S>,
  },
  /// A message was posted in the current realm's chat.
  LocationMessagePosted {
    sender: crate::player::PlayerIdentifier<S>,
    body: crate::communication::MessageBody<S>,
    timestamp: chrono::DateTime<chrono::Utc>,
  },
  /// A collection of messages when a time range was queried
  LocationMessages {
    messages: Vec<location::LocationMessage<S>>,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// No event, just a keep alive
  NoOperation,
  ToHost {
    event: self_hosted::HostEvent<S>,
  },
  /// A response from the host in to this player, who is guest
  FromHost {
    response: self_hosted::GuestResponse<S>,
  },
  /// The invitation created for a new player
  InviteSuccess {
    id: i32,
    url: S,
  },
  /// The failed reason an invitation could not be created for a new player
  InviteFailure {
    id: i32,
    error: communication::InvitationError,
  },
  /// Servers that are currently in contact with this server
  Peers {
    peers: Vec<S>,
  },
  /// Servers that are currently in banned on this server
  PeersBanned {
    bans: std::collections::HashSet<access::BannedPeer<S>>,
  },
  /// Whether updating a banned server (add, delete, clear) was successful (true) or failed (false)
  PeersBannedUpdate {
    id: i32,
    result: UpdateResult,
  },
  /// The result of trying to reset a player's information
  PlayerReset {
    id: i32,
    result: UpdateResult,
  },
  /// All the public keys this player can use to log in with (instead of providing a username/password)
  PublicKeys {
    keys: Vec<auth::PublicKey<S>>,
  },
  /// Whether updating a public key (add, delete, clear) was successful (true) or failed (false)
  PublicKeyUpdate {
    id: i32,
    result: UpdateResult,
  },
  RealmDelete {
    id: i32,
    result: UpdateResult,
  },
  /// The list of available realms for the filtering criteria provided
  RealmsAvailable {
    display: realm::RealmSource<S>,
    realms: Vec<realm::RealmDirectoryEntry<S>>,
  },
  /// Information on the whereabouts of a player
  PlayerState {
    player: player::PlayerIdentifier<S>,
    state: player::PlayerLocationState<S>,
  },
  TrainAdd {
    id: i32,
    result: TrainAddResult,
  },
  /// An event happened in a realm. If the player is accessing a realm on another server, these are proxied by the local server.
  InRealm {
    response: realm::RealmResponse<S>,
  },
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum TrainAddResult {
  Asset(AssetError),
  InternalError,
  NotAllowed,
  NotFound,
  NotRealm,
  NotTrain,
  Success,
}
/// The result of an update operation that requires permissions
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum UpdateResult {
  Success,
  InternalError,
  NotAllowed,
}

pub fn abs_difference<T: std::ops::Sub<Output = T> + Ord>(x: T, y: T) -> T {
  if x < y {
    y - x
  } else {
    x - y
  }
}

impl AssetError {
  pub fn description(&self) -> std::borrow::Cow<'static, str> {
    match self {
      AssetError::UnknownKind => std::borrow::Cow::Borrowed("Asset kind is not supported"),
      AssetError::DecodeFailure => std::borrow::Cow::Borrowed("Failed to decode asset data"),
      AssetError::Invalid => std::borrow::Cow::Borrowed("Asset failed validation"),
      AssetError::InternalError => std::borrow::Cow::Borrowed("Internal error"),
      AssetError::Missing(children) => std::borrow::Cow::Owned(format!("Asset missing dependencies: {:?}", children)),
      AssetError::PermissionError => std::borrow::Cow::Borrowed("Not authorized to store asset"),
    }
  }
}
impl std::fmt::Display for AssetError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      AssetError::UnknownKind => f.write_str("Asset kind is not supported"),
      AssetError::DecodeFailure => f.write_str("Failed to decode asset data"),
      AssetError::Invalid => f.write_str("Asset failed validation"),
      AssetError::InternalError => f.write_str("Internal Error"),
      AssetError::Missing(children) => {
        f.write_str("Asset missing dependencies: ")?;
        children.fmt(f)
      }
      AssetError::PermissionError => f.write_str("Not authorized to store asset"),
    }
  }
}
impl<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord + serde::Serialize> crate::net::ToWebMessage for ClientRequest<S> {}
impl<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord + serde::Serialize> crate::net::ToWebMessage for ClientResponse<S> {}

impl UpdateResult {
  pub fn description(&self) -> &'static str {
    match self {
      UpdateResult::Success => "success",
      UpdateResult::InternalError => "internal error",
      UpdateResult::NotAllowed => "not allowed",
    }
  }
}

impl Display for UpdateResult {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.description())
  }
}
impl Display for TrainAddResult {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TrainAddResult::Asset(e) => std::fmt::Display::fmt(e, f),
      TrainAddResult::InternalError => f.write_str("internal error"),
      TrainAddResult::NotAllowed => f.write_str("internal error"),
      TrainAddResult::NotFound => f.write_str("asset not found"),
      TrainAddResult::NotRealm => f.write_str("asset is not a realm"),
      TrainAddResult::NotTrain => f.write_str("realm is not usable for a train"),
      TrainAddResult::Success => f.write_str("success"),
    }
  }
}
