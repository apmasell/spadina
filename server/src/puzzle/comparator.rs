use crate::puzzle::EncodeValue;
const SERIALIZATION_LENGTH: u32 = 2;

struct Comparator<T>
where
  T: Copy,
  T: PartialOrd,
  T: EncodeValue,
  T: Sync,
  T: crate::puzzle::ExtractValue,
{
  left: T,
  right: T,
  operation: puzzleverse_core::asset::puzzle::ComparatorOperation,
}

pub struct ComparatorAsset {
  pub operation: puzzleverse_core::asset::puzzle::ComparatorOperation,
  pub value_type: puzzleverse_core::asset::puzzle::ComparatorType,
}

impl crate::puzzle::PuzzleAsset for ComparatorAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    match self.value_type {
      puzzleverse_core::asset::puzzle::ComparatorType::Bool => {
        Box::new(Comparator { left: false, right: false, operation: self.operation }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ComparatorType::Int => {
        Box::new(Comparator { left: 0, right: 0, operation: self.operation }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    match self.value_type {
      puzzleverse_core::asset::puzzle::ComparatorType::Bool => load::<bool, crate::puzzle::InputBuffer>(input, self.operation),
      puzzleverse_core::asset::puzzle::ComparatorType::Int => load::<u32, crate::puzzle::InputBuffer>(input, self.operation),
    }
  }
}

fn load<'a, T, R>(
  input: &mut crate::puzzle::InputBuffer,
  operation: puzzleverse_core::asset::puzzle::ComparatorOperation,
) -> crate::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Copy,
  T: Send + Sync,
  Vec<T>: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::EncodeValue,
  T: crate::puzzle::DecodeSaved,
  T: PartialOrd,
  T: crate::puzzle::ExtractValue,
{
  crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
  Ok(Box::new(Comparator { left: T::read(input)?, right: T::read(input)?, operation }) as Box<dyn crate::puzzle::PuzzlePiece>)
}

impl<T> crate::puzzle::PuzzlePiece for Comparator<T>
where
  T: Copy,
  T: Send + Sync,
  T: PartialOrd,
  T: EncodeValue,
  T: crate::puzzle::ExtractValue,
{
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let old_state = self.operation.compare(self.left, self.right);

    let update = T::extract_value(value)
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

    let new_state = self.operation.compare(self.left, self.right);
    if update && old_state != new_state {
      vec![crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Bool(new_state))]
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
