use serde::Deserialize;
use serde::Serialize;
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
