use crate::puzzle::PuzzlePiece;
const SERIALIZATION_LENGTH: u32 = 1;

pub(crate) struct Counter {
  value: u32,
  max: u32,
}

pub(crate) struct CounterAsset {
  pub max: u32,
}

impl crate::puzzle::PuzzleAsset for CounterAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Counter { value: 0, max: self.max })
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    rmp::decode::read_u32(input)
      .map(|v| Box::new(Counter { value: std::cmp::max(0, std::cmp::min(v, self.max)), max: self.max }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Counter {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let new_value = match (name, value) {
      (puzzleverse_core::PuzzleCommand::Up, puzzleverse_core::asset::rules::PieceValue::Empty) => {
        if self.value == self.max {
          self.value
        } else {
          self.value + 1
        }
      }
      (puzzleverse_core::PuzzleCommand::Down, puzzleverse_core::asset::rules::PieceValue::Empty) => {
        if self.value == 0 {
          self.value
        } else {
          self.value - 1
        }
      }
      (puzzleverse_core::PuzzleCommand::Up, puzzleverse_core::asset::rules::PieceValue::Num(delta)) => {
        if self.value > self.max - delta {
          self.max
        } else {
          self.value + delta
        }
      }
      (puzzleverse_core::PuzzleCommand::Down, puzzleverse_core::asset::rules::PieceValue::Num(delta)) => {
        if self.value < *delta {
          0
        } else {
          self.value - delta
        }
      }
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Empty) => 0,
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Num(v)) => *v,
      _ => self.value,
    };

    if new_value == self.value {
      vec![]
    } else {
      self.value = new_value;
      if self.value == 0 {
        vec![
          crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.value)),
          crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::AtMin, puzzleverse_core::asset::rules::PieceValue::Empty),
        ]
      } else if self.value == self.max {
        vec![
          crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.value)),
          crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::AtMax, puzzleverse_core::asset::rules::PieceValue::Empty),
        ]
      } else {
        vec![crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.value))]
      }
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
    rmp::encode::write_u32(output, self.value)
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
