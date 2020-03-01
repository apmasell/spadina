use crate::puzzle::PuzzlePiece;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct CycleButton {
  enabled: bool,
  matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  max: u32,
  value: u32,
}

pub(crate) struct CycleButtonAsset {
  pub enabled: bool,
  pub matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  pub max: u32,
}

impl crate::puzzle::PuzzleAsset for CycleButtonAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(CycleButton { enabled: self.enabled, matcher: self.matcher, max: self.max, value: 0 })
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(CycleButton { enabled: rmp::decode::read_bool(input)?, matcher: self.matcher, max: self.max, value: rmp::decode::read_u32(input)? })
      as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for CycleButton {
  fn accept<'a>(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let new_enabled = match (name, value) {
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Empty) => true,
      (puzzleverse_core::PuzzleCommand::Disable, puzzleverse_core::asset::rules::PieceValue::Empty) => false,
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Bool(v)) => *v,
      _ => self.enabled,
    };
    if new_enabled == self.enabled {
      vec![]
    } else {
      self.enabled = new_enabled;
      vec![crate::puzzle::OutputEvent::Event(
        puzzleverse_core::PuzzleEvent::Sensitive,
        puzzleverse_core::asset::rules::PieceValue::Bool(self.enabled),
      )]
    }
  }
  fn interact(
    self: &mut Self,
    interaction: &puzzleverse_core::InteractionType,
    _: &str,
    state: Option<u8>,
  ) -> (puzzleverse_core::InteractionResult, crate::puzzle::SimpleOutputEvents) {
    match interaction {
      puzzleverse_core::InteractionType::Click => {
        if self.enabled && self.matcher.matches(state) {
          self.value = self.value + 1 % (self.max + 1);
          (
            puzzleverse_core::InteractionResult::Accepted,
            vec![(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.value))],
          )
        } else {
          (puzzleverse_core::InteractionResult::Failed, vec![])
        }
      }
      _ => (puzzleverse_core::InteractionResult::Invalid, vec![]),
    }
  }

  fn serialize(self: &Self, output: &mut crate::puzzle::OutputBuffer) -> crate::puzzle::SerializationResult {
    rmp::encode::write_array_len(output, SERIALIZATION_LENGTH)?;
    rmp::encode::write_bool(output, self.enabled).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)?;
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
