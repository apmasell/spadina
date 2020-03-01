use crate::realm::navigation::PlayerNavigationEvent;
use rmp::decode::{DecodeStringError, ValueReadError};

pub(crate) mod arithmetic;
pub(crate) mod buffer;
pub(crate) mod button;
pub(crate) mod clock;
pub(crate) mod comparator;
pub(crate) mod counter;
pub(crate) mod cycle_button;
pub(crate) mod event_sink;
pub(crate) mod holiday;
pub(crate) mod index;
pub(crate) mod index_list;
pub(crate) mod logic;
pub(crate) mod map_sink;
pub(crate) mod metronome;
pub(crate) mod permutation;
pub(crate) mod proximity;
pub(crate) mod radio_button;
pub(crate) mod realm_selector;
pub(crate) mod sink;
pub(crate) mod switch;
pub(crate) mod timer;

/// Read a value from a Message Pack buffer
pub(crate) trait DecodeSaved: Sized {
  /// Read the value
  fn read(input: &mut InputBuffer) -> Result<Self, rmp::decode::ValueReadError>;
}

type DeserializationResult<'a> = Result<Box<dyn PuzzlePiece + 'a>, rmp::decode::ValueReadError>;

/// A value that can be serialized
pub(crate) trait EncodeValue {
  /// Write the value to a serialization buffer
  fn write(self: &Self, output: &mut OutputBuffer) -> SerializationResult;
}

/// An event created by a puzzle piece updating its state
#[derive(Debug, Clone)]
pub(crate) struct Event {
  /// The piece that triggered the event
  pub(crate) sender: usize,
  /// The event name
  pub(crate) name: puzzleverse_core::PuzzleEvent,
  /// The output value associated with the event
  pub(crate) value: puzzleverse_core::asset::rules::PieceValue,
}

/// A type which can be converted from a puzzle piece value
pub(crate) trait ExtractValue: Sized {
  /// Try to convert the value from a puzzle piece value
  fn extract_value(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<Self>;
}

/// A type where a list of values can be converted from a puzzle piece value
pub(crate) trait ExtractList: Sized {
  /// Try to convert a list of value from a puzzle piece value
  fn extract_list(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<&[Self]>;
}

type InputBuffer<'a> = std::io::Cursor<&'a [u8]>;

type OutputBuffer = std::io::Cursor<Vec<u8>>;

pub(crate) enum OutputEvent {
  Event(puzzleverse_core::PuzzleEvent, puzzleverse_core::asset::rules::PieceValue),
  BitClear(u8, Vec<crate::PlayerKey>),
  BitSet(u8, Vec<crate::PlayerKey>),
  BitToggle(u8, Vec<crate::PlayerKey>),
  Mark(u8, Vec<crate::PlayerKey>),
  Send(puzzleverse_core::asset::rules::RealmLink, Vec<crate::PlayerKey>),
  Unmark(Vec<crate::PlayerKey>),
}

pub(crate) type OutputEvents = Vec<OutputEvent>;

/// A prototype for a puzzle piece that can either produce a blank state for a new realm or recover state from a database
pub(crate) trait PuzzleAsset {
  /// Create a fresh new state
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<String, RadioSharedState>,
  ) -> Box<dyn PuzzlePiece>;
  /// Recover previously serialised state
  fn load<'a>(
    self: Box<Self>,
    input: &mut InputBuffer,
    time: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<String, RadioSharedState>,
  ) -> DeserializationResult<'a>;
}

pub type RadioSharedState = std::sync::Arc<(String, std::sync::atomic::AtomicU32, std::sync::atomic::AtomicBool)>;

/// The event that will have a consequence event
pub struct PuzzleConsequence<'a>(&'a puzzleverse_core::PropertyKey, crate::realm::Multi<puzzleverse_core::PropertyValue>);

