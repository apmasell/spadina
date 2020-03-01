use serde::Deserialize;
use serde::Serialize;
/// The full address of a realm
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct AbsoluteRealmTarget<S: AsRef<str>> {
  pub asset: S,
  pub owner: S,
  pub server: S,
}

/// An action the player wishes to perform in the realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Action<S: AsRef<str>> {
  /// Perform a character animation in place
  Emote {
    animation: CharacterAnimation<S>,
    duration: u32,
  },
  /// Interact with the item at the specified point
  Interaction {
    /// The item that the player is interacting with at this location
    target: InteractionKey<S>,
    /// The interaction to perform with this puzzle element
    interaction: InteractionType<S>,
  },
  Move {
    length: u16,
  },
  Rotate {
    direction: Direction,
  },
}
/// Direction a character can face
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum Direction {
  YPos,
  YNeg,
  XPos,
  XNeg,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum InteractionKey<S: AsRef<str>> {
  Button(S),
  Switch(S),
  RadioButton(S),
  RealmSelector(S),
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
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum InteractionType<S: AsRef<str>> {
  /// The user clicked/tapped
  Click,
  /// The user selected a realm
  Realm(AbsoluteRealmTarget<S>),
}

/// A way a character can be animated
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CharacterAnimation<S: AsRef<str>> {
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
  Custom(S),
}

/// The movement of a player's avatar to a new point using a particular animation
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum CharacterMotion<T, S: AsRef<str>> {
  DirectedEmote {
    start: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation<S>,
    direction: Direction,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteInitiator {
    start: chrono::DateTime<chrono::Utc>,
    animation: S,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteRecipient {
    start: chrono::DateTime<chrono::Utc>,
    animation: S,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  Enter {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    end: chrono::DateTime<chrono::Utc>,
  },
  Interaction {
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation<S>,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  Leave {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    start: chrono::DateTime<chrono::Utc>,
  },
  Move {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    animation: CharacterAnimation<S>,
  },
  Rotate {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    direction: Direction,
  },
}
/// The address of a realm without the server
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct LocalRealmTarget<S: AsRef<str>> {
  pub asset: S,
  pub owner: S,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerState<S: AsRef<str>> {
  pub effect: crate::avatar::Effect,
  pub final_direction: Direction,
  pub final_position: Point,
  pub motion: Vec<CharacterMotion<Point, S>>,
}

/// A collection of player movements
pub type PlayerStates<S> = std::collections::HashMap<crate::player::PlayerIdentifier<S>, PlayerState<S>>;

/// A point in 3D space
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Point {
  pub platform: u32,
  pub x: u32,
  pub y: u32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
pub enum PropertyKey<S: AsRef<str>> {
  BoolSink(S),
  EventSink(S),
  NumSink(S),
}

/// The state of a value associated with a puzzle piece/asset
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum PropertyValue {
  Num(u32),
  Bool(bool),
  Ticks(Vec<chrono::DateTime<chrono::Utc>>),
}

/// A request from the player to the realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmRequest<S: AsRef<str>> {
  /// Get the ACLs for a realm or communication.
  AccessGet { target: RealmAccessTarget },
  /// Set the ACLs for a realm or communication settings. The client must generate a unique ID
  /// that the server will respond with if the ACLs can be updated. If the realm is missing, the home realm is used.
  AccessSet {
    id: i32,
    target: RealmAccessTarget,
    rules: Vec<crate::access::AccessControl<crate::access::SimpleAccess>>,
    default: crate::access::SimpleAccess,
  },
  /// Adds an announcement to the realm announcement list
  AnnouncementAdd { id: i32, announcement: RealmAnnouncement<S> },
  /// Clears all realm anouncements
  AnnouncementClear { id: i32 },
  /// Fetches the realm announcements list (though they are sent unsolicted upon change)
  AnnouncementList,
  /// Change the name or directory listing status of a realm. The user must have admin rights on this realm. If either of these is optional, it is not modified.
  ChangeName { id: i32, name: Option<S>, in_directory: Option<bool> },
  /// Change a setting associated with the realm
  ChangeSetting { id: i32, name: S, value: RealmSetting<S> },
  /// Destroys the realm
  Delete,
  /// Kick a player out of the current realm. Requires admin privileges on the realm. It doesn't prevent them from rejoining; if that is desired, modify the access control.
  Kick { id: i32, target: crate::player::PlayerIdentifier<S> },
  /// Do nothing; When the server has authenticated a player to enter a realm, they will be in a limbo where the client is allowed to get assets it requires without the player being in the visible to other players into the realm. The first operation the player performs will trigger their entry into the realm. This method is provided as a convience to do this.
  NoOperation,
  /// Request that we move our avatar to the new location specified. All units are in absolute
  /// coordinates from the origin of a realm in 10cm increments. The server is not obliged to
  /// move us to these coordinates; it will make a judgement and send information to the client.
  Perform(Vec<Action<S>>),
}
/// A collection of labelled property values
pub type PropertyStates<S> = std::collections::HashMap<PropertyKey<S>, PropertyValue>;

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

/// When fetching realms from the server, what kind of realms to fetch
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RealmSource<S: AsRef<str>> {
  /// Realms owned by the user
  Personal,
  /// Realms in the player's bookmark list
  Bookmarks,
  /// Realms marked as public on the local server
  LocalServer,
  /// Public realms on a remote server
  RemoteServer(S),
  /// Check for a specific realm by identifier
  Manual(RealmTarget<S>),
}

/// A realm the player can access
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RealmDirectoryEntry<S: AsRef<str>> {
  /// The asset ID of the realm
  pub asset: S,
  /// The friendly name for this realm
  pub name: S,
  /// How busy the realm is
  pub activity: RealmActivity,
  /// The player that owns this realm
  pub owner: S,
  /// The server that hosts this realm (or none of the local server)
  pub server: S,
  /// If the realm is part of a train, then the position in the train
  pub train: Option<u16>,
}
/// A message from the server about the current realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmResponse<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> {
  /// Indicate the results of an ACL change request. If no message is supplied, the request was
  /// successful; otherwise, an error is provided.
  AccessChange {
    id: i32,
    result: crate::UpdateResult,
  },
  /// The current ACLs associated with a realm
  AccessCurrent {
    target: RealmAccessTarget,
    rules: Vec<crate::access::AccessControl<crate::access::SimpleAccess>>,
    default: crate::access::SimpleAccess,
  },
  /// The realm announcements have changed
  Announcements(Vec<RealmAnnouncement<S>>),
  /// Whether updating the announcements (either set or clear) was successful (true) or failed (false)
  AnnouncementUpdate {
    id: i32,
    result: super::UpdateResult,
  },
  Kick {
    id: i32,
    result: bool,
  },
  NameChange {
    id: i32,
    result: crate::UpdateResult,
  },
  /// The realm's name and/or directory listing status has been changed
  NameChanged {
    name: S,
    in_directory: bool,
  },
  SettingChange {
    id: i32,
    result: crate::UpdateResult,
  },
  /// A setting has been changed
  SettingChanged {
    name: S,
    value: RealmSetting<S>,
  },
  UpdateState {
    time: chrono::DateTime<chrono::Utc>,
    player: PlayerStates<S>,
    state: PropertyStates<S>,
  },
}
/// The access control type for a realm
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash, Copy)]
pub enum RealmAccessTarget {
  /// Access for the current realm
  Access,
  /// Administrator for the current realm
  Admin,
}
/// A realm-specific announcement that should be visible to users
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RealmAnnouncement<S: AsRef<str>> {
  /// The summary/title of the event
  pub title: S,
  /// The text that should be displayed
  pub body: S,
  /// The time when the event described will start
  pub when: crate::communication::AnnouncementTime,
  /// The announcement is visible on the public calendar (i.e., it can be seen without logging in)
  pub public: bool,
}

/// A property that can be set by realm administrators (rather than by puzzles)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmSetting<S: AsRef<str>> {
  AudioSource(RealmSettingAudio<S>),
  Bool(bool),
  Color(crate::asset::Color),
  Intensity(f64),
  Num(u32),
  Realm(RealmTarget<S>),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RealmSettingAudio<S: AsRef<str>> {
  Silence,
  Static,
  Remote(S),
}
pub type RealmSettings<S> = std::collections::BTreeMap<S, RealmSetting<S>>;

/// The realm that has been selected
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum RealmTarget<S: AsRef<str>> {
  Home,
  LocalRealm { asset: S, owner: S },
  PersonalRealm { asset: S },
  RemoteRealm { asset: S, owner: S, server: S },
}
pub enum RealmTargetParseError {
  BadHost,
  BadPath,
  BadSchema,
  UrlError(url::ParseError),
  EncodingError(std::str::Utf8Error),
}

impl<S: AsRef<str>> AbsoluteRealmTarget<S> {
  pub fn as_ref<'a>(&'a self) -> AbsoluteRealmTarget<&'a str> {
    AbsoluteRealmTarget { asset: self.asset.as_ref(), owner: self.owner.as_ref(), server: self.server.as_ref() }
  }
  pub fn as_local<'a>(&'a self) -> LocalRealmTarget<&'a str> {
    LocalRealmTarget { asset: self.asset.as_ref(), owner: self.owner.as_ref() }
  }
  pub fn convert_str<R: AsRef<str>>(self) -> AbsoluteRealmTarget<R>
  where
    S: Into<R>,
  {
    let AbsoluteRealmTarget { asset, owner, server } = self;
    AbsoluteRealmTarget { asset: asset.into(), owner: owner.into(), server: server.into() }
  }
  pub fn into_local(self) -> (LocalRealmTarget<S>, S) {
    (LocalRealmTarget { asset: self.asset, owner: self.owner }, self.server)
  }
  pub fn to_url(&self) -> String {
    RealmTarget::RemoteRealm { asset: self.asset.as_ref(), owner: self.owner.as_ref(), server: self.server.as_ref() }.to_url()
  }
}

