use crate::database::CalendarCacheEntries;
use serde::Serialize;
use spadina_core::asset::Asset;
use spadina_core::avatar::Avatar;
use spadina_core::communication::{DirectMessageStatus, MessageBody};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::{Activity, DirectoryEntry, SearchCriteria};
use spadina_core::location::protocol::{LocationRequest, LocationResponse};
use spadina_core::location::target::{LocalTarget, UnresolvedTarget};
use spadina_core::location::Descriptor;
use spadina_core::player::OnlineState;
use std::hash::Hash;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum VisitorTarget<S: AsRef<str>> {
  Location { owner: S, descriptor: Descriptor<S> },
  Host { host: S },
}

/// Messages exchanged between servers; although there is a client/server relationship implied by Web Sockets, the connection is peer-to-peer, therefore, there is no distinction between requests and responses in this structure
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PeerMessage<S: AsRef<str> + Ord + Eq + Hash, B: AsRef<[u8]>> {
  /// Request assets from this server if they are available
  AssetRequest {
    id: u32,
    asset: S,
  },
  /// Send assets requested by the other server
  AssetResponseOk {
    id: u32,
    asset: Asset<S, B>,
  },
  AssetResponseMissing {
    id: u32,
  },
  /// Change the avatar of a player
  AvatarSet {
    avatar: Avatar,
    player: S,
  },
  CalendarRequest {
    id: u32,
    locations: Vec<LocalTarget<S>>,
    player: S,
  },
  CalendarResponse {
    id: u32,
    entries: CalendarCacheEntries<S>,
  },
  /// Transfer a direct message
  DirectMessage {
    id: u32,
    sender: S,
    recipient: S,
    body: MessageBody<S>,
  },
  DirectMessageResponse {
    id: u32,
    status: DirectMessageStatus,
  },
  HostActivityRequest {
    id: u32,
    player: S,
  },
  HostActivityResponse {
    id: u32,
    activity: Activity,
  },
  /// Result of attempt to join a host
  LocationChange {
    player: S,
    response: LocationChangeResponse<S>,
  },
  /// Process a realm-related request for a player that has been handed off to this server
  LocationRequest {
    player: S,
    request: LocationRequest<S, B>,
  },
  /// Receive a realm-related response for a player that has been handed off to this server
  LocationResponse {
    player: S,
    response: LocationResponse<S, B>,
  },
  /// List realms that are on this server
  LocationsList {
    id: u32,
    query: PeerLocationSearch<S>,
  },
  /// The realms that are available on this server
  LocationsAvailable {
    id: u32,
    locations: Vec<DirectoryEntry<S>>,
  },
  LocationsUnavailable {
    id: u32,
  },
  /// Check the online status of a player
  OnlineStatusRequest {
    id: u32,
    requester: S,
    target: S,
  },
  /// Send the online status of a player
  OnlineStatusResponse {
    id: u32,
    state: OnlineState<S>,
  },
  /// Releases control of a player to the originating server
  VisitorRelease {
    player: S,
    target: UnresolvedTarget<S>,
  },
  /// Send player to a realm on the destination server
  ///
  /// This transfers control of that player to the peer server until the originating server yanks them back or the destination server send them back
  VisitorSend {
    player: S,
    target: VisitorTarget<S>,
    avatar: Avatar,
  },
  /// Forces a player to be removed from a peer server by the originating server
  VisitorYank {
    player: S,
  },
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PeerLocationSearch<S: AsRef<str>> {
  Public,
  Search { query: SearchCriteria<String> },
  Specific { locations: Vec<LocalTarget<S>> },
}
impl<S: AsRef<str> + Ord + Eq + Hash + Serialize, B: AsRef<[u8]> + Serialize> From<PeerMessage<S, B>> for Message {
  fn from(value: PeerMessage<S, B>) -> Self {
    Message::Binary(rmp_serde::to_vec_named(&value).expect("Failed to serialize message").into())
  }
}