/// An active puzzle component in a realm's logic
pub(crate) trait PuzzlePiece: Send + Sync {
  /// Adjust state based on the activity of another puzzle piece and the realm author's rules and generate any output changes the author may want to use
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    time: &chrono::DateTime<chrono::Utc>,
  ) -> OutputEvents;
  /// Respond to a player interacting with this puzzle piece via the UI.
  ///
  /// Most puzzle pieces do not directly interact with players.
  fn interact(
    self: &mut Self,
    interaction: &puzzleverse_core::InteractionType,
    player_server: &str,
    state: Option<u8>,
  ) -> (puzzleverse_core::InteractionResult, SimpleOutputEvents);
  /// Write this puzzle piece out to a format that can be recovered later
  fn serialize(self: &Self, output: &mut OutputBuffer) -> SerializationResult;
  /// Generate any events trigger purely by time events
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> SimpleOutputEvents;
  /// Determine the next time when this piece _could_ update, if it responds to clock changes
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>>;
  /// Generate events when thawed from database
  fn reset(&self) -> SimpleOutputEvents;
  /// Determine if the current state of this piece should trigger any client state changes.
  ///
  /// Many pieces do not directly modify the player environment
  fn update_check<'a>(self: &'a Self, active_states: &std::collections::BTreeSet<u8>) -> Option<PuzzleConsequence<'a>>;
  /// Respond to a player entering or exiting the sphere of influence a puzzle piece
  ///
  /// Most puzzle pieces do not directly interact with player movements.
  fn walk(
    self: &mut Self,
    player: &crate::PlayerKey,
    state: Option<u8>,
    event: crate::realm::navigation::PlayerNavigationEvent,
  ) -> SimpleOutputEvents;
}

type SerializationResult = Result<(), rmp::encode::ValueWriteError>;

type SimpleEvent = (puzzleverse_core::PuzzleEvent, puzzleverse_core::asset::rules::PieceValue);
pub(crate) type SimpleOutputEvents = Vec<SimpleEvent>;

impl Event {
  /// Create a new event generated by a puzzle piece
  pub(crate) fn new(sender: usize, event: SimpleEvent) -> Event {
    Event { sender, name: event.0, value: event.1 }
  }
}

/// Assert that an MessagePack value is an array length with the value specified
pub fn check_length(input: &mut InputBuffer, expected_length: u32) -> Result<(), rmp::decode::ValueReadError> {
  if rmp::decode::read_array_len(input)? == expected_length {
    Ok(())
  } else {
    Err(rmp::decode::ValueReadError::TypeMismatch(rmp::Marker::Reserved))
  }
}
/// Update a realm's state from puzzle pieces
pub(crate) fn prepare_consequences(state: &mut crate::realm::RealmPuzzleState) -> bool {
  let mut changed = false;
  let active_states = state.active_players.values().flat_map(|(_, _, s)| s.iter()).copied().collect();
  for PuzzleConsequence(name, value) in state.pieces.iter().flat_map(|p| p.update_check(&active_states).into_iter()) {
    changed |= match state.current_states.entry(name.clone()) {
      std::collections::hash_map::Entry::Vacant(v) => {
        v.insert(value);
        true
      }
      std::collections::hash_map::Entry::Occupied(mut o) => {
        if o.get_mut() == &value {
          false
        } else {
          *o.get_mut() = value;
          true
        }
      }
    };
  }
  changed
}

