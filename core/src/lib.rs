use std::borrow::Borrow;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub mod asset;
pub mod asset_store;
pub mod avatar;
pub mod net;

/// The result from attempting to change an access control setting
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AccessChangeResponse {
  /// The access control has been changed
  Changed,
  /// The player does not have permissions to update this access setting
  Denied,
  /// There was an unexplained error updating the access control setting
  InternalError,
}

/// An access control rule
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AccessControl {
  /// Allow a particular player access; if a time is provided, this rule will expire after that time
  AllowPlayer(String, Option<chrono::DateTime<chrono::Utc>>),
  /// Deny a particular player access; if a time is provided, this rule will expire after that time
  DenyPlayer(String, Option<chrono::DateTime<chrono::Utc>>),
  /// Allow all users from a particular server access; if a time is provided, this rule will expire after that time
  AllowServer(String, Option<chrono::DateTime<chrono::Utc>>),
  /// Deny all users from a particular server access; if a time is provided, this rule will expire after that time
  DenyServer(String, Option<chrono::DateTime<chrono::Utc>>),
  /// Allow all users from this server; if a time is provided, this rule will expire after that time
  AllowLocal(Option<chrono::DateTime<chrono::Utc>>),
  /// Deny all users from this server; if a time is provided, this rule will expire after that time
  DenyLocal(Option<chrono::DateTime<chrono::Utc>>),
}
/// Access to allow if no access control rules apply
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AccessDefault {
  /// Allow all remaining players
  Allow,
  /// Deny all remaining players
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
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum AccessTarget {
  /// Access controls for a server.
  ///
  /// A player with access controls can may visit a realm on a server.
  AccessServer,
  /// Administrator controls for a server.
  ///
  /// A player with administrator controls can change access controls on any realm in a server or direct messaging on a server or change the server admin access.
  AdminServer,
  /// Sending direct messages to a user
  ///
  /// A player with direct messaging can send messages to a recipient player
  DirectMessagesUser,
  /// Sending direct messages to any user on this server
  ///
  /// A player with direct messaging can send messages to any player on a server
  DirectMessagesServer,
  /// Can another player determine if this player is online or not
  CheckOnline,
  /// When a new realm is created, set the access permissions for that realm to a copy of these
  NewRealmDefaultAccess,
  /// When a new realm is created, set the administration permissions for that realm to a copy of these
  NewRealmDefaultAdmin,
  /// Can another player determine what realm this player is in
  ///
  /// This permits the other player to be able to follow this player around
  ViewLocation,
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

/// An action the player wishes to perform in the realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Action {
  /// Perform a character animation in place pointing in a particular direction
  DirectedEmote { animation: String, at: Point, direction: Direction, duration: u32 },
  /// Perform a character animation in place
  Emote { animation: String, at: Point, duration: u32 },
  /// Interact with the item at the specified point
  Interaction {
    /// The location of the item; this must be adjacent to the player's location
    at: Point,
    /// The item that the player is interacting with at this location
    target: InteractionKey,
    /// The interaction to perform with this puzzle element
    interaction: InteractionType,
    /// A player may provide a sequence of actions to perform. If the interaction fails, this determines if the remaining sequence of actions should be kept or discarded.
    stop_on_failure: bool,
  },
  /// Move the player to the specified point; this point must be adjacent to their existing location
  Move(Point),
}
/// An announcement that should be visible to users
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Announcement {
  /// Just raw text
  Text(String),
  /// Text with a time. The time is relevant to the event but does *not* control when the text is displayed.
  Event(String, chrono::DateTime<chrono::Utc>),
  /// Text with a time range. The time is relevant to the event but does *not* control when the text is displayed.
  LimitedEvent(String, chrono::DateTime<chrono::Utc>, u32),
  /// Text with a link to a realm
  TextWithLocation { text: String, realm: AnnouncementRealmTarget },
  /// Text with a time and a link to a realm. The time is relevant to the event but does *not* control when the text is displayed.
  EventWithLocation { text: String, start: chrono::DateTime<chrono::Utc>, realm: AnnouncementRealmTarget },
  /// Text with a time range and a link to a realm. The time is relevant to the event but does *not* control when the text is displayed.
  LimitedEventWithLocation { text: String, start: chrono::DateTime<chrono::Utc>, length: u32, realm: AnnouncementRealmTarget },
}
/// A realm link for an announcement
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AnnouncementRealmTarget {
  /// The player's own copy of a realm identified by an asset
  Personal(String),
  /// A realm on this server
  Local(String),
  /// A realm on another server
  Remote { realm: String, server: String },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AssetError {
  ///The asset is not recognized by the server
  UnknownKind,
  /// The asset couldn't be decoded
  DecodeFailure,
  /// The data failed validation.
  Invalid,
  /// The data provided references unknown assets.
  Missing(Vec<String>),
  /// The user doesn't have the rights to upload assets to this server
  PermissionError,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthPublicKey<S: AsRef<str>> {
  pub name: S,
  pub nonce: S,
  pub signature: Vec<u8>,
}
/// Authentication mechanisms that the client and server can use
///
/// A client should perform a `GET` request on a server's `/auth` endpoint to get a JSON-encoded version of this struct detailing which authentication scheme to use
#[derive(Serialize, Deserialize, Debug)]
pub enum AuthScheme {
  /// Kerberos/GSSAPI authentication
  Kerberos,
  /// OpenIdConnect authentication using a remote server. The client should send a request to `/oidc?user=`_user_.
  OpenIdConnect,
  /// Simple username and password authentication. The client should send a JSON-serialised version of [PasswordRequest] to the `/password` endpoint
  Password,
}
/// The types of assets the server will store for the client
///
/// This do not necessarily map to the asset's real type. This is a more friendly version for tracking user preferences; in fact, players are not assets, though identified by short strings.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Copy)]
pub enum BookmarkType {
  Asset,
  ConsensualEmote,
  DirectedEmote,
  Emote,
  Player,
  Realm,
  RealmAsset,
  Server,
}
/// A way a character can be animated
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CharacterAnimation {
  /// Standing perfectly still
  Idle,
  /// Walking
  Walk,
  /// Climbing a ladder or vertical surface
  Climb,
  /// Jumping
  Jump,
  /// Server decided that the character was doing something invalid and they should be befuddled
  Confused,
  /// Touch an object in front of the character
  Touch,
  /// A custom character animation using the provided emote asset hash; if the client does not
  /// have this asset, it should request it and can substitute a standard animation as it sees
  /// fit.
  Custom(String),
}

/// The movement of a player's avatar to a new point using a particular animation
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CharacterMotion<T> {
  Leave {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    start: chrono::DateTime<chrono::Utc>,
  },
  Enter {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    end: chrono::DateTime<chrono::Utc>,
  },
  Internal {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation,
  },
  Interaction {
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation,
    interaction: InteractionType,
    target: InteractionKey,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  DirectedEmote {
    start: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation,
    direction: Direction,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteInitiator {
    start: chrono::DateTime<chrono::Utc>,
    animation: String,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteRecipient {
    start: chrono::DateTime<chrono::Utc>,
    animation: String,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientRequest {
  /// Request an account be locked or unlocked
  AccountLockChange {
    id: i32,
    name: String,
    locked: bool,
  },
  /// Check whether an account is locked or not
  AccountLockStatus {
    name: String,
  },
  /// Request the contents of a particular asset, using its hash ID
  AssetPull {
    id: String,
  },
  /// Uploads a new asset to the server. The client must generate a unique ID that the server
  /// will respond with
  AssetCreate {
    id: i32,
    asset_type: String,
    name: String,
    tags: Vec<String>,
    licence: asset::Licence,
    data: Vec<u8>,
  },
  /// Get the ACLs for a realm or communication.
  AccessGet {
    target: AccessTarget,
  },
  /// Set the ACLs for a realm or communication settings. The client must generate a unique ID
  /// that the server will respond with if the ACLs can be updated. If the realm is missing, the home realm is used.
  AccessSet {
    id: i32,
    target: AccessTarget,
    acls: Vec<AccessControl>,
    default: AccessDefault,
  },
  /// Adds an announcement to the global announcement list
  AnnouncementAdd(Announcement),
  /// Clears all global anouncements
  AnnnouncementClear,
  /// Gets the player's current avatar
  ///
  /// If the player has not ever created an avatar, a default will be provided.
  AvatarGet,
  /// Sets the player's current avatar, replacing any existing avatar
  AvatarSet(avatar::Avatar),
  /// Add a new asset to the book mark list; if this book mark is already there, no action is taken. The asset type of the bookmark is not checked against the bookmark type
  BookmarkAdd(BookmarkType, String),
  /// Retrieve all bookmarked assets of a particular type
  BookmarksGet(BookmarkType),
  /// Remove an asset from the book mark list. If not present, this is a no-op.
  BookmarkRemove(BookmarkType, String),
  /// Request a list of capabilities/extensions that the server supports. This should never change
  /// over the life of our connection, so we only need to ask it once.
  Capabilities,

  /// Retrieve direct messages between this player and another
  DirectMessageGet {
    player: String,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// Send a message to a player on this or any server.
  DirectMessageSend {
    id: i32,
    recipient: String,
    body: String,
  },
  /// Get a summary of when the last direct message received for this player
  DirectMessageStats,
  /// Create an invitation for a new player
  Invite {
    id: i32,
  },
  /// Tell the server that we no longer want to play. This should remove our avatar from the
  /// current realm and terminate the connection.
  Quit,
  /// Try to get the online status and location of another player
  PlayerCheck(String),
  /// Erases a player from the server
  ///
  /// If the player is currently active, they are kicked offline. All of their realms and chats will be deleted. This does *not* prevent them from logging in again. They must also be removed from the authentication provider.
  ///
  /// Only administrators can perform this command.
  PlayerDelete(String),

  /// Adds a new public key for this player to login with. If the name is already used, this key will replace it.
  PublicKeyAdd {
    name: String,
    der: Vec<u8>,
  },
  /// Removes a public key for this player to login with.
  PublicKeyDelete {
    name: String,
  },
  /// Removes all public keys for this player to login with
  PublicKeyDeleteAll,
  /// List the names of all the keys a player can log in with
  PublicKeyList,

  /// Request that we are moved to the entry point of a new realm.
  RealmChange {
    realm: RealmTarget,
  },
  /// Create a new realm
  RealmCreate {
    id: i32,
    name: String,
    asset: String,
    seed: Option<i32>,
  },
  RealmDelete {
    id: i32,
    target: String,
  },
  /// List the realms that we can access.
  RealmsList(RealmSource),
  /// List any servers that are in contact with the local server
  Servers,
  /// Removes servers from the banned list
  ServersClearBanned(Vec<String>),
  /// List servers that are banned
  ServersListBanned,
  /// Adds servers to the ban list
  ServersSetBanned(Vec<String>),
  /// Add a realm asset identifier to the train
  ///
  /// The asset will be check to see if it is train-compatible. If it is not, the request will be ignored.
  TrainAdd(String, bool),
  /// Requests that are tied to being in a particular realm. This mostly matters when a player is accessing a realm on another server as these requests will be proxied to the remote server without modification. When a player is first sent to a realm, it is told to download the assets. Once a player has all the assets, it should send any in-realm command to be actually moved into the realm. This will trigger the server to send the current realm state.
  InRealm(RealmRequest),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientResponse {
  /// Result of trying to lock an account
  AccountLockChange { id: i32, name: String, success: bool },
  /// The current status of an account
  AccountLockStatus { name: String, status: AccountLockState },
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange { id: i32, response: AccessChangeResponse },
  /// The server announcements have changed
  Announcements(Vec<Announcement>),
  /// The asset was created successfully. The hash for the asset is provided.
  AssetCreationSucceeded { id: i32, hash: String },
  /// The asset could not be created
  AssetCreationFailed { id: i32, error: AssetError },
  /// An asset the client requested is available.
  Asset(String, asset::Asset),
  /// An asset requested by the client is corrupt or cannot be loaded due to an internal error
  AssetUnavailable(String),
  /// The player's current avatar
  AvatarCurrent(avatar::Avatar),
  /// The book marks of the particular type that are currently stored on the server
  Bookmarks(BookmarkType, Vec<String>),
  /// The server is about to terminate the connection. This maybe in a response to a request or
  /// unsolicited.
  Disconnect,
  /// The list of capabilities this servers supports. This is in response to a request.
  Capabilities { server_capabilities: Vec<String> },

  /// An unsolicited message that the client will need the provided assets. If the client does
  /// not have them locally, it should pull them.
  CheckAssets { asset: Vec<String> },
  /// The current ACLs associated with a realm
  CurrentAccess { target: AccessTarget, acls: Vec<AccessControl>, default: AccessDefault },
  /// A direct message was received
  DirectMessageReceived { sender: String, body: String, timestamp: chrono::DateTime<chrono::Utc> },
  /// The status of a direct message that was sent
  DirectMessageReceipt { id: i32, status: DirectMessageStatus },
  /// The last message receipt time for all players this player has received messages from
  DirectMessageStats { stats: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>, last_login: chrono::DateTime<chrono::Utc> },
  /// All the direct messages in a particular time frame requested
  DirectMessages { player: String, messages: Vec<DirectMessage> },

  /// The player is in transit between realms. While in transit, allowed to request changing to
  /// another realm and the server will send messages about the assets it requires. The player
  /// should be animated as warping out from the last known position. The server may send send a
  /// RealmChanged if there is a realm that is targeted..
  InTransit,
  /// The invitation created for a new player
  Invite { id: i32, url: Option<String> },
  /// All the public keys this player can use to log in with (instead of providing a username/password)
  PublicKeys(Vec<String>),

  /// The user has been changed to a new realm or been denied access. If the player is not in the realm and visible to other players until a realm command is sent. This allows the client time to download any required assets.
  RealmChanged(RealmChange),
  /// The result from creating a realm
  RealmCreation { id: i32, status: RealmCreationStatus },
  /// The result for deleting a realm
  RealmDeletion { id: i32, ok: bool },
  /// The list of available realms for the filtering criteria provided
  RealmsAvailable { display: RealmSource, realms: Vec<Realm> },
  /// Servers that are currently in contact with this server
  Servers(Vec<String>),
  /// Servers that are currently in banned on this server
  ServersBanned(Vec<String>),
  /// Information on the whereabouts of a player
  PlayerState { player: String, state: PlayerLocationState },
  /// An event happened in a realm. If the player is accessing a realm on another server, these are proxied by the local server.
  InRealm(RealmResponse),
}

/// Information about direct messages between this player and another
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectMessage {
  /// Whether the direction was to the player or from the player
  pub inbound: bool,
  /// The contents of the message
  pub body: String,
  /// The time the message was sent
  pub timestamp: chrono::DateTime<chrono::Utc>,
}
/// The status of a direct message after sending
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DirectMessageStatus {
  /// The message was written to user's inbox
  Delivered,
  /// The recipient is invalid
  UnknownRecipient,
  /// The message was placed in a queue to send to a remote server. More delivery information may follow.
  Queued,
  /// The player isn't allowed to send direct messages yet
  Forbidden,
  /// An error occurred on the server while sending the message
  InternalError,
}
/// Compass directions
#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
pub enum Direction {
  N,
  NE,
  E,
  SE,
  S,
  SW,
  W,
  NW,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum InteractionKey {
  Button(String),
  Switch(String),
  RadioButton(String),
  RealmSelector(String),
}

/// The result of a player interacting with an element in a realm
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum InteractionResult {
  /// Interaction is not one allowed by the game
  Invalid,
  /// Interaction was done not supported at the current time (e.g., the control is disabled, or lacks power)
  Failed,
  /// The Interaction was successful
  Accepted,
}
/// An interaction between a player and an element in a realm
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum InteractionType {
  /// The user clicked/tapped
  Click,
  /// The user selected a realm
  Realm(RealmTarget),
}
/// The information provided by the server to do OpenID Connect authentication
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenIdConnectInformation {
  /// The URL the user should be directed to in order to complete authentication
  pub authorization_url: String,
  /// A token that the client should use to pick up the JWT once authentication is complete
  pub request_id: String,
}
/// The data structure for performing a password-authenticated request
#[derive(Serialize, Deserialize, Debug)]
pub struct PasswordRequest<T: AsRef<str>> {
  /// The player's login name
  pub username: T,
  /// The player's raw password; it is the client's responsibility to ensure the channel is encrypted or warn the player
  pub password: T,
}
/// When querying the online status and location of another player, this is the response
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PlayerLocationState {
  /// Player state is not visible
  Unknown,
  /// Not a valid player
  Invalid,
  /// Offline
  Offline,
  /// Online, but location is unavailable
  Online,
  /// Online and not in a realm
  InTransit,
  /// Online and in a particular realm
  Realm(String, String),
  /// The server is not yet contacted; a second response may follow
  ServerDown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerState {
  pub avatar: avatar::Avatar,
  pub effect: avatar::Effect,
  pub motion: Vec<CharacterMotion<Point>>,
}

/// A collection of player movements
pub type PlayerStates = std::collections::HashMap<String, PlayerState>;

/// A point in 3D space
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Point {
  pub platform: u32,
  pub x: u32,
  pub y: u32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub enum PropertyKey {
  BoolSink(String),
  EventSink(String),
  NumSink(String),
}

/// The state of a value associated with a puzzle piece/asset
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum PropertyValue {
  Num(u32),
  Bool(bool),
  Ticks(Vec<chrono::DateTime<chrono::Utc>>),
}

/// A collection of labelled property values
pub type PropertyStates = std::collections::HashMap<PropertyKey, PropertyValue>;

/// These are the instructions that can be given to puzzle elements to change their state
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum PuzzleCommand {
  /// Remove or reset state
  Clear,
  /// Lock the state and make it unable to be updated by players
  Disable,
  /// Decrement the state
  Down,
  /// Unlock the state and make updatable by players
  Enable,
  /// Set the frequency of a timer
  Frequency,
  /// Add additional state
  Insert,
  /// Transport players
  Send,
  /// Change the state to a provided value
  Set,
  /// Change the "left" state when multiple states are present in the element
  SetLeft,
  /// Change the "right" state when multiple states are present in the element
  SetRight,
  /// Invert the current state
  Toggle,
  /// Increment the state
  Up,
}
/// These are the events that
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PuzzleEvent {
  /// The state is at its maximum value
  AtMax,
  /// The state is at its minimum value
  AtMin,
  /// The state has changed
  Changed,
  /// The state has reset or rolled over
  Cleared,
  /// The value currently associated with a piece that can hold many values has changed
  Selected,
  /// Whether the piece is accepting user input
  Sensitive,
}

/// A realm the player can access
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Realm {
  /// The hash ID of the realm
  pub id: String,
  /// The friendly name for this realm
  pub name: String,
  /// How busy the realm is
  pub activity: RealmActivity,
  /// The last access time of this realm
  pub accessed: Option<chrono::DateTime<chrono::Utc>>,
  /// The server that hosts this realm (or none of the local server)
  pub server: Option<String>,
  /// If the realm is part of a train, then the position in the train
  pub train: Option<u16>,
}
/// How much activity is there in a realm
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum RealmActivity {
  /// Activity information not available
  Unknown,
  /// No players in recent time
  Deserted,
  /// Some players with low chat volume
  Quiet,
  /// Some players with moderate chat volume
  Popular,
  /// Lots of players with moderate chat volume
  Busy,
  /// Lots of players with high chat volume
  Crowded,
}

/// The access control type for a realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmAccessTarget {
  /// Access for the current realm
  Access,
  /// Administrator for the current realm
  Admin,
}
/// The server's indication that the player's realm has changed.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmChange {
  /// The realm change was successful. This has the data required for the client to download the realm's assets.
  Success {
    /// The ID of the realm
    realm: String,
    /// The server that hosts the realm
    server: String,
    /// The friendly name for the realm
    name: String,
    /// The root asset for the realm
    asset: String,
    /// Some elements in the realm and randomised and this serves as the seed for those random choices. It is provided here, so players see a consistent version of those choices even though the selection is done client-side.
    seed: i32,
    /// Admin controllable parameters for this realm
    settings: RealmSettings,
    /// The service capabilities required by the realm
    capabilities: Vec<String>,
  },
  /// The realm that the player requested was not available or the player is not allowed to access it
  Denied,
}
/// The result of attempting to create a realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmCreationStatus {
  /// The realm was created and the ID of the realm is provided. If the assets are not present on the server, the server may stall when linking to the realm until the assets are available. Creating a realm with a bad asset may stall forever.
  Created(String),
  /// A realm with that asset already exists
  Duplicate,
  /// The realm has exceeded a server-defined limit for how many realms a player may have
  TooManyRealms,
  /// A server error occurred trying to create the realm
  InternalError,
}
/// A message to all players in a realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RealmMessage {
  /// The contents of the message
  pub body: String,
  /// The principal of the player that sent the message
  pub sender: String,
  /// The time the message was sent
  pub timestamp: chrono::DateTime<chrono::Utc>,
}
/// A property that can be set by realm administrators (rather than by puzzles)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmSetting {
  Announcement(Vec<Announcement>),
  AudioStream(String),
  Bool(bool),
  Color(asset::Color),
  Intensity(f64),
  Num(u32),
  Realm(RealmSettingLink),
}
pub type RealmSettings = std::collections::BTreeMap<String, RealmSetting>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RealmSettingLink {
  Global(String, String),
  Owner(String),
  Home,
}

/// A request from the player to the realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmRequest {
  /// Change the name or directory listing status of a realm. The user must have admin rights on this realm. If either of these is optional, it is not modified.
  ChangeName(Option<String>, Option<bool>),
  /// Change a setting associated with the realm
  ChangeSetting(String, RealmSetting),
  ConsensualEmoteRequest {
    emote: String,
    player: String,
  },
  ConsensualEmoteResponse {
    id: i32,
    ok: bool,
  },
  FollowRequest {
    player: String,
  },
  FollowResponse {
    id: i32,
    ok: bool,
  },
  /// Get the ACLs for a realm or communication.
  GetAccess {
    target: RealmAccessTarget,
  },
  /// Kick a player out of the current realm. Requires admin privileges on the realm. It doesn't prevent them from rejoining; if that is desired, modify the access control.
  Kick(String),
  /// Do nothing; When the server has authenticated a player to enter a realm, they will be in a limbo where the client is allowed to get assets it requires without the player being in the visible to other players into the realm. The first operation the player performs will trigger their entry into the realm. This method is provided as a convience to do this.
  NoOperation,

  /// Request that we move our avatar to the new location specified. All units are in absolute
  /// coordinates from the origin of a realm in 10cm increments. The server is not obliged to
  /// move us to these coordinates; it will make a judgement and send information to the client.
  Perform(Vec<Action>),
  /// Send a message to the group chat associated with the realm we are currently logged into. If
  /// none, the server should discard the message.
  SendMessage(String),
  /// Retrieve older messages for a realm
  GetMessages {
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// Set the ACLs for a realm or communication settings. The client must generate a unique ID
  /// that the server will respond with if the ACLs can be updated. If the realm is missing, the home realm is used.
  SetAccess {
    id: i32,
    target: RealmAccessTarget,
    acls: Vec<AccessControl>,
    default: AccessDefault,
  },
}
/// A message from the server about the current realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmResponse {
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange {
    id: i32,
    response: AccessChangeResponse,
  },
  /// The current ACLs associated with a realm
  AccessCurrent {
    target: RealmAccessTarget,
    acls: Vec<AccessControl>,
    default: AccessDefault,
  },
  ConsensualEmoteRequest {
    id: i32,
    emote: String,
    player: String,
  },
  FollowRequest {
    id: i32,
    player: String,
  },
  /// A message was posted in the current realm's chat.
  MessagePosted {
    sender: String,
    body: String,
    timestamp: chrono::DateTime<chrono::Utc>,
  },
  /// A collection of messages when a time range was queried
  Messages(Vec<RealmMessage>),
  /// The realm's name and/or directory listing status has been changed
  NameChanged(String, bool),
  /// A setting has been changed
  SettingChanged(String, RealmSetting),
  UpdateState {
    time: chrono::DateTime<chrono::Utc>,
    player: PlayerStates,
    state: PropertyStates,
  },
}
/// When fetching realms from the server, what kind of realms to fetch
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RealmSource {
  /// Realms owned by the user
  Personal,
  /// Realms in the player's bookmark list
  Bookmarks,
  /// Realms marked as public on the local server
  LocalServer,
  /// Public realms on a remote server
  RemoteServer(String),
  /// Check for a specific realm by identifier
  Manual(RealmTarget),
}
/// The realm that has been selected
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum RealmTarget {
  Home,
  LocalRealm(String),
  PersonalRealm(String),
  RemoteRealm { realm: String, server: String },
}
/// Value for an asset
///
/// In each asset, there are parameters (colour, texture, animation) and these can be set based on action in the puzzle logic of the game
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ValueSelector<T> {
  /// The value is fixed to a single static value
  Fixed {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    value: T,
  },
  /// The value is selected based on a seed value for each realm. The realm will provide the seed value and the choice should be `possibilities[seed % possibilities.len()]`
  Random {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    possibilities: Vec<T>,
  },
  /// The value is selected based on a Boolean value emitted by the puzzle's logic
  BooleanPuzzle {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    when_false: T,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    when_true: T,
    piece_name: String,
  },
  /// The value is selected based on an integer value emitted by the puzzle's logic
  NumPuzzle {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    values: std::collections::HashMap<u32, T>,
    piece_name: String,
  },
}

pub const CAPABILITIES: &[&str] = &["base"];

/// Check if an access control rule applies
///
/// * `acls` - the access control list to use
/// * `user` - the player being checked
/// * `server` - the server the player is from or `None` for the local server
/// * `local_server` - the name of the local server
pub fn check_acls<S: Borrow<str>, A: Borrow<AccessControl> + Sized, I: Iterator<Item = A>>(
  acls: I,
  user: &str,
  server: &Option<S>,
  local_server: &str,
) -> Option<bool> {
  fn time_ok(time: &Option<chrono::DateTime<chrono::Utc>>) -> bool {
    time.map(|t| t > chrono::Utc::now()).unwrap_or(true)
  }
  fn player_match<S: Borrow<str>>(player: &str, user: &str, server: &Option<S>, local_server: &str) -> bool {
    player == user || player == format!("{}@{}", user, server.as_ref().map(|s| s.borrow()).unwrap_or(local_server))
  }
  acls
    .filter_map(|a| match a.borrow() {
      AccessControl::AllowPlayer(player, time) => {
        if time_ok(time) && player_match(player, user, server, local_server) {
          Some(true)
        } else {
          None
        }
      }
      AccessControl::DenyPlayer(player, time) => {
        if time_ok(time) && player_match(player, user, server, local_server) {
          Some(false)
        } else {
          None
        }
      }
      AccessControl::AllowServer(server_id, time) => {
        if time_ok(time) && server_id == server.as_ref().map(|s| s.borrow()).unwrap_or(local_server) {
          Some(true)
        } else {
          None
        }
      }
      AccessControl::DenyServer(server_id, time) => {
        if time_ok(time) && server_id == server.as_ref().map(|s| s.borrow()).unwrap_or(local_server) {
          Some(false)
        } else {
          None
        }
      }
      AccessControl::AllowLocal(time) => {
        if time_ok(time) && server.as_ref().map(|s| s.borrow()).unwrap_or(local_server) == local_server {
          Some(true)
        } else {
          None
        }
      }
      AccessControl::DenyLocal(time) => {
        if time_ok(time) && server.as_ref().map(|s| s.borrow()).unwrap_or(local_server) == local_server {
          Some(false)
        } else {
          None
        }
      }
    })
    .next()
}
impl AccessDefault {
  /// Check if an access control rule applies
  ///
  /// * `acls` - the access control list to use
  /// * `user` - the player being checked
  /// * `server` - the server the player is from or `None` for the local server
  /// * `local_server` - the name of the local server
  pub fn check<S: Borrow<str>, A: Borrow<AccessControl> + Sized, I: Iterator<Item = A>>(
    &self,
    acls: I,
    user: &str,
    server: &Option<S>,
    local_server: &str,
  ) -> bool {
    check_acls(acls, user, server, local_server).unwrap_or(match self {
      AccessDefault::Allow => true,
      AccessDefault::Deny => false,
    })
  }
}
impl Action {
  /// The location where an action occurs
  pub fn position(&self) -> &Point {
    match self {
      Action::DirectedEmote { at, .. } => at,
      Action::Emote { at, .. } => at,
      Action::Interaction { at, .. } => at,
      Action::Move(point) => point,
    }
  }
}
impl<T> CharacterMotion<T>
where
  T: DeserializeOwned + Serialize,
{
  /// Create a new character motion in a different coordinate system
  pub fn map<R: DeserializeOwned + Serialize, F: Fn(&T) -> R>(&self, func: F) -> CharacterMotion<R> {
    match self {
      CharacterMotion::Internal { from, to, start, end, animation } => {
        CharacterMotion::Internal { from: func(from), to: func(to), start: *start, end: *end, animation: animation.clone() }
      }

      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from: func(from), start: *start },
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to: func(to), end: *end },
      CharacterMotion::Interaction { animation, start, end, interaction, at, target } => CharacterMotion::Interaction {
        animation: animation.clone(),
        start: *start,
        end: *end,
        interaction: interaction.clone(),
        at: func(at),
        target: target.clone(),
      },
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start: start.clone(), animation: animation.clone(), direction: direction.clone(), at: func(at) }
      }
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start: start.clone(), animation: animation.clone(), at: func(at) }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start: start.clone(), animation: animation.clone(), at: func(at) }
      }
    }
  }
  pub fn time(&self) -> &chrono::DateTime<chrono::Utc> {
    match &self {
      CharacterMotion::Internal { end, .. } => end,
      CharacterMotion::Leave { start, .. } => start,
      CharacterMotion::Enter { end, .. } => end,
      CharacterMotion::Interaction { end, .. } => end,
      CharacterMotion::DirectedEmote { start, .. } => start,
      CharacterMotion::ConsensualEmoteInitiator { start, .. } => start,
      CharacterMotion::ConsensualEmoteRecipient { start, .. } => start,
    }
  }
  pub fn end_position(&self) -> Option<&T> {
    match &self {
      CharacterMotion::Internal { to, .. } => Some(to),
      CharacterMotion::Leave { .. } => None,
      CharacterMotion::Enter { to, .. } => Some(to),
      CharacterMotion::Interaction { at, .. } => Some(at),
      CharacterMotion::DirectedEmote { at, .. } => Some(at),
      CharacterMotion::ConsensualEmoteInitiator { at, .. } => Some(at),
      CharacterMotion::ConsensualEmoteRecipient { at, .. } => Some(at),
    }
  }
}
impl RealmSetting {
  /// Ensure any data in the request looks like valid data
  pub fn clean(self) -> Option<Self> {
    match self {
      RealmSetting::Realm(target) => Some(RealmSetting::Realm(target.clean()?)),
      RealmSetting::AudioStream(stream) => {
        if url::Url::parse(&stream).is_ok() {
          Some(RealmSetting::AudioStream(stream))
        } else {
          None
        }
      }
      x => Some(x),
    }
  }
  /// Update a setting from a value, but only if it's the correct type
  pub fn type_matched_update(&mut self, other: &RealmSetting) -> bool {
    match self {
      RealmSetting::Announcement(messages) => {
        if let RealmSetting::Announcement(new_messages) = other {
          messages.clear();
          messages.extend(new_messages.iter().cloned());
          true
        } else {
          false
        }
      }
      RealmSetting::AudioStream(url) => {
        if let RealmSetting::AudioStream(new_url) = other {
          url.clear();
          url.push_str(new_url);
          true
        } else {
          false
        }
      }
      RealmSetting::Color(value) => {
        if let RealmSetting::Color(new_value) = other {
          *value = *new_value;
          true
        } else {
          false
        }
      }
      RealmSetting::Bool(value) => {
        if let RealmSetting::Bool(new_value) = other {
          *value = *new_value;
          true
        } else {
          false
        }
      }
      RealmSetting::Intensity(value) => {
        if let RealmSetting::Intensity(new_value) = other {
          if (0.0_f64..=1.0_f64).contains(new_value) {
            *value = *new_value;
            true
          } else {
            false
          }
        } else {
          false
        }
      }
      RealmSetting::Num(value) => {
        if let RealmSetting::Num(new_value) = other {
          *value = *new_value;
          true
        } else {
          false
        }
      }
      RealmSetting::Realm(target) => {
        if let RealmSetting::Realm(new_target) = other {
          *target = new_target.clone();
          true
        } else {
          false
        }
      }
    }
  }
  pub fn type_name(&self) -> &'static str {
    match self {
      RealmSetting::Announcement(_) => "announcement",
      RealmSetting::AudioStream(_) => "audio stream",
      RealmSetting::Bool(_) => "Boolean value",
      RealmSetting::Color(_) => "color",
      RealmSetting::Intensity(_) => "intensity",
      RealmSetting::Num(_) => "numeric value",
      RealmSetting::Realm { .. } => "realm",
    }
  }
}
impl RealmSettingLink {
  pub fn clean(self) -> Option<Self> {
    match self {
      RealmSettingLink::Home => Some(RealmSettingLink::Home),
      RealmSettingLink::Owner(id) => {
        if id.chars().all(|c| c.is_alphanumeric()) {
          Some(RealmSettingLink::Owner(id))
        } else {
          None
        }
      }
      RealmSettingLink::Global(realm, server) => match (parse_server_name(&server), realm.chars().all(|c| c.is_alphanumeric())) {
        (Some(server), true) => Some(RealmSettingLink::Global(realm, server)),
        _ => None,
      },
    }
  }
}
impl RealmTarget {
  pub fn new<T>(realm: impl Into<String>, server: Option<impl Into<String>>) -> Self {
    match server {
      None => RealmTarget::LocalRealm(realm.into()),
      Some(server) => RealmTarget::RemoteRealm { realm: realm.into(), server: server.into() },
    }
  }

