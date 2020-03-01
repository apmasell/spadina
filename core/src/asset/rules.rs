/// Comparison logic between different values
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum Comparator {
  NotEqual,
  Equal,
  LessThanOrEqual,
  LessThan,
  GreaterThanOrEqual,
  GreaterThan,
  Congruent(u32),
}
/// A value associated with an event
#[derive(Clone, Debug, PartialEq)]
pub enum PieceValue {
  Empty,
  Bool(bool),
  Num(u32),
  Realm(RealmLink),
  BoolList(Vec<bool>),
  NumList(Vec<u32>),
  RealmList(Vec<RealmLink>),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum PlayerMarkMatcher {
  Any,
  Unmarked,
  Marked(Vec<u8>, bool),
  HasBit(u8, bool),
  NotMarked(Vec<u8>, bool),
  HasNotBit(u8, bool),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PropagationRule<I> {
  pub sender: I,
  pub trigger: crate::PuzzleEvent,
  pub recipient: I,
  pub causes: crate::PuzzleCommand,
  pub propagation_match: PropagationValueMatcher,
}
/// Determine how a change in the internal state of a puzzle piece should be used to change the state of other puzzle pieces.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum PropagationValueMatcher {
  Unchanged,
  EmptyToBool { output: bool },
  EmptyToNum { output: u32 },
  EmptyToBoolList { output: Vec<bool> },
  EmptyToNumList { output: Vec<u32> },
  EmptyToGlobalRealm { realm: String, server: String },
  EmptyToOwnerRealm { asset: String },
  EmptyToSettingRealm { setting: String },
  EmptyToSpawnPoint { name: String },
  EmptyToTrainNext,
  EmptyToHome,
  BoolInvert,
  BoolEmpty { input: bool },
  BoolToNum { input: bool, output: u32 },
  BoolToNumList { input: bool, output: Vec<u32> },
  BoolToBoolList { input: bool, output: Vec<bool> },
  NumToEmpty { input: u32, comparison: Comparator },
  NumToBool { input: u32, comparison: Comparator },
  NumToBoolList { bits: u32, low_to_high: bool },
  AnyToEmpty,
}

/// A realm or spawn point that a player should  be sent to
#[derive(Clone, Debug, PartialEq)]
pub enum RealmLink {
  Global(String, String),
  Owner(String),
  Spawn(String),
  Home,
  TrainNext,
}

impl Comparator {
  /// Compare two integer values
  pub fn compare(self: &Comparator, source: u32, reference: u32) -> bool {
    match self {
      Comparator::NotEqual => source != reference,
      Comparator::Equal => source == reference,
      Comparator::LessThanOrEqual => source <= reference,
      Comparator::LessThan => source < reference,
      Comparator::GreaterThanOrEqual => source >= reference,
      Comparator::GreaterThan => source > reference,
      Comparator::Congruent(by) => (source % by) == (reference % by),
    }
  }
}

impl PlayerMarkMatcher {
  pub fn matches(&self, state: Option<u8>) -> bool {
    match self {
      PlayerMarkMatcher::Any => true,
      PlayerMarkMatcher::Unmarked => state.is_none(),
      PlayerMarkMatcher::Marked(allowed, when_unmarked) => match state {
        None => *when_unmarked,
        Some(state) => allowed.iter().any(|&s| s == state),
      },
      PlayerMarkMatcher::NotMarked(disallowed, when_unmarked) => match state {
        None => *when_unmarked,
        Some(state) => !disallowed.iter().any(|&s| s == state),
      },
      PlayerMarkMatcher::HasBit(bit, when_unmarked) => match state {
        None => *when_unmarked,
        Some(state) => 1_u8.checked_shl(*bit as u32).map(|mask| mask & state != 0).unwrap_or(*when_unmarked),
      },
      PlayerMarkMatcher::HasNotBit(bit, when_unmarked) => match state {
        None => *when_unmarked,
        Some(state) => 1_u8.checked_shl(*bit as u32).map(|mask| mask & state == 0).unwrap_or(*when_unmarked),
      },
    }
  }
}

impl PropagationValueMatcher {
  /// Determine the value that should be sent to the next piece based on an event's value and a rule to modify them
  pub fn apply(
    self: &PropagationValueMatcher,
    input: &PieceValue,
    settings: &std::collections::BTreeMap<String, crate::RealmSetting>,
  ) -> Option<PieceValue> {
    match self {
      PropagationValueMatcher::Unchanged => Some(input.clone()),
      PropagationValueMatcher::EmptyToBool { output } => match input {
        PieceValue::Empty => Some(PieceValue::Bool(*output)),
        _ => None,
      },
      PropagationValueMatcher::EmptyToNum { output } => match input {
        PieceValue::Empty => Some(PieceValue::Num(*output)),
        _ => None,
      },
      PropagationValueMatcher::EmptyToBoolList { output } => match input {
        PieceValue::Empty => Some(PieceValue::BoolList(output.clone())),
        _ => None,
      },
      PropagationValueMatcher::EmptyToNumList { output } => match input {
        PieceValue::Empty => Some(PieceValue::NumList(output.clone())),
        _ => None,
      },
      PropagationValueMatcher::EmptyToGlobalRealm { realm, server } => match input {
        PieceValue::Empty => Some(PieceValue::Realm(RealmLink::Global(realm.clone(), server.clone()))),
        _ => None,
      },
      PropagationValueMatcher::EmptyToOwnerRealm { asset } => match input {
        PieceValue::Empty => Some(PieceValue::Realm(RealmLink::Owner(asset.clone()))),
        _ => None,
      },
      PropagationValueMatcher::EmptyToSettingRealm { setting } => match input {
        PieceValue::Empty => match settings.get(setting) {
          Some(crate::RealmSetting::Realm(realm)) => Some(realm.clone().into()),
          _ => None,
        },
        _ => None,
      },
      PropagationValueMatcher::EmptyToSpawnPoint { name } => match input {
        PieceValue::Empty => Some(PieceValue::Realm(RealmLink::Spawn(name.clone()))),
        _ => None,
      },
      PropagationValueMatcher::EmptyToTrainNext => match input {
        PieceValue::Empty => Some(PieceValue::Realm(RealmLink::TrainNext)),
        _ => None,
      },
      PropagationValueMatcher::EmptyToHome => match input {
        PieceValue::Empty => Some(PieceValue::Realm(RealmLink::Home)),
        _ => None,
      },
      PropagationValueMatcher::BoolInvert => match input {
        PieceValue::Bool(input_boolean) => Some(PieceValue::Bool(!*input_boolean)),
        _ => None,
      },
      PropagationValueMatcher::BoolEmpty { input: reference } => match input {
        PieceValue::Bool(input_boolean) => {
          if input_boolean == reference {
            Some(PieceValue::Empty)
          } else {
            None
          }
        }
        _ => None,
      },
      PropagationValueMatcher::BoolToNum { input: reference, output } => match input {
        PieceValue::Bool(input_boolean) => {
          if input_boolean == reference {
            Some(PieceValue::Num(*output))
          } else {
            None
          }
        }
        _ => None,
      },
      PropagationValueMatcher::BoolToNumList { input: reference, output } => match input {
        PieceValue::Bool(input_boolean) => {
          if *input_boolean == *reference {
            Some(PieceValue::NumList(output.clone()))
          } else {
            None
          }
        }
        _ => None,
      },
      PropagationValueMatcher::BoolToBoolList { input: reference, output } => match input {
        PieceValue::Bool(input_boolean) => {
          if *input_boolean == *reference {
            Some(PieceValue::BoolList(output.clone()))
          } else {
            None
          }
        }
        _ => None,
      },
      PropagationValueMatcher::NumToEmpty { input: reference, comparison } => match input {
        PieceValue::Num(input_int) => {
          if comparison.compare(*input_int, *reference) {
            Some(PieceValue::Empty)
          } else {
            None
          }
        }
        _ => None,
      },
      PropagationValueMatcher::NumToBool { input: reference, comparison } => match input {
        PieceValue::Num(input_int) => Some(PieceValue::Bool(comparison.compare(*input_int, *reference))),
        _ => None,
      },
      PropagationValueMatcher::NumToBoolList { bits, low_to_high } => match input {
        PieceValue::Num(input_int) => Some(PieceValue::BoolList(if *low_to_high {
          (0..*bits).map(|i| *input_int & (1 << i) == 1).collect()
        } else {
          (0..*bits).rev().map(|i| *input_int & (1 << i) == 1).collect()
        })),
        _ => None,
      },
      PropagationValueMatcher::AnyToEmpty => Some(PieceValue::Empty),
    }
  }
}
impl Into<PieceValue> for bool {
  fn into(self: Self) -> PieceValue {
    PieceValue::Bool(self)
  }
}

impl Into<PieceValue> for Vec<bool> {
  fn into(self: Self) -> PieceValue {
    PieceValue::BoolList(self)
  }
}
impl Into<PieceValue> for RealmLink {
  fn into(self: Self) -> PieceValue {
    PieceValue::Realm(self)
  }
}
impl Into<PieceValue> for Vec<RealmLink> {
  fn into(self: Self) -> PieceValue {
    PieceValue::RealmList(self)
  }
}
impl Into<PieceValue> for crate::RealmSettingLink {
  fn into(self) -> PieceValue {
    PieceValue::Realm(match self {
      crate::RealmSettingLink::Home => RealmLink::Home,
      crate::RealmSettingLink::Owner(id) => RealmLink::Owner(id),
      crate::RealmSettingLink::Global(realm, server) => RealmLink::Global(realm, server),
    })
  }
}
impl Into<PieceValue> for u32 {
  fn into(self: Self) -> PieceValue {
    PieceValue::Num(self)
  }
}

impl Into<PieceValue> for Vec<u32> {
  fn into(self: Self) -> PieceValue {
    PieceValue::NumList(self)
  }
}