impl<S: AsRef<str>> Action<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> Action<T>
  where
    S: Into<T>,
  {
    match self {
      Action::Emote { animation, duration } => Action::Emote { animation: animation.convert_str(), duration },
      Action::Interaction { target, interaction } => Action::Interaction { target: target.convert_str(), interaction: interaction.convert_str() },
      Action::Move { length } => Action::Move { length },
      Action::Rotate { direction } => Action::Rotate { direction },
    }
  }
}
impl<S: AsRef<str>> CharacterAnimation<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> CharacterAnimation<T>
  where
    S: Into<T>,
  {
    match self {
      CharacterAnimation::Idle => CharacterAnimation::Idle,
      CharacterAnimation::Walk => CharacterAnimation::Walk,
      CharacterAnimation::Climb => CharacterAnimation::Climb,
      CharacterAnimation::Jump => CharacterAnimation::Jump,
      CharacterAnimation::Confused => CharacterAnimation::Confused,
      CharacterAnimation::Touch => CharacterAnimation::Touch,
      CharacterAnimation::Custom(s) => CharacterAnimation::Custom(s.into()),
    }
  }
}
impl<T, S: AsRef<str>> CharacterMotion<T, S>
where
  T: serde::de::DeserializeOwned + Serialize,
{
  pub fn convert_str<U: AsRef<str>>(self) -> CharacterMotion<T, U>
  where
    S: Into<U>,
  {
    match self {
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start, animation: animation.into(), at }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start, animation: animation.into(), at }
      }
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start, animation: animation.convert_str(), direction, at }
      }
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to, end },
      CharacterMotion::Interaction { start, end, animation, at } => {
        CharacterMotion::Interaction { start, end, animation: animation.convert_str(), at }
      }
      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from, start },
      CharacterMotion::Move { from, to, start, end, animation } => CharacterMotion::Move { from, to, start, end, animation: animation.convert_str() },
      CharacterMotion::Rotate { at, start, end, direction } => CharacterMotion::Rotate { at, start, end, direction },
    }
  }

  pub fn time(&self) -> &chrono::DateTime<chrono::Utc> {
    match &self {
      CharacterMotion::ConsensualEmoteInitiator { start, .. } => start,
      CharacterMotion::ConsensualEmoteRecipient { start, .. } => start,
      CharacterMotion::DirectedEmote { start, .. } => start,
      CharacterMotion::Enter { end, .. } => end,
      CharacterMotion::Interaction { end, .. } => end,
      CharacterMotion::Leave { start, .. } => start,
      CharacterMotion::Move { end, .. } => end,
      CharacterMotion::Rotate { end, .. } => end,
    }
  }
  pub fn end_position(&self) -> Option<&T> {
    match &self {
      CharacterMotion::ConsensualEmoteInitiator { at, .. } => Some(at),
      CharacterMotion::ConsensualEmoteRecipient { at, .. } => Some(at),
      CharacterMotion::DirectedEmote { at, .. } => Some(at),
      CharacterMotion::Enter { to, .. } => Some(to),
      CharacterMotion::Interaction { at, .. } => Some(at),
      CharacterMotion::Leave { .. } => None,
      CharacterMotion::Move { to, .. } => Some(to),
      CharacterMotion::Rotate { at, .. } => Some(at),
    }
  }
}
impl<T, S: AsRef<str> + Clone> CharacterMotion<T, S> {
  /// Create a new character motion in a different coordinate system
  pub fn map<R: serde::de::DeserializeOwned + Serialize, F: Fn(&T) -> R>(&self, func: F) -> CharacterMotion<R, S> {
    match self {
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start: start.clone(), animation: animation.clone(), at: func(at) }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start: start.clone(), animation: animation.clone(), at: func(at) }
      }
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start: start.clone(), animation: animation.clone(), direction: direction.clone(), at: func(at) }
      }
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to: func(to), end: *end },
      CharacterMotion::Interaction { animation, start, end, at } => {
        CharacterMotion::Interaction { animation: animation.clone(), start: *start, end: *end, at: func(at) }
      }
      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from: func(from), start: *start },
      CharacterMotion::Move { from, to, start, end, animation } => {
        CharacterMotion::Move { from: func(from), to: func(to), start: *start, end: *end, animation: animation.clone() }
      }
      CharacterMotion::Rotate { at, start, end, direction } => {
        CharacterMotion::Rotate { at: func(at), start: *start, end: *end, direction: *direction }
      }
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

