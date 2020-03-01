struct Comparator<T>
where
  T: Copy,
  T: PartialOrd,
  T: serde::Serialize,
  T: Sync,
  T: crate::realm::puzzle::ExtractValue,
{
  left: T,
  right: T,
  operation: spadina_core::asset::puzzle::ComparatorOperation,
}

pub struct ComparatorAsset {
  pub operation: spadina_core::asset::puzzle::ComparatorOperation,
  pub value_type: spadina_core::asset::puzzle::ComparatorType,
}

impl crate::realm::puzzle::PuzzleAsset for ComparatorAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    match self.value_type {
      spadina_core::asset::puzzle::ComparatorType::Bool => {
        Box::new(Comparator { left: false, right: false, operation: self.operation }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
      spadina_core::asset::puzzle::ComparatorType::Int => {
        Box::new(Comparator { left: 0, right: 0, operation: self.operation }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    match self.value_type {
      spadina_core::asset::puzzle::ComparatorType::Bool => load::<bool>(input, self.operation),
      spadina_core::asset::puzzle::ComparatorType::Int => load::<u32>(input, self.operation),
    }
  }
}

fn load<'a, T>(
  input: serde_json::Value,
  operation: spadina_core::asset::puzzle::ComparatorOperation,
) -> crate::realm::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Copy,
  T: Send + Sync,
  Vec<T>: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: serde::de::DeserializeOwned,
  T: serde::Serialize,
  T: PartialOrd,
  T: crate::realm::puzzle::ExtractValue,
{
  let (left, right) = serde_json::from_value(input)?;
  Ok(Box::new(Comparator::<T> { left, right, operation }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
}

impl<T> crate::realm::puzzle::PuzzlePiece for Comparator<T>
where
  T: Copy,
  T: Send + Sync,
  T: PartialOrd,
  T: serde::Serialize,
  T: crate::realm::puzzle::ExtractValue,
{
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let old_state = self.operation.compare(self.left, self.right);

    let update = T::extract_value(value)
      .map(|v| {
        if name == &spadina_core::puzzle::PuzzleCommand::SetLeft {
          self.left = v;
          true
        } else if name == &spadina_core::puzzle::PuzzleCommand::SetRight {
          self.right = v;
          true
        } else {
          false
        }
      })
      .unwrap_or_else(|| false);

    let new_state = self.operation.compare(self.left, self.right);
    if update && old_state != new_state {
      vec![crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Changed,
        spadina_core::asset::rules::PieceValue::Bool(new_state),
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
    serde_json::to_value(&(&self.left, &self.right))
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
