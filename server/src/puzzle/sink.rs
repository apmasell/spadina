const SERIALIZATION_LENGTH: u32 = 1;
struct Sink<M: MultiHandler> {
  name: puzzleverse_core::PropertyKey,
  value: M::Value,
  multi: M,
}

pub enum SinkAsset {
  Bool(String),
  Int(String),
}
pub struct MultiSinkAsset<M>(pub String, pub M);

impl crate::puzzle::PuzzleAsset for SinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    match *self {
      SinkAsset::Bool(name) => Sink::new(name, BoolSingle),
      SinkAsset::Int(name) => Sink::new(name, NumSingle),
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    match *self {
      SinkAsset::Bool(name) => load(name, BoolSingle, input),
      SinkAsset::Int(name) => load(name, NumSingle, input),
    }
  }
}
impl<M: MultiHandler> crate::puzzle::PuzzleAsset for MultiSinkAsset<M> {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Sink::new(self.0, self.1)
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    load(self.0, self.1, input)
  }
}

pub(crate) trait SinkType: 'static + Default + Send + Sync + Clone + crate::puzzle::EncodeValue + crate::puzzle::ExtractValue {
  fn name(name: String) -> puzzleverse_core::PropertyKey;
  fn read(input: &mut crate::puzzle::InputBuffer) -> Result<Self, rmp::decode::ValueReadError>;
  fn value(&self) -> puzzleverse_core::PropertyValue;
}

impl SinkType for bool {
  fn name(name: String) -> puzzleverse_core::PropertyKey {
    puzzleverse_core::PropertyKey::BoolSink(name)
  }
  fn read(input: &mut crate::puzzle::InputBuffer) -> Result<Self, rmp::decode::ValueReadError> {
    rmp::decode::read_bool(input)
  }

  fn value(&self) -> puzzleverse_core::PropertyValue {
    puzzleverse_core::PropertyValue::Bool(*self)
  }
}

impl SinkType for u32 {
  fn name(name: String) -> puzzleverse_core::PropertyKey {
    puzzleverse_core::PropertyKey::NumSink(name)
  }
  fn read(input: &mut crate::puzzle::InputBuffer) -> Result<Self, rmp::decode::ValueReadError> {
    rmp::decode::read_u32(input)
  }
  fn value(&self) -> puzzleverse_core::PropertyValue {
    puzzleverse_core::PropertyValue::Num(*self)
  }
}

fn load<'a, M: MultiHandler>(name: String, multi: M, input: &mut crate::puzzle::InputBuffer) -> crate::puzzle::DeserializationResult<'a> {
  crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
  Ok(Box::new(Sink::<M> { name: M::Value::name(name), multi, value: M::Value::read(input)? }) as Box<dyn crate::puzzle::PuzzlePiece>)
}

impl<M> Sink<M>
where
  M: MultiHandler,
{
  fn new(name: String, multi: M) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Sink::<M> { name: M::Value::name(name), value: M::Value::default(), multi }) as Box<dyn crate::puzzle::PuzzlePiece>
  }
}

pub(crate) trait MultiHandler: Send + Sync + 'static {
  type Value: SinkType;
  fn create_multi(&self, active_states: &std::collections::BTreeSet<u8>, value: &Self::Value)
    -> crate::realm::Multi<puzzleverse_core::PropertyValue>;
}
struct BoolSingle;
impl MultiHandler for BoolSingle {
  type Value = bool;
  fn create_multi(&self, _: &std::collections::BTreeSet<u8>, value: &Self::Value) -> crate::realm::Multi<puzzleverse_core::PropertyValue> {
    crate::realm::Multi::Single(Self::Value::value(value))
  }
}
struct NumSingle;
impl MultiHandler for NumSingle {
  type Value = u32;
  fn create_multi(&self, _: &std::collections::BTreeSet<u8>, value: &Self::Value) -> crate::realm::Multi<puzzleverse_core::PropertyValue> {
    crate::realm::Multi::Single(Self::Value::value(value))
  }
}

impl<T: SinkType> MultiHandler for Vec<puzzleverse_core::asset::Mask<T>> {
  type Value = T;

  fn create_multi(
    &self,
    active_states: &std::collections::BTreeSet<u8>,
    value: &Self::Value,
  ) -> crate::realm::Multi<puzzleverse_core::PropertyValue> {
    let mut map = std::collections::BTreeMap::new();
    for mask in self {
      match mask {
        puzzleverse_core::asset::Mask::Marked(states, value) => {
          states.iter().copied().filter(|s| active_states.contains(s)).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          });
        }
        puzzleverse_core::asset::Mask::HasBit(bit, value) => match 1u8.checked_shl(*bit as u32) {
          Some(bit_mask) => active_states.iter().copied().filter(|s| *s & bit_mask != 0).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          }),
          None => (),
        },
        puzzleverse_core::asset::Mask::NotMarked(states, value) => {
          active_states.iter().copied().filter(|s| !states.contains(s)).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          });
        }
        puzzleverse_core::asset::Mask::HasNotBit(bit, value) => match 1u8.checked_shl(*bit as u32) {
          Some(bit_mask) => active_states.iter().copied().filter(|s| *s & bit_mask == 0).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          }),
          None => (),
        },
      }
    }

    crate::realm::Multi::Multi(T::value(value), map)
  }
}
impl<M> crate::puzzle::PuzzlePiece for Sink<M>
where
  M: MultiHandler,
{
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    if name == &puzzleverse_core::PuzzleCommand::Set {
      use crate::puzzle::ExtractValue;
      match M::Value::extract_value(value) {
        Some(v) => self.value = v,
        None => (),
      }
    }
    vec![]
  }
  fn interact(
    self: &mut Self,
    _: &puzzleverse_core::InteractionType,
    _: &str,
    _: Option<u8>,
  ) -> (puzzleverse_core::InteractionResult, crate::puzzle::SimpleOutputEvents) {
    (puzzleverse_core::InteractionResult::Invalid, vec![])
  }
  fn serialize(self: &Self, output: &mut crate::puzzle::OutputBuffer) -> crate::puzzle::SerializationResult {
    use crate::puzzle::EncodeValue;
    rmp::encode::write_array_len(output, SERIALIZATION_LENGTH)?;
    self.value.write(output)
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    None
  }
  fn reset(&self) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn update_check<'a>(self: &'a Self, active_states: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    Some(super::PuzzleConsequence(&self.name, self.multi.create_multi(active_states, &self.value)))
  }
  fn walk(
    self: &mut Self,
    _: &crate::PlayerKey,
    _: Option<u8>,
    _: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
}
