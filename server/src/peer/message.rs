#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum VisitorTarget {
  Realm { owner: std::sync::Arc<str>, asset: std::sync::Arc<str> },
  Host { host: std::sync::Arc<str> },
}

/// Messages exchanged between servers; all though there is a client/server relationship implied by Web Sockets, the connection is peer-to-peer, therefore, there is no distinction between requests and responses in this structure
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PeerMessage<S: AsRef<str> + std::cmp::Ord + std::cmp::Eq + std::hash::Hash> {
  /// Request assets from this server if they are available
  AssetsPull {
    assets: Vec<S>,
  },
  /// Send assets requested by the other server
  AssetsPush {
    assets: std::collections::BTreeMap<S, Option<spadina_core::asset::Asset>>,
  },
  /// Change the avatar of a player
  AvatarSet {
    player: S,
    avatar: spadina_core::avatar::Avatar,
  },
  ConsensualEmoteRequestInitiate {
    player: S,
    emote: S,
    recipient: spadina_core::player::PlayerIdentifier<S>,
  },
  ConsensualEmoteRequestFromLocation {
    id: i32,
    player: S,
    emote: S,
    sender: spadina_core::player::PlayerIdentifier<S>,
  },
  ConsensualEmoteResponse {
    player: S,
    id: i32,
    ok: bool,
  },
  /// Transfer a direct message
  DirectMessage {
    id: i32,
    sender: S,
    recipient: S,
    body: spadina_core::communication::MessageBody<S>,
  },
  DirectMessageResponse {
    id: i32,
    status: spadina_core::communication::DirectMessageStatus,
  },
  FollowRequestInitiate {
    player: S,
    target: spadina_core::player::PlayerIdentifier<S>,
  },
  FollowRequestFromLocation {
    player: S,
    id: i32,
    source: spadina_core::player::PlayerIdentifier<S>,
  },
  FollowResponse {
    player: S,
    id: i32,
    ok: bool,
  },
  /// Make request in self-hosted connection to host
  GuestRequest {
    player: S,
    request: spadina_core::self_hosted::GuestRequest<S>,
  },
  /// Send response in self-hosted connection to guest
  GuestResponse {
    player: S,
    response: spadina_core::self_hosted::GuestResponse<S>,
  },
  /// Result of attempt to join a host
  LocationChange {
    player: S,
    response: spadina_core::location::LocationResponse<S>,
  },
  /// A collection of messages when a time range was queried
  LocationMessages {
    player: S,
    messages: Vec<spadina_core::location::LocationMessage<S>>,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// Retrieve older messages for a location
  LocationMessagesGet {
    player: S,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  },
  /// A message was posted in the current realm's chat.
  LocationMessagePosted {
    player: S,
    message: spadina_core::location::LocationMessage<S>,
  },
  /// Send a message to the group chat associated with the location we are currently logged into. If
  /// not in a location, the server should discard the message.
  LocationMessageSend {
    player: S,
    body: spadina_core::communication::MessageBody<S>,
  },

  /// Check the online status of a player
  OnlineStatusRequest {
    requester: S,
    target: S,
  },
  /// Send the online status of a player
  OnlineStatusResponse {
    requester: S,
    target: S,
    state: spadina_core::player::PlayerLocationState<S>,
  },
  /// Process a realm-related request for a player that has been handed off to this server
  RealmRequest {
    player: S,
    request: spadina_core::realm::RealmRequest<S>,
  },
  /// Receive a realm-related response for a player that has been handed off to this server
  RealmResponse {
    player: S,
    response: spadina_core::realm::RealmResponse<S>,
  },
  /// List realms that are on this server
  RealmsList {
    id: i32,
    source: PeerRealmSource<S>,
  },
  /// The realms that are available on this server
  RealmsAvailable {
    id: i32,
    available: Vec<spadina_core::realm::RealmDirectoryEntry<S>>,
  },
  /// Releases control of a player to the originating server
  VisitorRelease {
    player: S,
    target: Option<spadina_core::realm::RealmTarget<S>>,
  },
  /// Send player to a realm on the destination server
  ///
  /// This transfers control of that player to the peer server until the originating server yanks them back or the destination server send them back
  VisitorSend {
    capabilities: Vec<S>,
    player: S,
    target: VisitorTarget,
    avatar: spadina_core::avatar::Avatar,
  },
  /// Forces a player to be removed from a peer server by the originating server
  VisitorYank {
    player: S,
  },
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PeerRealmSource<S: AsRef<str>> {
  InDirectory,
  Specific { realms: Vec<spadina_core::realm::LocalRealmTarget<S>> },
}

impl<S: AsRef<str> + std::cmp::Ord + std::cmp::Eq + std::hash::Hash + serde::Serialize> spadina_core::net::ToWebMessage for PeerMessage<S> {}
