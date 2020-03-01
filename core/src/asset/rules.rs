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
pub enum PieceValue<S: AsRef<str>> {
  Empty,
  Bool(bool),
  Num(u32),
  Realm(LinkOut<S>),
  BoolList(Vec<bool>),
  NumList(Vec<u32>),
  RealmList(Vec<LinkOut<S>>),
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
pub struct PropagationRule<I, S: AsRef<str>> {
  pub sender: I,
  pub trigger: crate::puzzle::PuzzleEvent,
  pub recipient: I,
  pub causes: crate::puzzle::PuzzleCommand,
  pub propagation_match: PropagationValueMatcher<S>,
}
/// Determine how a change in the internal state of a puzzle piece should be used to change the state of other puzzle pieces.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum PropagationValueMatcher<S: AsRef<str>> {
  Unchanged,
  EmptyToBool { output: bool },
  EmptyToNum { output: u32 },
  EmptyToBoolList { output: Vec<bool> },
  EmptyToNumList { output: Vec<u32> },
  EmptyToGlobalRealm { asset: S, owner: S, server: S },
  EmptyToOwnerRealm { asset: S },
  EmptyToSettingRealm { setting: S },
  EmptyToSpawnPoint { name: S },
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkOut<S: AsRef<str>> {
  Realm(crate::realm::RealmTarget<S>),
  Spawn(S),
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

impl<S: AsRef<str> + Clone + std::cmp::Ord> PropagationValueMatcher<S> {
  /// Determine the value that should be sent to the next piece based on an event's value and a rule to modify them
  pub fn apply(
    self: &PropagationValueMatcher<S>,
    realm_owner: &S,
    input: &PieceValue<S>,
    settings: &crate::realm::RealmSettings<S>,
  ) -> Option<PieceValue<S>> {
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
      PropagationValueMatcher::EmptyToGlobalRealm { asset, owner, server } => match input {
        PieceValue::Empty => Some(PieceValue::Realm(LinkOut::Realm(crate::realm::RealmTarget::RemoteRealm {
          asset: asset.clone(),
          owner: owner.clone(),
          server: server.clone(),
        }))),
        _ => None,
      },
      PropagationValueMatcher::EmptyToOwnerRealm { asset } => match input {
        PieceValue::Empty => {
          Some(PieceValue::Realm(LinkOut::Realm(crate::realm::RealmTarget::LocalRealm { asset: asset.clone(), owner: realm_owner.clone() })))
        }
        _ => None,
      },
      PropagationValueMatcher::EmptyToSettingRealm { setting } => match input {
        PieceValue::Empty => match settings.get(setting) {
          Some(crate::realm::RealmSetting::Realm(realm)) => Some(realm.clone().into()),
          _ => None,
        },
        _ => None,
      },
      PropagationValueMatcher::EmptyToSpawnPoint { name } => match input {
        PieceValue::Empty => Some(PieceValue::Realm(LinkOut::Spawn(name.clone()))),
        _ => None,
      },
      PropagationValueMatcher::EmptyToTrainNext => match input {
        PieceValue::Empty => Some(PieceValue::Realm(LinkOut::TrainNext)),
        _ => None,
      },
      PropagationValueMatcher::EmptyToHome => match input {
        PieceValue::Empty => Some(PieceValue::Realm(LinkOut::Realm(crate::realm::RealmTarget::Home))),
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
  pub fn convert_str<T: AsRef<str> + Clone + std::cmp::Ord>(self) -> PropagationValueMatcher<T>
  where
    S: Into<T>,
  {
    match self {
      PropagationValueMatcher::Unchanged => PropagationValueMatcher::Unchanged,
      PropagationValueMatcher::EmptyToBool { output } => PropagationValueMatcher::EmptyToBool { output },
      PropagationValueMatcher::EmptyToNum { output } => PropagationValueMatcher::EmptyToNum { output },
      PropagationValueMatcher::EmptyToBoolList { output } => PropagationValueMatcher::EmptyToBoolList { output },
      PropagationValueMatcher::EmptyToNumList { output } => PropagationValueMatcher::EmptyToNumList { output },
      PropagationValueMatcher::EmptyToGlobalRealm { asset, owner, server } => {
        PropagationValueMatcher::EmptyToGlobalRealm { asset: asset.into(), owner: owner.into(), server: server.into() }
      }
      PropagationValueMatcher::EmptyToOwnerRealm { asset } => PropagationValueMatcher::EmptyToOwnerRealm { asset: asset.into() },
      PropagationValueMatcher::EmptyToSettingRealm { setting } => PropagationValueMatcher::EmptyToSettingRealm { setting: setting.into() },
      PropagationValueMatcher::EmptyToSpawnPoint { name } => PropagationValueMatcher::EmptyToSpawnPoint { name: name.into() },
      PropagationValueMatcher::EmptyToTrainNext => PropagationValueMatcher::EmptyToTrainNext,
      PropagationValueMatcher::EmptyToHome => PropagationValueMatcher::EmptyToHome,
      PropagationValueMatcher::BoolInvert => PropagationValueMatcher::BoolInvert,
      PropagationValueMatcher::BoolEmpty { input } => PropagationValueMatcher::BoolEmpty { input },
      PropagationValueMatcher::BoolToNum { input, output } => PropagationValueMatcher::BoolToNum { input, output },
      PropagationValueMatcher::BoolToNumList { input, output } => PropagationValueMatcher::BoolToNumList { input, output },
      PropagationValueMatcher::BoolToBoolList { input, output } => PropagationValueMatcher::BoolToBoolList { input, output },
      PropagationValueMatcher::NumToEmpty { input, comparison } => PropagationValueMatcher::NumToEmpty { input, comparison },
      PropagationValueMatcher::NumToBool { input, comparison } => PropagationValueMatcher::NumToBool { input, comparison },
      PropagationValueMatcher::NumToBoolList { bits, low_to_high } => PropagationValueMatcher::NumToBoolList { bits, low_to_high },
      PropagationValueMatcher::AnyToEmpty => PropagationValueMatcher::AnyToEmpty,
    }
  }
}
impl<S: AsRef<str>> From<bool> for PieceValue<S> {
  fn from(value: bool) -> Self {
    PieceValue::Bool(value)
  }
}

impl<S: AsRef<str>> From<Vec<bool>> for PieceValue<S> {
  fn from(value: Vec<bool>) -> Self {
    PieceValue::BoolList(value)
  }
}
impl<S: AsRef<str>> From<LinkOut<S>> for PieceValue<S> {
  fn from(value: LinkOut<S>) -> Self {
    PieceValue::Realm(value)
  }
}
impl<S: AsRef<str>> From<Vec<LinkOut<S>>> for PieceValue<S> {
  fn from(value: Vec<LinkOut<S>>) -> Self {
    PieceValue::RealmList(value)
  }
}
impl<S: AsRef<str>> From<crate::realm::RealmTarget<S>> for PieceValue<S> {
  fn from(value: crate::realm::RealmTarget<S>) -> Self {
    PieceValue::Realm(LinkOut::Realm(value))
  }
}
impl<S: AsRef<str>> From<u32> for PieceValue<S> {
  fn from(value: u32) -> Self {
    PieceValue::Num(value)
  }
}

impl<S: AsRef<str>> From<Vec<u32>> for PieceValue<S> {
  fn from(value: Vec<u32>) -> Self {
    PieceValue::NumList(value)
  }
}
