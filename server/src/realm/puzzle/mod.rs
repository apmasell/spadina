pub mod arithmetic;
pub mod buffer;
pub mod button;
pub mod clock;
pub mod comparator;
pub mod counter;
pub mod cycle_button;
pub mod event_sink;
pub mod holiday;
pub mod index;
pub mod index_list;
pub mod logic;
pub mod map_sink;
pub mod metronome;
pub mod permutation;
pub mod proximity;
pub mod radio_button;
pub mod realm_selector;
pub mod sink;
pub mod switch;
pub mod timer;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct PlayerKey(pub u64);

type DeserializationResult<'a> = Result<Box<dyn PuzzlePiece + 'a>, serde_json::Error>;

/// An event created by a puzzle piece updating its state
#[derive(Debug, Clone)]
pub(crate) struct Event {
  /// The piece that triggered the event
  pub(crate) sender: usize,
  /// The event name
  pub(crate) name: spadina_core::puzzle::PuzzleEvent,
  /// The output value associated with the event
  pub(crate) value: spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
}

/// A type which can be converted from a puzzle piece value
pub(crate) trait ExtractValue: Sized {
  /// Try to convert the value from a puzzle piece value
  fn extract_value(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<Self>;
}

/// A type where a list of values can be converted from a puzzle piece value
pub(crate) trait ExtractList: Sized {
  /// Try to convert a list of value from a puzzle piece value
  fn extract_list(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<&[Self]>;
}

pub(crate) enum OutputEvent {
  Event(spadina_core::puzzle::PuzzleEvent, spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>),
  BitClear(u8, Vec<PlayerKey>),
  BitSet(u8, Vec<PlayerKey>),
  BitToggle(u8, Vec<PlayerKey>),
  Mark(u8, Vec<PlayerKey>),
  Send(spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>, Vec<PlayerKey>),
  Unmark(Vec<PlayerKey>),
}

pub(crate) type OutputEvents = Vec<OutputEvent>;

/// A prototype for a puzzle piece that can either produce a blank state for a new realm or recover state from a database
pub(crate) trait PuzzleAsset {
  /// Create a fresh new state
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<crate::shstr::ShStr, RadioSharedState>,
  ) -> Box<dyn PuzzlePiece>;
  /// Recover previously serialised state
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    time: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<crate::shstr::ShStr, RadioSharedState>,
  ) -> DeserializationResult<'a>;
}

pub type RadioSharedState = std::sync::Arc<(crate::shstr::ShStr, std::sync::atomic::AtomicU32, std::sync::atomic::AtomicBool)>;

/// The event that will have a consequence event
pub struct PuzzleConsequence<'a>(
  &'a spadina_core::realm::PropertyKey<crate::shstr::ShStr>,
  crate::realm::output::Multi<spadina_core::realm::PropertyValue>,
);

/// An active puzzle component in a realm's logic
pub(crate) trait PuzzlePiece: Send + Sync {
  /// Adjust state based on the activity of another puzzle piece and the realm author's rules and generate any output changes the author may want to use
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    time: &chrono::DateTime<chrono::Utc>,
  ) -> OutputEvents;
  /// Respond to a player interacting with this puzzle piece via the UI.
  ///
  /// Most puzzle pieces do not directly interact with players.
  fn interact(
    self: &mut Self,
    interaction: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    state: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, SimpleOutputEvents);
  /// Write this puzzle piece out to a format that can be recovered later
  fn serialize(self: &Self) -> SerializationResult;
  /// Generate any events trigger purely by time events
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> SimpleOutputEvents;
  /// Determine the next time when this piece _could_ update, if it responds to clock changes
  fn next(self: &Self) -> Option<std::time::Duration>;
  /// Generate events when thawed from database
  fn reset(&self) -> SimpleOutputEvents;
  /// Determine if the current state of this piece should trigger any client state changes.
  ///
  /// Many pieces do not directly modify the player environment
  fn update_check<'a>(self: &'a Self, active_states: &std::collections::BTreeSet<u8>) -> Option<PuzzleConsequence<'a>>;
  /// Respond to a player entering or exiting the sphere of influence a puzzle piece
  ///
  /// Most puzzle pieces do not directly interact with player movements.
  fn walk(self: &mut Self, player: &PlayerKey, state: Option<u8>, event: crate::realm::navigation::PlayerNavigationEvent) -> SimpleOutputEvents;
}

