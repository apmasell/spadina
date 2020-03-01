struct Sink<M: MultiHandler> {
  name: spadina_core::realm::PropertyKey<crate::shstr::ShStr>,
  value: M::Value,
  multi: M,
}

pub enum SinkAsset {
  Bool(crate::shstr::ShStr),
  Int(crate::shstr::ShStr),
}
pub struct MultiSinkAsset<M>(pub crate::shstr::ShStr, pub M);

impl crate::realm::puzzle::PuzzleAsset for SinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    match *self {
      SinkAsset::Bool(name) => Sink::new(name, BoolSingle),
      SinkAsset::Int(name) => Sink::new(name, NumSingle),
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    match *self {
      SinkAsset::Bool(name) => load(name, BoolSingle, input),
      SinkAsset::Int(name) => load(name, NumSingle, input),
    }
  }
}
impl<M: MultiHandler> crate::realm::puzzle::PuzzleAsset for MultiSinkAsset<M> {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Sink::new(self.0, self.1)
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    load(self.0, self.1, input)
  }
}

pub(crate) trait SinkType:
  'static + Default + Send + Sync + Clone + serde::de::DeserializeOwned + serde::Serialize + crate::realm::puzzle::ExtractValue
{
  fn name(name: crate::shstr::ShStr) -> spadina_core::realm::PropertyKey<crate::shstr::ShStr>;
  fn value(&self) -> spadina_core::realm::PropertyValue;
}

impl SinkType for bool {
  fn name(name: crate::shstr::ShStr) -> spadina_core::realm::PropertyKey<crate::shstr::ShStr> {
    spadina_core::realm::PropertyKey::BoolSink(name)
  }

  fn value(&self) -> spadina_core::realm::PropertyValue {
    spadina_core::realm::PropertyValue::Bool(*self)
  }
}

impl SinkType for u32 {
  fn name(name: crate::shstr::ShStr) -> spadina_core::realm::PropertyKey<crate::shstr::ShStr> {
    spadina_core::realm::PropertyKey::NumSink(name)
  }
  fn value(&self) -> spadina_core::realm::PropertyValue {
    spadina_core::realm::PropertyValue::Num(*self)
  }
}

fn load<'a, M: MultiHandler>(name: crate::shstr::ShStr, multi: M, input: serde_json::Value) -> crate::realm::puzzle::DeserializationResult<'a> {
  Ok(Box::new(Sink::<M> { name: M::Value::name(name), multi, value: serde_json::from_value(input)? }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
}

impl<M> Sink<M>
where
  M: MultiHandler,
{
  fn new(name: crate::shstr::ShStr, multi: M) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Sink::<M> { name: M::Value::name(name), value: M::Value::default(), multi }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
}

pub(crate) trait MultiHandler: Send + Sync + 'static {
  type Value: SinkType;
  fn create_multi(
    &self,
    active_states: &std::collections::BTreeSet<u8>,
    value: &Self::Value,
  ) -> crate::realm::output::Multi<spadina_core::realm::PropertyValue>;
}
struct BoolSingle;
impl MultiHandler for BoolSingle {
  type Value = bool;
  fn create_multi(&self, _: &std::collections::BTreeSet<u8>, value: &Self::Value) -> crate::realm::output::Multi<spadina_core::realm::PropertyValue> {
    crate::realm::output::Multi::Single(Self::Value::value(value))
  }
}
struct NumSingle;
impl MultiHandler for NumSingle {
  type Value = u32;
  fn create_multi(&self, _: &std::collections::BTreeSet<u8>, value: &Self::Value) -> crate::realm::output::Multi<spadina_core::realm::PropertyValue> {
    crate::realm::output::Multi::Single(Self::Value::value(value))
  }
}

impl<T: SinkType> MultiHandler for Vec<spadina_core::asset::Mask<T>> {
  type Value = T;

  fn create_multi(
    &self,
    active_states: &std::collections::BTreeSet<u8>,
    value: &Self::Value,
  ) -> crate::realm::output::Multi<spadina_core::realm::PropertyValue> {
    let mut map = std::collections::BTreeMap::new();
    for mask in self {
      match mask {
        spadina_core::asset::Mask::Marked(states, value) => {
          states.iter().copied().filter(|s| active_states.contains(s)).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          });
        }
        spadina_core::asset::Mask::HasBit(bit, value) => match 1u8.checked_shl(*bit as u32) {
          Some(bit_mask) => active_states.iter().copied().filter(|s| *s & bit_mask != 0).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          }),
          None => (),
        },
        spadina_core::asset::Mask::NotMarked(states, value) => {
          active_states.iter().copied().filter(|s| !states.contains(s)).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          });
        }
        spadina_core::asset::Mask::HasNotBit(bit, value) => match 1u8.checked_shl(*bit as u32) {
          Some(bit_mask) => active_states.iter().copied().filter(|s| *s & bit_mask == 0).for_each(|s| {
            map.entry(s).or_insert(T::value(value));
          }),
          None => (),
        },
      }
    }

    crate::realm::output::Multi::Multi(T::value(value), map)
  }
}
impl<M> crate::realm::puzzle::PuzzlePiece for Sink<M>
where
  M: MultiHandler,
{
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    if name == &spadina_core::puzzle::PuzzleCommand::Set {
      use crate::realm::puzzle::ExtractValue;
      match M::Value::extract_value(value) {
        Some(v) => self.value = v,
        None => (),
      }
    }
    vec![]
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }
  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&self.value)
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    None
  }
  fn reset(&self) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn update_check<'a>(self: &'a Self, active_states: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    Some(super::PuzzleConsequence(&self.name, self.multi.create_multi(active_states, &self.value)))
  }
  fn walk(
    self: &mut Self,
    _: &crate::realm::puzzle::PlayerKey,
    _: Option<u8>,
    _: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
}
