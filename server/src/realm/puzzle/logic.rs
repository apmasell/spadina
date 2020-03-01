use crate::realm::puzzle::ExtractValue;

struct Logic {
  left: bool,
  right: bool,
  operation: spadina_core::asset::puzzle::LogicOperation,
}

pub struct LogicAsset(pub spadina_core::asset::puzzle::LogicOperation);

impl crate::realm::puzzle::PuzzleAsset for LogicAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Logic { left: false, right: false, operation: self.0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (left, right) = serde_json::from_value(input)?;
    Ok(Box::new(Logic { left, right, operation: self.0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Logic {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let old_state = self.operation.perform(self.left, self.right);

    let update = bool::extract_value(value)
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

    let new_state = self.operation.perform(self.left, self.right);
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
    serde_json::to_value(&(self.left, self.right))
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
