use crate::realm::puzzle::PuzzlePiece;

pub(crate) struct Counter {
  value: u32,
  max: u32,
}

pub(crate) struct CounterAsset {
  pub max: u32,
}

impl crate::realm::puzzle::PuzzleAsset for CounterAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Counter { value: 0, max: self.max })
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let value = serde_json::from_value(input)?;
    Ok(Box::new(Counter { value: std::cmp::max(0, std::cmp::min(value, self.max)), max: self.max }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Counter {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let new_value = match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Up, spadina_core::asset::rules::PieceValue::Empty) => {
        if self.value == self.max {
          self.value
        } else {
          self.value + 1
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Down, spadina_core::asset::rules::PieceValue::Empty) => {
        if self.value == 0 {
          self.value
        } else {
          self.value - 1
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Up, spadina_core::asset::rules::PieceValue::Num(delta)) => {
        if self.value > self.max - delta {
          self.max
        } else {
          self.value + delta
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Down, spadina_core::asset::rules::PieceValue::Num(delta)) => {
        if self.value < *delta {
          0
        } else {
          self.value - delta
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Empty) => 0,
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Num(v)) => *v,
      _ => self.value,
    };

    if new_value == self.value {
      vec![]
    } else {
      self.value = new_value;
      if self.value == 0 {
        vec![
          crate::realm::puzzle::OutputEvent::Event(
            spadina_core::puzzle::PuzzleEvent::Changed,
            spadina_core::asset::rules::PieceValue::Num(self.value),
          ),
          crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::AtMin, spadina_core::asset::rules::PieceValue::Empty),
        ]
      } else if self.value == self.max {
        vec![
          crate::realm::puzzle::OutputEvent::Event(
            spadina_core::puzzle::PuzzleEvent::Changed,
            spadina_core::asset::rules::PieceValue::Num(self.value),
          ),
          crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::AtMax, spadina_core::asset::rules::PieceValue::Empty),
        ]
      } else {
        vec![crate::realm::puzzle::OutputEvent::Event(
          spadina_core::puzzle::PuzzleEvent::Changed,
          spadina_core::asset::rules::PieceValue::Num(self.value),
        )]
      }
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
    serde_json::to_value(self.value)
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
