pub mod room_world;

use crate::reference_converter::{Converter, Referencer};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use std::hash::Hash;

//ConsensualEmoteRequest { emote: S, player: player::PlayerIdentifier<S>, }, ConsensualEmoteResponse { id: i32, ok: bool, },  FollowRequest { player: player::PlayerIdentifier<S>, }, FollowResponse { id: i32, ok: bool, },
//

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
    target: S,
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
pub enum CharacterMotion<T: Copy, S: AsRef<str>> {
  DirectedEmote {
    start: DateTime<Utc>,
    animation: CharacterAnimation<S>,
    direction: Direction,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteInitiator {
    start: DateTime<Utc>,
    animation: S,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  ConsensualEmoteRecipient {
    start: DateTime<Utc>,
    animation: S,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  Enter {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    end: DateTime<Utc>,
  },
  Interaction {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    animation: CharacterAnimation<S>,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
  },
  Leave {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    start: DateTime<Utc>,
  },
  Move {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    from: T,
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    to: T,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    animation: CharacterAnimation<S>,
  },
  Rotate {
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    at: T,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    direction: Direction,
  },
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
  Ticks(Vec<DateTime<Utc>>),
}

/// A request from the player to the realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmRequest<S: AsRef<str>> {
  /// Change a setting associated with the realm
  ChangeSetting { id: i32, name: S, value: RealmSetting<S> },
  /// Request that we move our avatar to the new location specified. All units are in absolute
  /// coordinates from the origin of a realm in 10cm increments. The server is not obliged to
  /// move us to these coordinates; it will make a judgement and send information to the client.
  Perform(Vec<Action<S>>),
}
/// A collection of labelled property values
pub type PropertyStates<S> = std::collections::HashMap<PropertyKey<S>, PropertyValue>;

/// A message from the server about the current realm
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmResponse<S: AsRef<str> + Eq + Hash> {
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
    time: DateTime<Utc>,
    player: PlayerStates<S>,
    state: PropertyStates<S>,
  },
}

/// A property that can be set by realm administrators (rather than by puzzles)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmSetting<S: AsRef<str>> {
  AudioSource(RealmSettingAudio<S>),
  Bool(bool),
  Color(crate::scene::Color),
  Intensity(f64),
  Num(u32),
  Realm(crate::location::target::UnresolvedTarget<S>),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RealmSettingAudio<S: AsRef<str>> {
  Silence,
  Static,
  Remote(S),
}
pub type RealmSettings<S> = std::collections::BTreeMap<S, RealmSetting<S>>;

pub enum RealmTargetParseError {
  BadHost,
  BadPath,
  BadSchema,
  UrlError(url::ParseError),
  EncodingError(std::str::Utf8Error),
}

impl<S: AsRef<str>> Action<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> Action<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      Action::Emote { animation, duration } => Action::Emote { animation: animation.reference(reference), duration: *duration },
      Action::Interaction { target } => Action::Interaction { target: reference.convert(target) },
      Action::Move { length } => Action::Move { length: *length },
      Action::Rotate { direction } => Action::Rotate { direction: *direction },
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> Action<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      Action::Emote { animation, duration } => Action::Emote { animation: animation.convert(converter), duration },
      Action::Interaction { target } => Action::Interaction { target: converter.convert(target) },
      Action::Move { length } => Action::Move { length },
      Action::Rotate { direction } => Action::Rotate { direction },
    }
  }
}
impl<S: AsRef<str>> CharacterAnimation<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> CharacterAnimation<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      CharacterAnimation::Idle => CharacterAnimation::Idle,
      CharacterAnimation::Walk => CharacterAnimation::Walk,
      CharacterAnimation::Climb => CharacterAnimation::Climb,
      CharacterAnimation::Jump => CharacterAnimation::Jump,
      CharacterAnimation::Confused => CharacterAnimation::Confused,
      CharacterAnimation::Touch => CharacterAnimation::Touch,
      CharacterAnimation::Custom(s) => CharacterAnimation::Custom(reference.convert(s)),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> CharacterAnimation<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      CharacterAnimation::Idle => CharacterAnimation::Idle,
      CharacterAnimation::Walk => CharacterAnimation::Walk,
      CharacterAnimation::Climb => CharacterAnimation::Climb,
      CharacterAnimation::Jump => CharacterAnimation::Jump,
      CharacterAnimation::Confused => CharacterAnimation::Confused,
      CharacterAnimation::Touch => CharacterAnimation::Touch,
      CharacterAnimation::Custom(s) => CharacterAnimation::Custom(converter.convert(s)),
    }
  }
}
impl<T: Copy, S: AsRef<str>> CharacterMotion<T, S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> CharacterMotion<T, R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start: *start, animation: reference.convert(animation), at: *at }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start: *start, animation: reference.convert(animation), at: *at }
      }
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start: *start, animation: animation.reference(reference), direction: *direction, at: *at }
      }
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to: *to, end: *end },
      CharacterMotion::Interaction { start, end, animation, at } => {
        CharacterMotion::Interaction { start: *start, end: *end, animation: animation.reference(reference), at: *at }
      }
      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from: *from, start: *start },
      CharacterMotion::Move { from, to, start, end, animation } => {
        CharacterMotion::Move { from: *from, to: *to, start: *start, end: *end, animation: animation.reference(reference) }
      }
      CharacterMotion::Rotate { at, start, end, direction } => CharacterMotion::Rotate { at: *at, start: *start, end: *end, direction: *direction },
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> CharacterMotion<T, C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start, animation: converter.convert(animation), at }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start, animation: converter.convert(animation), at }
      }
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start, animation: animation.convert(converter), direction, at }
      }
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to, end },
      CharacterMotion::Interaction { start, end, animation, at } => {
        CharacterMotion::Interaction { start, end, animation: animation.convert(converter), at }
      }
      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from, start },
      CharacterMotion::Move { from, to, start, end, animation } => {
        CharacterMotion::Move { from, to, start, end, animation: animation.convert(converter) }
      }
      CharacterMotion::Rotate { at, start, end, direction } => CharacterMotion::Rotate { at, start, end, direction },
    }
  }

  pub fn time(&self) -> &DateTime<Utc> {
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
  /// Create a new character motion in a different coordinate system
  pub fn map<R: Copy, F: Fn(T) -> R>(self, func: F) -> CharacterMotion<R, S> {
    match self {
      CharacterMotion::ConsensualEmoteInitiator { start, animation, at } => {
        CharacterMotion::ConsensualEmoteInitiator { start, animation, at: func(at) }
      }
      CharacterMotion::ConsensualEmoteRecipient { start, animation, at } => {
        CharacterMotion::ConsensualEmoteRecipient { start, animation, at: func(at) }
      }
      CharacterMotion::DirectedEmote { start, animation, direction, at } => {
        CharacterMotion::DirectedEmote { start, animation, direction, at: func(at) }
      }
      CharacterMotion::Enter { to, end } => CharacterMotion::Enter { to: func(to), end },
      CharacterMotion::Interaction { animation, start, end, at } => CharacterMotion::Interaction { animation, start, end, at: func(at) },
      CharacterMotion::Leave { from, start } => CharacterMotion::Leave { from: func(from), start },
      CharacterMotion::Move { from, to, start, end, animation } => CharacterMotion::Move { from: func(from), to: func(to), start, end, animation },
      CharacterMotion::Rotate { at, start, end, direction } => CharacterMotion::Rotate { at: func(at), start, end, direction },
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

impl<S: AsRef<str>> PropertyKey<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> PropertyKey<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      PropertyKey::BoolSink(n) => PropertyKey::BoolSink(reference.convert(n)),
      PropertyKey::EventSink(n) => PropertyKey::EventSink(reference.convert(n)),
      PropertyKey::NumSink(n) => PropertyKey::NumSink(reference.convert(n)),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> PropertyKey<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      PropertyKey::BoolSink(n) => PropertyKey::BoolSink(converter.convert(n)),
      PropertyKey::EventSink(n) => PropertyKey::EventSink(converter.convert(n)),
      PropertyKey::NumSink(n) => PropertyKey::NumSink(converter.convert(n)),
    }
  }
}
impl<S: AsRef<str>> PlayerState<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> PlayerState<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    PlayerState {
      effect: self.effect.clone(),
      final_direction: self.final_direction,
      final_position: self.final_position,
      motion: self.motion.iter().map(|m| m.reference(reference)).collect(),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> PlayerState<C::Output>
  where
    C::Output: AsRef<str>,
  {
    PlayerState {
      effect: self.effect,
      final_direction: self.final_direction,
      final_position: self.final_position,
      motion: self.motion.into_iter().map(|m| m.convert(converter)).collect(),
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

impl<S: AsRef<str>> RealmRequest<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> RealmRequest<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      RealmRequest::ChangeSetting { id, name, value } => {
        RealmRequest::ChangeSetting { id: *id, name: reference.convert(name), value: value.reference(reference) }
      }

      RealmRequest::Perform(actions) => RealmRequest::Perform(actions.into_iter().map(|a| a.reference(reference)).collect()),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> RealmRequest<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      RealmRequest::ChangeSetting { id, name, value } => {
        RealmRequest::ChangeSetting { id, name: converter.convert(name), value: value.convert(converter) }
      }

      RealmRequest::Perform(actions) => RealmRequest::Perform(actions.into_iter().map(|a| a.convert(converter)).collect()),
    }
  }
}
impl<S: AsRef<str> + Eq + Hash> RealmResponse<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> RealmResponse<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str> + Eq + Hash,
  {
    match self {
      RealmResponse::SettingChange { id, result } => RealmResponse::SettingChange { id: *id, result: *result },
      RealmResponse::SettingChanged { name, value } => {
        RealmResponse::SettingChanged { name: reference.convert(name), value: value.reference(reference) }
      }
      RealmResponse::UpdateState { time, player, state } => RealmResponse::UpdateState {
        time: *time,
        player: player.iter().map(|(k, v)| (k.reference(reference), v.reference(reference))).collect(),
        state: state.iter().map(|(k, v)| (k.reference(reference), v.clone())).collect(),
      },
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> RealmResponse<C::Output>
  where
    C::Output: AsRef<str> + Eq + Hash,
  {
    match self {
      RealmResponse::SettingChange { id, result } => RealmResponse::SettingChange { id, result },
      RealmResponse::SettingChanged { name, value } => {
        RealmResponse::SettingChanged { name: converter.convert(name), value: value.convert(converter) }
      }
      RealmResponse::UpdateState { time, player, state } => RealmResponse::UpdateState {
        time,
        player: player.into_iter().map(|(k, v)| (k.convert(converter), v.convert(converter))).collect(),
        state: state.into_iter().map(|(k, v)| (k.convert(converter), v)).collect(),
      },
    }
  }
}
impl<S: AsRef<str>> RealmSetting<S> {
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> RealmSetting<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      RealmSetting::AudioSource(source) => RealmSetting::AudioSource(source.reference(reference)),
      RealmSetting::Bool(value) => RealmSetting::Bool(*value),
      RealmSetting::Color(value) => RealmSetting::Color(*value),
      RealmSetting::Intensity(value) => RealmSetting::Intensity(*value),
      RealmSetting::Num(value) => RealmSetting::Num(*value),
      RealmSetting::Realm(value) => RealmSetting::Realm(value.reference(reference)),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> RealmSetting<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      RealmSetting::AudioSource(source) => RealmSetting::AudioSource(source.convert(converter)),
      RealmSetting::Bool(value) => RealmSetting::Bool(value),
      RealmSetting::Color(value) => RealmSetting::Color(value),
      RealmSetting::Intensity(value) => RealmSetting::Intensity(value),
      RealmSetting::Num(value) => RealmSetting::Num(value),
      RealmSetting::Realm(value) => RealmSetting::Realm(value.convert(converter)),
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
  pub fn reference<'a, R: Referencer<S>>(&'a self, reference: R) -> RealmSettingAudio<R::Output<'a>>
  where
    R::Output<'a>: AsRef<str>,
  {
    match self {
      RealmSettingAudio::Silence => RealmSettingAudio::Silence,
      RealmSettingAudio::Static => RealmSettingAudio::Static,
      RealmSettingAudio::Remote(url) => RealmSettingAudio::Remote(reference.convert(url)),
    }
  }
  pub fn convert<C: Converter<S>>(self, converter: C) -> RealmSettingAudio<C::Output>
  where
    C::Output: AsRef<str>,
  {
    match self {
      RealmSettingAudio::Silence => RealmSettingAudio::Silence,
      RealmSettingAudio::Static => RealmSettingAudio::Static,
      RealmSettingAudio::Remote(url) => RealmSettingAudio::Remote(converter.convert(url)),
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