impl From<Direction> for u32 {
  fn from(direction: Direction) -> u32 {
    match direction {
      Direction::YPos => 0,
      Direction::XPos => 1,
      Direction::YNeg => 2,
      Direction::XNeg => 3,
    }
  }
}
impl From<u32> for Direction {
  fn from(direction: u32) -> Direction {
    match direction % 4 {
      0 => Direction::YPos,
      1 => Direction::XPos,
      2 => Direction::YNeg,
      3 => Direction::XNeg,
      _ => panic!("impossible direction"),
    }
  }
}
impl rand::distributions::Distribution<Direction> for rand::distributions::Standard {
  fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Direction {
    rng.gen_range(0..=4).into()
  }
}
impl<S: AsRef<str>> InteractionKey<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> InteractionKey<T>
  where
    S: Into<T>,
  {
    match self {
      InteractionKey::Button(value) => InteractionKey::Button(value.into()),
      InteractionKey::Switch(value) => InteractionKey::Switch(value.into()),
      InteractionKey::RadioButton(value) => InteractionKey::RadioButton(value.into()),
      InteractionKey::RealmSelector(value) => InteractionKey::RealmSelector(value.into()),
    }
  }
}
impl<S: AsRef<str>> InteractionType<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> InteractionType<T>
  where
    S: Into<T>,
  {
    match self {
      InteractionType::Click => InteractionType::Click,
      InteractionType::Realm(realm) => InteractionType::Realm(realm.convert_str()),
    }
  }
}