/// Update puzzle state based on some player-initiated input event
pub(crate) fn process<T: IntoIterator<Item = Event>>(
  state: &mut crate::realm::RealmPuzzleState,
  time: &chrono::DateTime<chrono::Utc>,
  links: &mut std::collections::HashMap<crate::PlayerKey, puzzleverse_core::asset::rules::RealmLink>,
  events: T,
) {
  let mut count = 100;
  let mut queue = std::collections::VecDeque::new();
  let mut players_in_transit = Vec::new();
  queue.extend(events);
  // We only do 100 rounds of processing and then we assume the puzzle is some kind of unstable oscillator and give up.
  while count > 0 {
    count -= 1;
    // We're really handling two kinds of events: one based on our input and then knock on effects from having moved players around, so first, drain all our input events
    if let Some(current) = queue.pop_front() {
      // Check each puzzle piece for an input event and gather the output events
      for propagation_rule in state
        .propagation_rules
        .iter()
        .filter(|propagation_rule| propagation_rule.sender == current.sender && propagation_rule.trigger == current.name)
      {
        if let Some(piece) = state.pieces.get_mut(propagation_rule.recipient) {
          if let Some(result) = propagation_rule.propagation_match.apply(&current.value, &state.settings).as_mut() {
            for value in piece.accept(&propagation_rule.causes, result, time) {
              match value {
                // Any output events that are due to puzzle state, get fed back into out input queue
                OutputEvent::Event(event_name, value) => {
                  queue.push_back(Event { sender: propagation_rule.recipient, name: event_name, value });
                }
                // The puzzle piece wants to move players; if we've already moved the players around, don't move them again
                OutputEvent::Send(link, players) => {
                  for player in players {
                    if !links.contains_key(&player) {
                      players_in_transit.push(player.clone());
                      links.insert(player, link.clone());
                    }
                  }
                }
                OutputEvent::Mark(s, players) => {
                  for player in players {
                    if let Some((_, _, state)) = state.active_players.get_mut(&player) {
                      *state = Some(s);
                    }
                  }
                }
                OutputEvent::Unmark(players) => {
                  for player in players {
                    if let Some((_, _, state)) = state.active_players.get_mut(&player) {
                      *state = None;
                    }
                  }
                }
                OutputEvent::BitClear(bit, players) => {
                  if let Some(mask) = 1_u8.checked_shl(bit as u32) {
                    for player in players {
                      if let Some((_, _, Some(state))) = state.active_players.get_mut(&player) {
                        *state &= !mask;
                      }
                    }
                  }
                }
                OutputEvent::BitSet(bit, players) => {
                  if let Some(mask) = 1_u8.checked_shl(bit as u32) {
                    for player in players {
                      if let Some((_, _, state)) = state.active_players.get_mut(&player) {
                        *state = match *state {
                          Some(state) => Some(state | mask),
                          None => Some(mask),
                        };
                      }
                    }
                  }
                }
                OutputEvent::BitToggle(bit, players) => {
                  if let Some(mask) = 1_u8.checked_shl(bit as u32) {
                    for player in players {
                      if let Some((_, _, state)) = state.active_players.get_mut(&player) {
                        *state = match *state {
                          Some(state) => Some(state ^ mask),
                          None => Some(mask),
                        };
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    } else if !players_in_transit.is_empty() {
      // Given we've processed all the input events, then make sure our players that are leaving exist nowhere on the map and all the pieces no that. If that generates more events, put them in the queue
      for player in players_in_transit.drain(..) {
        for (recipient, piece) in state.pieces.iter_mut().enumerate() {
          queue.extend(
            piece
              .walk(&player, state.active_players.get(&player).map(|(_, _, s)| s.clone()).flatten(), PlayerNavigationEvent::Leave)
              .drain(..)
              .map(|(event_name, value)| Event { sender: recipient, name: event_name, value }),
          );
        }
      }
    } else {
      // This means we are done with input events and done with clearing players on the map
      break;
    }
  }
}

/// Update puzzle piece states based on the tick of the clock
pub(crate) fn process_time(
  state: &mut crate::realm::RealmPuzzleState,
  time: &chrono::DateTime<chrono::Utc>,
  links: &mut std::collections::HashMap<crate::PlayerKey, puzzleverse_core::asset::rules::RealmLink>,
) {
  let mut events = Vec::new();
  for (piece_name, piece) in state.pieces.iter_mut().enumerate() {
    events.extend(piece.tick(time).drain(..).map::<Event, _>(|(name, value)| Event { sender: piece_name, name, value }))
  }
  process(state, time, links, events.drain(..))
}

/// Read a string value from a buffer
fn read_str(input: &mut InputBuffer) -> Result<String, ValueReadError> {
  rmp::decode::read_str(input, &mut Vec::new())
    .map_err(|err| match err {
      rmp::decode::DecodeStringError::InvalidMarkerRead(err) => ValueReadError::InvalidMarkerRead(err),
      rmp::decode::DecodeStringError::InvalidDataRead(err) => ValueReadError::InvalidDataRead(err),
      rmp::decode::DecodeStringError::TypeMismatch(marker) => ValueReadError::TypeMismatch(marker),
      DecodeStringError::BufferSizeTooSmall(_) => ValueReadError::TypeMismatch(rmp::Marker::Str8),
      DecodeStringError::InvalidUtf8(_, _) => ValueReadError::TypeMismatch(rmp::Marker::Str8),
    })
    .map(|s| s.to_string())
}

impl DecodeSaved for bool {
  fn read(input: &mut InputBuffer) -> Result<Self, rmp::decode::ValueReadError> {
    rmp::decode::read_bool(input)
  }
}

impl EncodeValue for bool {
  fn write(self: &Self, output: &mut OutputBuffer) -> SerializationResult {
    rmp::encode::write_bool(output, *self).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)
  }
}

impl ExtractList for bool {
  fn extract_list(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<&[bool]> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::BoolList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for bool {
  fn extract_value(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<bool> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::Bool(v) => Some(*v),
      _ => None,
    }
  }
}

impl DecodeSaved for u32 {
  fn read(input: &mut InputBuffer) -> Result<Self, ValueReadError> {
    rmp::decode::read_u32(input)
  }
}

impl EncodeValue for u32 {
  fn write(self: &Self, output: &mut OutputBuffer) -> SerializationResult {
    rmp::encode::write_u32(output, *self)
  }
}

impl ExtractList for u32 {
  fn extract_list(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<&[u32]> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::NumList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for u32 {
  fn extract_value(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<u32> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::Num(v) => Some(*v),
      _ => None,
    }
  }
}

impl DecodeSaved for puzzleverse_core::asset::rules::RealmLink {
  fn read(input: &mut InputBuffer) -> Result<Self, ValueReadError> {
    check_length(input, 2)?;
    match rmp::decode::read_u8(input)? {
      b'g' => {
        check_length(input, 2)?;
        Ok(puzzleverse_core::asset::rules::RealmLink::Global(read_str(input)?.into(), read_str(input)?.into()))
      }
      b's' => Ok(puzzleverse_core::asset::rules::RealmLink::Spawn(read_str(input)?.into())),
      b'o' => Ok(puzzleverse_core::asset::rules::RealmLink::Owner(read_str(input)?.into())),
      b'h' => Ok(puzzleverse_core::asset::rules::RealmLink::Home),
      b't' => Ok(puzzleverse_core::asset::rules::RealmLink::TrainNext),
      _ => Err(ValueReadError::TypeMismatch(rmp::Marker::Reserved)),
    }
  }
}

impl EncodeValue for puzzleverse_core::asset::rules::RealmLink {
  fn write(self: &Self, output: &mut OutputBuffer) -> SerializationResult {
    rmp::encode::write_array_len(output, 2)?;
    match self {
      puzzleverse_core::asset::rules::RealmLink::Global(realm, server) => {
        rmp::encode::write_u8(output, b'g')?;
        rmp::encode::write_array_len(output, 2)?;
        rmp::encode::write_str(output, realm)?;
        rmp::encode::write_str(output, server)
      }
      puzzleverse_core::asset::rules::RealmLink::Owner(name) => {
        rmp::encode::write_u8(output, b'o')?;
        rmp::encode::write_str(output, name)
      }
      puzzleverse_core::asset::rules::RealmLink::Spawn(point) => {
        rmp::encode::write_u8(output, b's')?;
        rmp::encode::write_str(output, point)
      }
      puzzleverse_core::asset::rules::RealmLink::Home => rmp::encode::write_u8(output, b'h'),
      puzzleverse_core::asset::rules::RealmLink::TrainNext => rmp::encode::write_u8(output, b't'),
    }
  }
}

impl ExtractList for puzzleverse_core::asset::rules::RealmLink {
  fn extract_list(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<&[puzzleverse_core::asset::rules::RealmLink]> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::RealmList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for puzzleverse_core::asset::rules::RealmLink {
  fn extract_value(value: &puzzleverse_core::asset::rules::PieceValue) -> Option<puzzleverse_core::asset::rules::RealmLink> {
    match value {
      puzzleverse_core::asset::rules::PieceValue::Realm(v) => Some(v.clone()),
      _ => None,
    }
  }
}
