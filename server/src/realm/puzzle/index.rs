use spadina_core::asset::rules::LinkOut;

pub(crate) struct Index<T> {
  items: Vec<T>,
  selected: u32,
}

pub(crate) struct IndexAsset(spadina_core::asset::puzzle::ListType);

impl crate::realm::puzzle::PuzzleAsset for IndexAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    match self.0 {
      spadina_core::asset::puzzle::ListType::Bool => {
        Box::new(Index::<bool> { items: Vec::new(), selected: 0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
      spadina_core::asset::puzzle::ListType::Int => {
        Box::new(Index::<u32> { items: Vec::new(), selected: 0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
      spadina_core::asset::puzzle::ListType::Realm => {
        Box::new(Index::<LinkOut<crate::shstr::ShStr>> { items: Vec::new(), selected: 0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    match self.0 {
      spadina_core::asset::puzzle::ListType::Bool => load::<bool>(input),
      spadina_core::asset::puzzle::ListType::Int => load::<u32>(input),
      spadina_core::asset::puzzle::ListType::Realm => load::<LinkOut<crate::shstr::ShStr>>(input),
    }
  }
}
fn load<'a, T>(input: serde_json::Value) -> crate::realm::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Clone,
  T: Send + Sync,
  T: serde::de::DeserializeOwned,
  T: serde::Serialize,
  T: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: crate::realm::puzzle::ExtractList,
  T: crate::realm::puzzle::ExtractValue,
{
  let (items, selected) = serde_json::from_value(input)?;
  Ok(Box::new(Index::<T> { items, selected }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
}

impl<T> crate::realm::puzzle::PuzzlePiece for Index<T>
where
  T: Clone,
  T: Send + Sync,
  T: serde::de::DeserializeOwned,
  T: serde::Serialize,
  T: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: crate::realm::puzzle::ExtractList,
  T: crate::realm::puzzle::ExtractValue,
{
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let changed = match name {
      spadina_core::puzzle::PuzzleCommand::Insert => match T::extract_list(value) {
        Some(v) => {
          self.items = v.iter().cloned().collect();
          true
        }
        None => false,
      },
      spadina_core::puzzle::PuzzleCommand::Set => match value {
        spadina_core::asset::rules::PieceValue::Num(s) => {
          self.selected = *s;
          true
        }
        _ => false,
      },
      _ => false,
    };

    if changed {
      vec![crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Changed,
        self.items.get(self.selected as usize).map(|v| v.clone().into()).unwrap_or(spadina_core::asset::rules::PieceValue::Empty),
      )]
    } else {
      vec![]
    }
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&(&self.items, self.selected))
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
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
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
