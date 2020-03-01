use crate::puzzle::{EncodeValue, ExtractValue};

const SERIALIZATION_LENGTH: u32 = 2;

struct Arithmetic {
  left: u32,
  right: u32,
  operation: puzzleverse_core::asset::puzzle::ArithmeticOperation,
}

pub struct ArithmeticAsset(pub puzzleverse_core::asset::puzzle::ArithmeticOperation);

impl crate::puzzle::PuzzleAsset for ArithmeticAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Box::new(Arithmetic { left: 0, right: 0, operation: self.0 }) as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Arithmetic { left: rmp::decode::read_u32(input)?, right: rmp::decode::read_u32(input)?, operation: self.0 })
      as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}

impl crate::puzzle::PuzzlePiece for Arithmetic {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let old_state = self.operation.perform(self.left, self.right);

    let update = u32::extract_value(value)
      .map(|v| {
        if name == &puzzleverse_core::PuzzleCommand::SetLeft {
          self.left = v;
          true
        } else if name == &puzzleverse_core::PuzzleCommand::SetRight {
          self.right = v;
          true
        } else {
          false
        }
      })
      .unwrap_or_else(|| false);

    let new_state = self.operation.perform(self.left, self.right);
    if update && old_state != new_state {
      vec![crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(new_state))]
    } else {
      vec![]
    }
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
    rmp::encode::write_array_len(output, SERIALIZATION_LENGTH)?;
    self.left.write(output)?;
    self.right.write(output)
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

  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
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