type SerializationResult = Result<serde_json::Value, serde_json::Error>;

type SimpleEvent = (spadina_core::puzzle::PuzzleEvent, spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>);
pub(crate) type SimpleOutputEvents = Vec<SimpleEvent>;

impl Event {
  /// Create a new event generated by a puzzle piece
  pub(crate) fn new(sender: usize, event: SimpleEvent) -> Event {
    Event { sender, name: event.0, value: event.1 }
  }
}
/// Update a realm's state from puzzle pieces
pub(crate) fn prepare_consequences(state: &mut crate::realm::Realm) -> bool {
  let mut changed = false;
  let active_states = state.active_players.values().flat_map(|active_player| active_player.state.iter()).copied().collect();
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
  state: &mut crate::realm::Realm,
  time: &chrono::DateTime<chrono::Utc>,
  links: &mut std::collections::HashMap<PlayerKey, spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>>,
  events: T,
) {
  let owner = crate::shstr::ShStr::Shared(state.owner.clone());
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
          if let Some(result) = propagation_rule.propagation_match.apply(&owner, &current.value, state.settings.read()).as_mut() {
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
                    if let Some(super::ActivePlayer { state, .. }) = state.active_players.get_mut(&player) {
                      *state = Some(s);
                    }
                  }
                }
                OutputEvent::Unmark(players) => {
                  for player in players {
                    if let Some(super::ActivePlayer { state, .. }) = state.active_players.get_mut(&player) {
                      *state = None;
                    }
                  }
                }
                OutputEvent::BitClear(bit, players) => {
                  if let Some(mask) = 1_u8.checked_shl(bit as u32) {
                    for player in players {
                      if let Some(super::ActivePlayer { state: Some(state), .. }) = state.active_players.get_mut(&player) {
                        *state &= !mask;
                      }
                    }
                  }
                }
                OutputEvent::BitSet(bit, players) => {
                  if let Some(mask) = 1_u8.checked_shl(bit as u32) {
                    for player in players {
                      if let Some(super::ActivePlayer { state, .. }) = state.active_players.get_mut(&player) {
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
                      if let Some(super::ActivePlayer { state, .. }) = state.active_players.get_mut(&player) {
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
              .walk(
                &player,
                state.active_players.get(&player).map(|active_player| active_player.state.clone()).flatten(),
                crate::realm::navigation::PlayerNavigationEvent::Leave,
              )
              .into_iter()
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

impl ExtractList for bool {
  fn extract_list(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<&[bool]> {
    match value {
      spadina_core::asset::rules::PieceValue::BoolList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for bool {
  fn extract_value(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<bool> {
    match value {
      spadina_core::asset::rules::PieceValue::Bool(v) => Some(*v),
      _ => None,
    }
  }
}

impl ExtractList for u32 {
  fn extract_list(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<&[u32]> {
    match value {
      spadina_core::asset::rules::PieceValue::NumList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for u32 {
  fn extract_value(value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>) -> Option<u32> {
    match value {
      spadina_core::asset::rules::PieceValue::Num(v) => Some(*v),
      _ => None,
    }
  }
}

impl ExtractList for spadina_core::asset::rules::LinkOut<crate::shstr::ShStr> {
  fn extract_list(
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
  ) -> Option<&[spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>]> {
    match value {
      spadina_core::asset::rules::PieceValue::RealmList(v) => Some(v),
      _ => None,
    }
  }
}

impl ExtractValue for spadina_core::asset::rules::LinkOut<crate::shstr::ShStr> {
  fn extract_value(
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
  ) -> Option<spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>> {
    match value {
      spadina_core::asset::rules::PieceValue::Realm(v) => Some(v.clone()),
      _ => None,
    }
  }
}