impl<S: AsRef<str>> LocalRealmTarget<S> {
  pub fn as_ref<'a>(&'a self) -> LocalRealmTarget<&'a str> {
    LocalRealmTarget { asset: self.asset.as_ref(), owner: self.owner.as_ref() }
  }
  pub fn convert_str<R: AsRef<str>>(self) -> LocalRealmTarget<R>
  where
    S: Into<R>,
  {
    let LocalRealmTarget { asset, owner } = self;
    LocalRealmTarget { asset: asset.into(), owner: owner.into() }
  }
  pub fn into_absolute(self, server: S) -> AbsoluteRealmTarget<S> {
    AbsoluteRealmTarget { asset: self.asset, owner: self.owner, server }
  }
  pub fn to_url(&self) -> String {
    RealmTarget::LocalRealm { asset: self.asset.as_ref(), owner: self.owner.as_ref() }.to_url()
  }
}
impl<S: AsRef<str>> std::fmt::Display for LocalRealmTarget<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    RealmTarget::LocalRealm { asset: self.asset.as_ref(), owner: self.owner.as_ref() }.fmt(f)
  }
}
impl<S: AsRef<str>> PropertyKey<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> PropertyKey<T>
  where
    S: Into<T>,
  {
    match self {
      PropertyKey::BoolSink(n) => PropertyKey::BoolSink(n.into()),
      PropertyKey::EventSink(n) => PropertyKey::EventSink(n.into()),
      PropertyKey::NumSink(n) => PropertyKey::NumSink(n.into()),
    }
  }
}
impl<S: AsRef<str>> PlayerState<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> PlayerState<T>
  where
    S: Into<T>,
  {
    PlayerState {
      effect: self.effect,
      final_direction: self.final_direction,
      final_position: self.final_position,
      motion: self.motion.into_iter().map(|m| m.convert_str()).collect(),
    }
  }
}
impl Point {
  pub fn is_neighbour(&self, other: &Self) -> bool {
    self.platform == other.platform
      && match (crate::abs_difference(self.x, other.x), crate::abs_difference(self.y, other.y)) {
        (0, 0) | (0, 1) | (1, 0) => true,
        _ => false,
      }
  }
  pub fn neighbour(&self, direction: Direction) -> Option<Self> {
    let x = match direction {
      Direction::XPos => self.x.checked_add(1),
      Direction::XNeg => self.x.checked_sub(1),
      _ => Some(self.x),
    }?;
    let y = match direction {
      Direction::YPos => self.y.checked_add(1),
      Direction::YNeg => self.y.checked_sub(1),
      _ => Some(self.y),
    }?;
    Some(Point { platform: self.platform, x, y })
  }
}
impl<S: AsRef<str>> RealmAnnouncement<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> RealmAnnouncement<T>
  where
    S: Into<T>,
  {
    RealmAnnouncement { title: self.title.into(), body: self.body.into(), when: self.when, public: self.public }
  }
}
impl<S: AsRef<str>> RealmDirectoryEntry<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> RealmDirectoryEntry<T>
  where
    S: Into<T>,
  {
    RealmDirectoryEntry {
      asset: self.asset.into(),
      name: self.name.into(),
      activity: self.activity,
      owner: self.owner.into(),
      server: self.server.into(),
      train: self.train,
    }
  }
}
impl<S: AsRef<str>> RealmRequest<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> RealmRequest<T>
  where
    S: Into<T>,
  {
    match self {
      RealmRequest::AccessGet { target } => RealmRequest::AccessGet { target },
      RealmRequest::AccessSet { id, target, rules, default } => RealmRequest::AccessSet { id, target, rules, default },
      RealmRequest::AnnouncementAdd { id, announcement } => RealmRequest::AnnouncementAdd { id, announcement: announcement.convert_str() },
      RealmRequest::AnnouncementClear { id } => RealmRequest::AnnouncementClear { id },
      RealmRequest::AnnouncementList => RealmRequest::AnnouncementList,
      RealmRequest::ChangeName { id, name, in_directory } => RealmRequest::ChangeName { id, name: name.map(|n| n.into()), in_directory },
      RealmRequest::ChangeSetting { id, name, value } => RealmRequest::ChangeSetting { id, name: name.into(), value: value.convert_str() },
      RealmRequest::Delete => RealmRequest::Delete,
      RealmRequest::Kick { id, target } => RealmRequest::Kick { id, target: target.convert_str() },
      RealmRequest::NoOperation => RealmRequest::NoOperation,
      RealmRequest::Perform(actions) => RealmRequest::Perform(actions.into_iter().map(|a| a.convert_str()).collect()),
    }
  }
}
impl<S: AsRef<str> + Eq + std::hash::Hash + std::cmp::Ord> From<RealmRequest<S>> for crate::ClientRequest<S> {
  fn from(value: RealmRequest<S>) -> Self {
    crate::ClientRequest::InRealm { request: value }
  }
}
impl<S: AsRef<str> + Eq + std::hash::Hash> RealmResponse<S> {
  pub fn convert_str<T: AsRef<str> + Eq + std::hash::Hash>(self) -> RealmResponse<T>
  where
    S: Into<T>,
  {
    match self {
      RealmResponse::AccessChange { id, result } => RealmResponse::AccessChange { id, result },
      RealmResponse::AccessCurrent { target, rules, default } => RealmResponse::AccessCurrent { target, rules, default },
      RealmResponse::Announcements(announcements) => RealmResponse::Announcements(announcements.into_iter().map(|a| a.convert_str()).collect()),
      RealmResponse::AnnouncementUpdate { id, result } => RealmResponse::AnnouncementUpdate { id, result },
      RealmResponse::Kick { id, result } => RealmResponse::Kick { id, result },
      RealmResponse::NameChange { id, result } => RealmResponse::NameChange { id, result },
      RealmResponse::NameChanged { name, in_directory } => RealmResponse::NameChanged { name: name.into(), in_directory },
      RealmResponse::SettingChange { id, result } => RealmResponse::SettingChange { id, result },
      RealmResponse::SettingChanged { name, value } => RealmResponse::SettingChanged { name: name.into(), value: value.convert_str() },
      RealmResponse::UpdateState { time, player, state } => RealmResponse::UpdateState {
        time,
        player: player.into_iter().map(|(k, v)| (k.convert_str(), v.convert_str())).collect(),
        state: state.into_iter().map(|(k, v)| (k.convert_str(), v)).collect(),
      },
    }
  }
}
impl<S: AsRef<str>> RealmSetting<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> RealmSetting<T>
  where
    S: Into<T>,
  {
    match self {
      RealmSetting::AudioSource(source) => RealmSetting::AudioSource(source.convert_str()),
      RealmSetting::Bool(value) => RealmSetting::Bool(value),
      RealmSetting::Color(value) => RealmSetting::Color(value),
      RealmSetting::Intensity(value) => RealmSetting::Intensity(value),
      RealmSetting::Num(value) => RealmSetting::Num(value),
      RealmSetting::Realm(value) => RealmSetting::Realm(value.convert_str()),
    }
  }
  /// Ensure any data in the request looks like valid data
  pub fn clean(self) -> Option<Self> {
    match self {
      RealmSetting::Realm(target) => Some(RealmSetting::Realm(target)),
      RealmSetting::AudioSource(source) => Some(RealmSetting::AudioSource(source.clean()?)),
      x => Some(x),
    }
  }
}
impl<S: AsRef<str> + Clone> RealmSetting<S> {
  /// Update a setting from a value, but only if it's the correct type
  pub fn type_matched_update(&mut self, other: &Self) -> bool {
    match self {
      RealmSetting::AudioSource(source) => {
        if let RealmSetting::AudioSource(new_source) = other {
          *source = new_source.clone();
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
      RealmSetting::AudioSource(_) => "audio source",
      RealmSetting::Bool(_) => "Boolean value",
      RealmSetting::Color(_) => "color",
      RealmSetting::Intensity(_) => "intensity",
      RealmSetting::Num(_) => "numeric value",
      RealmSetting::Realm { .. } => "realm",
    }
  }
}
impl<S: AsRef<str>> RealmSettingAudio<S> {
  pub fn convert_str<T: AsRef<str>>(self) -> RealmSettingAudio<T>
  where
    S: Into<T>,
  {
    match self {
      RealmSettingAudio::Silence => RealmSettingAudio::Silence,
      RealmSettingAudio::Static => RealmSettingAudio::Static,
      RealmSettingAudio::Remote(url) => RealmSettingAudio::Remote(url.into()),
    }
  }
  pub fn clean(self) -> Option<Self> {
    match self {
      RealmSettingAudio::Silence => Some(RealmSettingAudio::Silence),
      RealmSettingAudio::Static => Some(RealmSettingAudio::Static),
      RealmSettingAudio::Remote(stream) => {
        if url::Url::parse(stream.as_ref()).is_ok() {
          Some(RealmSettingAudio::Remote(stream))
        } else {
          None
        }
      }
    }
  }
}
impl<S: AsRef<str>> RealmSource<S> {
  pub fn convert_str<R: AsRef<str>>(self) -> RealmSource<R>
  where
    S: Into<R>,
  {
    match self {
      RealmSource::Personal => RealmSource::Personal,
      RealmSource::Bookmarks => RealmSource::Bookmarks,
      RealmSource::LocalServer => RealmSource::LocalServer,
      RealmSource::RemoteServer(server) => RealmSource::RemoteServer(server.into()),
      RealmSource::Manual(realm) => RealmSource::Manual(realm.convert_str()),
    }
  }
}
impl<S: AsRef<str>> RealmTarget<S> {
  pub fn new<T>(asset: S, player: crate::player::PlayerIdentifier<S>) -> Self {
    match player {
      crate::player::PlayerIdentifier::Local(owner) => RealmTarget::LocalRealm { owner, asset },
      crate::player::PlayerIdentifier::Remote { player: owner, server } => RealmTarget::RemoteRealm { owner, asset, server },
    }
  }
  pub fn as_owned_str(&self) -> RealmTarget<String> {
    match self {
      RealmTarget::Home => RealmTarget::Home,
      RealmTarget::PersonalRealm { asset } => RealmTarget::PersonalRealm { asset: asset.as_ref().to_string() },
      RealmTarget::LocalRealm { asset, owner } => RealmTarget::LocalRealm { asset: asset.as_ref().to_string(), owner: owner.as_ref().to_string() },
      RealmTarget::RemoteRealm { asset, owner, server } => {
        RealmTarget::RemoteRealm { asset: asset.as_ref().to_string(), owner: owner.as_ref().to_string(), server: server.as_ref().to_string() }
      }
    }
  }
  pub fn as_ref(&self) -> RealmTarget<&'_ str> {
    match self {
      RealmTarget::Home => RealmTarget::Home,
      RealmTarget::PersonalRealm { asset } => RealmTarget::PersonalRealm { asset: asset.as_ref() },
      RealmTarget::LocalRealm { asset, owner } => RealmTarget::LocalRealm { asset: asset.as_ref(), owner: owner.as_ref() },
      RealmTarget::RemoteRealm { asset, owner, server } => {
        RealmTarget::RemoteRealm { asset: asset.as_ref(), owner: owner.as_ref(), server: server.as_ref() }
      }
    }
  }
  pub fn convert_str<R: AsRef<str>>(self) -> RealmTarget<R>
  where
    S: Into<R>,
  {
    match self {
      RealmTarget::Home => RealmTarget::Home,
      RealmTarget::LocalRealm { asset, owner } => RealmTarget::LocalRealm { asset: asset.into(), owner: owner.into() },
      RealmTarget::PersonalRealm { asset } => RealmTarget::PersonalRealm { asset: asset.into() },
      RealmTarget::RemoteRealm { asset, owner, server } => {
        RealmTarget::RemoteRealm { asset: asset.into(), owner: owner.into(), server: server.into() }
      }
    }
  }
  pub fn globalize(self, server: impl Into<S>) -> Self {
    match self {
      RealmTarget::Home => RealmTarget::Home,
      RealmTarget::PersonalRealm { asset } => RealmTarget::PersonalRealm { asset },
      RealmTarget::LocalRealm { asset, owner } => RealmTarget::RemoteRealm { asset, owner, server: server.into() },
      RealmTarget::RemoteRealm { asset, owner, server } => RealmTarget::RemoteRealm { asset, owner, server },
    }
  }
  pub fn localize(self, local_server: &str) -> Self {
    match self {
      RealmTarget::Home => RealmTarget::Home,
      RealmTarget::PersonalRealm { asset } => RealmTarget::PersonalRealm { asset },
      RealmTarget::LocalRealm { asset, owner } => RealmTarget::LocalRealm { asset, owner },
      RealmTarget::RemoteRealm { asset, owner, server } => {
        if server.as_ref() == local_server {
          RealmTarget::LocalRealm { asset, owner }
        } else {
          RealmTarget::RemoteRealm { asset, owner, server }
        }
      }
    }
  }
  pub fn to_url(&self) -> String {
    self.to_string()
  }
}
impl<S: AsRef<str>> std::fmt::Display for RealmTarget<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      RealmTarget::PersonalRealm { asset } => {
        write!(f, "spadina:~{}", percent_encoding::percent_encode(asset.as_ref().as_bytes(), percent_encoding::NON_ALPHANUMERIC))
      }
      RealmTarget::LocalRealm { asset, owner } => write!(
        f,
        "spadina:///{}/{}",
        percent_encoding::percent_encode(owner.as_ref().as_bytes(), percent_encoding::NON_ALPHANUMERIC),
        percent_encoding::percent_encode(asset.as_ref().as_bytes(), percent_encoding::NON_ALPHANUMERIC)
      ),
      RealmTarget::RemoteRealm { asset, owner, server } => write!(
        f,
        "spadina://{}/{}/{}",
        server.as_ref(),
        percent_encoding::percent_encode(owner.as_ref().as_bytes(), percent_encoding::NON_ALPHANUMERIC),
        percent_encoding::percent_encode(asset.as_ref().as_bytes(), percent_encoding::NON_ALPHANUMERIC)
      ),
      RealmTarget::Home => f.write_str("spadina:~"),
    }
  }
}
impl<S: AsRef<str>> From<AbsoluteRealmTarget<S>> for RealmTarget<S> {
  fn from(value: AbsoluteRealmTarget<S>) -> Self {
    let AbsoluteRealmTarget { asset, owner, server } = value;
    RealmTarget::RemoteRealm { asset, owner, server }
  }
}
impl<S: AsRef<str>> From<LocalRealmTarget<S>> for RealmTarget<S> {
  fn from(value: LocalRealmTarget<S>) -> Self {
    let LocalRealmTarget { asset, owner } = value;
    RealmTarget::LocalRealm { asset, owner }
  }
}
impl std::str::FromStr for RealmTarget<String> {
  type Err = RealmTargetParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match url::Url::parse(s) {
      Ok(url) => {
        if url.scheme() == "spadina" {
          if url.cannot_be_a_base() {
            if url.path() == "~" {
              Ok(RealmTarget::Home)
            } else if url.path().starts_with("~") {
              Ok(RealmTarget::PersonalRealm { asset: percent_encoding::percent_decode_str(&url.path()[1..]).decode_utf8()?.to_string() })
            } else {
              Err(RealmTargetParseError::BadPath)
            }
          } else if let Some(path_segments) = url.path_segments().map(|s| s.collect::<Vec<_>>()) {
            if let [owner, asset] = path_segments.as_slice() {
              let owner = percent_encoding::percent_decode_str(owner).decode_utf8()?.to_string();
              let asset = percent_encoding::percent_decode_str(asset).decode_utf8()?.to_string();
              match url.host() {
                None => Ok(RealmTarget::LocalRealm { owner, asset }),
                Some(url::Host::Domain(host)) => match crate::net::parse_server_name(host) {
                  Some(host) => Ok(RealmTarget::RemoteRealm { owner, asset, server: host }),
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
impl From<std::str::Utf8Error> for RealmTargetParseError {
  fn from(value: std::str::Utf8Error) -> Self {
    RealmTargetParseError::EncodingError(value)
  }
}