  pub fn to_url(&self) -> String {
    match self {
      RealmTarget::PersonalRealm(asset) => format!("puzzleverse:~{}", asset),
      RealmTarget::LocalRealm(id) => format!("puzzleverse:///{}", id),
      RealmTarget::RemoteRealm { realm, server } => format!("puzzleverse://{}/{}", server, realm),
      RealmTarget::Home => "puzzleverse:~".to_string(),
    }
  }
}
pub enum RealmTargetParseError {
  BadHost,
  BadPath,
  BadSchema,
  UrlError(url::ParseError),
}
impl std::str::FromStr for RealmTarget {
  type Err = RealmTargetParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match url::Url::parse(s) {
      Ok(url) => {
        if url.scheme() == "puzzleverse" {
          if url.cannot_be_a_base() {
            if url.path() == "~" {
              Ok(RealmTarget::Home)
            } else if url.path().starts_with("~") {
              Ok(RealmTarget::PersonalRealm(url.path()[1..].to_string()))
            } else {
              Err(RealmTargetParseError::BadPath)
            }
          } else if let Some(path_segments) = url.path_segments().map(|s| s.collect::<Vec<_>>()) {
            if let [path] = path_segments.as_slice() {
              match url.host() {
                None => Ok(RealmTarget::LocalRealm(path.to_string())),
                Some(url::Host::Domain(host)) => match parse_server_name(host) {
                  Some(host) => Ok(RealmTarget::RemoteRealm { realm: path.to_string(), server: host }),
                  None => Err(RealmTargetParseError::BadHost),
                },
                _ => Err(RealmTargetParseError::BadHost),
              }
            } else {
              Err(RealmTargetParseError::BadPath)
            }
          } else {
            Err(RealmTargetParseError::BadPath)
          }
        } else {
          Err(RealmTargetParseError::BadSchema)
        }
      }
      Err(e) => Err(RealmTargetParseError::UrlError(e)),
    }
  }
}
impl From<Direction> for u32 {
  fn from(direction: Direction) -> u32 {
    match direction {
      Direction::N => 0,
      Direction::NE => 1,
      Direction::E => 2,
      Direction::SE => 3,
      Direction::S => 4,
      Direction::SW => 5,
      Direction::W => 6,
      Direction::NW => 7,
    }
  }
}
impl From<u32> for Direction {
  fn from(direction: u32) -> Direction {
    match (direction % 8) + 8 % 8 {
      0 => Direction::N,
      1 => Direction::NE,
      2 => Direction::E,
      3 => Direction::SE,
      4 => Direction::S,
      5 => Direction::SW,
      6 => Direction::W,
      7 => Direction::NW,
      _ => panic!("bug in modular arithmetic for direction conversion"),
    }
  }
}

