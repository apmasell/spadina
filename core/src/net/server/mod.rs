use crate::asset::Asset;
use crate::communication::DirectMessageInfo;
use crate::location::directory::{Activity, Visibility};
use crate::location::protocol;
use crate::player::PlayerIdentifier;
use crate::resource::Resource;
use crate::{access, avatar, communication, location, player, UpdateResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{Debug, Display};
use tokio_tungstenite::tungstenite::Message;

pub mod administration;
pub mod auth;
pub mod hosting;

pub const AUTH_METHOD_PATH: &str = "/api/auth/method";

pub const CALENDAR_PATH: &str = "/api/calendar";

pub const CAPABILITY_HEADER: &str = "X-Spadina-Capability";

pub const CLIENT_KEY_PATH: &str = "/api/client/key";

pub const CLIENT_V1_PATH: &str = "/api/client/v1";

pub const PASSWORD_AUTH_PATH: &str = "/api/auth/password";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientRequest<S: AsRef<str> + Eq + std::hash::Hash + Ord, B> {
  Activity {
    id: u32,
    player: PlayerIdentifier<S>,
  },
  Administration {
    id: u32,
    request: administration::AdministrationRequest<S>,
  },
  /// Request the contents of a particular asset, using its hash ID
  AssetPull {
    id: u32,
    principal: S,
  },
  /// Uploads a complete asset
  AssetUpload {
    id: u32,
    asset: Asset<S, B>,
  },
  /// Get the ACLs for who can send direct messages
  AccessGetDirectMessage,
  /// Get the default ACL for new locations
  AccessGetDefault,
  /// Get the ACLs for a location and online status sharing.
  AccessGetOnline,
  AccessSetDefault {
    id: u32,
    rules: Vec<access::AccessControl<S, access::Privilege>>,
    default: access::Privilege,
  },
  /// Set the ACLs for direct messages.
  AccessSetDirectMessage {
    id: u32,
    rules: Vec<access::AccessControl<S, access::SimpleAccess>>,
    default: access::SimpleAccess,
  },
  AccessSetLocationBulk {
    id: u32,
    rules: Vec<access::AccessControl<S, access::Privilege>>,
    default: access::Privilege,
    selection: access::BulkLocationSelector<S>,
  },
  AccessSetOnline {
    id: u32,
    rules: Vec<access::AccessControl<S, access::OnlineAccess>>,
    default: access::OnlineAccess,
  },

  /// Adds an announcement to the global announcement list
  AnnouncementAdd {
    id: u32,
    announcement: communication::Announcement<S>,
  },
  /// Clears all global announcements
  AnnouncementClear {
    id: u32,
  },
  /// Fetches the global announcements list (though they are sent unsolicited upon change)
  AnnouncementList,
  /// Gets the player's current avatar
  ///
  /// If the player has not ever created an avatar, a default will be provided.
  AvatarGet,
  /// Sets the player's current avatar, replacing any existing avatar
  AvatarSet {
    id: u32,
    avatar: avatar::Avatar,
  },
  /// Add a new asset to the book mark list; if this book mark is already there, no action is taken.
  BookmarkAdd {
    id: u32,
    bookmark: Resource<S>,
  },
  /// Retrieve all bookmarks
  BookmarksList,
  /// Remove an asset from the book mark list. If not present, this is a no-op.
  BookmarkRemove {
    id: u32,
    bookmark: Resource<S>,
  },
  CalendarIdentifier,
  CalendarReset,
  CalendarLocationAdd {
    id: u32,
    location: location::target::AbsoluteTarget<S>,
  },
  CalendarLocationClear {
    id: u32,
  },
  CalendarLocationList,
  CalendarLocationRemove {
    id: u32,
    location: location::target::AbsoluteTarget<S>,
  },

  /// Retrieve direct messages between this player and another
  DirectMessageGet {
    player: PlayerIdentifier<S>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
  },
  /// Send a message to a player on this or any server.
  DirectMessageSend {
    id: u32,
    recipient: PlayerIdentifier<S>,
    body: communication::MessageBody<S>,
  },
  /// Get a summary of when the last direct message received for this player
  DirectMessageStats,
  /// Request that we are moved to a new location
  LocationChange {
    location: location::change::LocationChangeRequest<S>,
  },
  /// List any servers that are in contact with the local server
  Peers,
  /// Adds servers to the ban list
  PeerBanAdd {
    id: u32,
    ban: access::BannedPeer<S>,
  },
  /// Removes all servers from the banned list
  PeerBanClear {
    id: u32,
  },
  /// List servers that are banned
  PeerBanList,
  /// Adds servers to the ban list
  PeerBanRemove {
    id: u32,
    ban: access::BannedPeer<S>,
  },
  /// Try to get the online status and location of another player
  PlayerOnlineCheck {
    id: u32,
    player: PlayerIdentifier<S>,
  },
  /// Erases a player from the server on next restart
  ///
  /// All of their realms and chats will be deleted. This does *not* prevent them from logging in again. They must also be removed from the authentication provider.
  ///
  /// Only administrators can perform this command.
  PlayerReset {
    id: u32,
    player: S,
  },

  /// Adds a new public key for this player to login with. If the name is already used, this key will replace it.
  PublicKeyAdd {
    id: u32,
    der: B,
  },
  /// Removes a public key for this player to login with.
  PublicKeyDelete {
    id: u32,
    name: S,
  },
  /// Removes all public keys for this player to login with
  PublicKeyDeleteAll {
    id: u32,
  },
  /// List the names of all the keys a player can log in with
  PublicKeyList,
  /// List the realms that we can access.
  LocationsList {
    id: u32,
    source: location::directory::Search<S>,
    timeout: u16,
  },
  LocationChangeVisibility {
    id: u32,
    visibility: Visibility,
    selection: access::BulkLocationSelector<S>,
  },
  /// Send a response from the player, who is the host, in self-hosted mode
  FromHost {
    request: hosting::HostCommand<S, B>,
  },
  /// Requests that are tied to being in a particular realm. This mostly matters when a player is accessing a realm on another server as these requests will be proxied to the remote server without modification. When a player is first sent to a realm, it is told to download the assets. Once a player has all the assets, it should send any in-realm command to be actually moved into the realm. This will trigger the server to send the current realm state.
  InLocation {
    request: protocol::LocationRequest<S, B>,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientResponse<S: AsRef<str> + Eq + std::hash::Hash + Ord, B> {
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange {
    id: u32,
    result: UpdateResult,
  },
  Activity {
    id: u32,
    activity: Activity,
  },
  Administration {
    id: u32,
    response: administration::AdministrationResponse<S>,
  },
  /// The server announcements have changed
  Announcements {
    announcements: Vec<communication::Announcement<S>>,
  },
  /// Whether updating the announcements (either set or clear) was successful (true) or failed (false)
  AnnouncementUpdate {
    id: u32,
    result: UpdateResult,
  },
  /// The asset was created successfully.
  AssetCreationSucceeded {
    id: u32,
  },
  /// The asset could not be created
  AssetCreationFailed {
    id: u32,
    error: AssetError,
  },
  /// An asset the client requested is available.
  Asset {
    id: u32,
    asset: Asset<S, B>,
  },
  /// An asset requested by the client is corrupt or cannot be loaded due to an internal error
  AssetUnavailable {
    id: u32,
  },
  /// The player's current avatar
  AvatarCurrent {
    avatar: avatar::Avatar,
  },
  /// Whether updating the avatar was successful
  AvatarUpdate {
    id: u32,
    result: UpdateResult,
  },
  /// The book marks that are currently stored on the server
  Bookmarks {
    bookmarks: HashSet<Resource<S>>,
  },
  /// Whether updating the bookmark (add or remove) was successful (true) or failed (false)
  BookmarkUpdate {
    id: u32,
    success: bool,
  },
  /// The server is about to terminate the connection. This maybe in a response to a request or
  /// unsolicited.
  Disconnect,
  // The current calendar ID for a player
  Calendar {
    id: B,
  },
  CalendarLocations {
    locations: Vec<location::target::LocalTarget<S>>,
  },
  CalendarLocationChange {
    id: u32,
    success: bool,
  },
  CurrentAccessDirectMessage {
    rules: Vec<access::AccessControl<S, access::SimpleAccess>>,
    default: access::SimpleAccess,
  },
  CurrentAccessDefault {
    rules: Vec<access::AccessControl<S, access::Privilege>>,
    default: access::Privilege,
  },
  CurrentAccessOnline {
    rules: Vec<access::AccessControl<S, access::OnlineAccess>>,
    default: access::OnlineAccess,
  },
  DirectMessage {
    player: PlayerIdentifier<S>,
    message: communication::DirectMessage<S>,
  },
  /// The status of a direct message that was sent
  DirectMessageReceipt {
    id: u32,
    status: communication::DirectMessageStatus,
  },
  /// The last message receipt time for all players this player has received messages from
  DirectMessageStats {
    stats: DirectMessageStats<S>,
    last_login: DateTime<Utc>,
  },
  /// All the direct messages in a particular time frame requested
  DirectMessages {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    player: PlayerIdentifier<S>,
    messages: Vec<communication::DirectMessage<S>>,
  },
  /// Where the player is currently located
  LocationChange {
    location: location::change::LocationChangeResponse<S>,
  },

  /// No event, just a keep alive
  NoOperation,
  ToHost {
    event: hosting::HostEvent<S, B>,
  },

  /// Servers that are currently in contact with this server
  Peers {
    peers: Vec<S>,
  },
  /// Servers that are currently in banned on this server
  PeersBanned {
    bans: HashSet<access::BannedPeer<S>>,
  },
  /// Whether updating a banned server (add, delete, clear) was successful (true) or failed (false)
  PeersBannedUpdate {
    id: u32,
    result: UpdateResult,
  },
  /// The result of trying to reset a player's information
  PlayerReset {
    id: u32,
    result: UpdateResult,
  },
  /// All the public keys this player can use to log in with (instead of providing a username/password)
  PublicKeys {
    keys: BTreeMap<S, auth::PublicKey>,
  },
  /// Whether updating a public key (add, delete, clear) was successful (true) or failed (false)
  PublicKeyUpdate {
    id: u32,
    result: UpdateResult,
  },
  LocationVisibility {
    id: u32,
    result: UpdateResult,
  },
  /// The list of available realms for the filtering criteria provided
  LocationsAvailable {
    id: u32,
    locations: Vec<location::directory::DirectoryEntry<S>>,
  },
  LocationsUnavailable {
    id: u32,
    server: Option<S>,
  },
  /// Information on the whereabouts of a player
  PlayerOnlineState {
    id: u32,
    state: player::OnlineState<S>,
  },
  /// An event happened in a realm. If the player is accessing a realm on another server, these are proxied by the local server.
  InLocation {
    response: protocol::LocationResponse<S, B>,
  },
}

pub type DirectMessageStats<S> = HashMap<PlayerIdentifier<S>, DirectMessageInfo>;

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

impl Display for AssetError {
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

impl<S: AsRef<str> + Eq + std::hash::Hash + Ord + Serialize, B: Serialize> From<ClientRequest<S, B>> for Message {
  fn from(value: ClientRequest<S, B>) -> Self {
    Message::Binary(rmp_serde::to_vec_named(&value).expect("Failed to serialize message").into())
  }
}
impl<S: AsRef<str> + Eq + std::hash::Hash + Ord + Serialize, B: Serialize> From<ClientResponse<S, B>> for Message {
  fn from(value: ClientResponse<S, B>) -> Self {
    Message::Binary(rmp_serde::to_vec_named(&value).expect("Failed to serialize message").into())
  }
}