impl std::ops::Add for Direction {
  type Output = Self;

  fn add(self, other: Self) -> Self {
    let x: u32 = self.into();
    let y: u32 = other.into();
    (x + y).into()
  }
}
impl std::ops::Sub for Direction {
  type Output = Self;

  fn sub(self, other: Self) -> Self {
    let x: u32 = self.into();
    let y: u32 = other.into();
    (x - y).into()
  }
}

/// The result of parsing a player identifier
#[derive(Clone, Eq, PartialEq)]
pub enum PlayerIdentifier {
  /// The player is an name for a player on the local server
  Local(String),
  /// The player is a name for a player on the remote server
  Remote {
    server: String,
    player: String,
  },
  Bad,
}

impl PlayerIdentifier {
  /// Parse a player name
  pub fn new(player_name: &str, local_server_name: Option<&str>) -> Self {
    lazy_static::lazy_static! {
      static ref PLAYER_NAME: regex::Regex = regex::Regex::new(r"^\p{Alphabetic}(\p{Alphabetic}\p{N}-_)*$").unwrap();
    }
    let parts: Vec<_> = player_name.rsplitn(2, '@').collect();
    match parts[..] {
      [name] => {
        if PLAYER_NAME.is_match(name) {
          PlayerIdentifier::Local(name.to_string())
        } else {
          PlayerIdentifier::Bad
        }
      }
      [name, server_name_raw] => {
        if !PLAYER_NAME.is_match(name) {
          PlayerIdentifier::Bad
        } else {
          match idna::domain_to_unicode(server_name_raw) {
            (server_name, Ok(())) => {
              if local_server_name == Some(&server_name) {
                PlayerIdentifier::Local(name.to_string())
              } else {
                PlayerIdentifier::Remote { server: server_name, player: name.to_string() }
              }
            }
            _ => PlayerIdentifier::Bad,
          }
        }
      }
      _ => PlayerIdentifier::Bad,
    }
  }
}
pub fn abs_difference<T: std::ops::Sub<Output = T> + Ord>(x: T, y: T) -> T {
  if x < y {
    y - x
  } else {
    x - y
  }
}

/// Parse and normalize a server name
pub fn parse_server_name(server_name: &str) -> Option<String> {
  match idna::domain_to_unicode(server_name) {
    (name, Ok(())) => Some(name),
    _ => None,
  }
}
