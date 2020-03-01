use crate::puzzle::PuzzlePiece;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct Switch {
  state: bool,
  enabled: bool,
  matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
}

pub(crate) struct SwitchAsset {
  pub initial: bool,
  pub enabled: bool,
  pub matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
}

impl crate::puzzle::PuzzleAsset for SwitchAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Switch { matcher: self.matcher, state: self.initial, enabled: self.enabled })
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Switch { matcher: self.matcher, state: rmp::decode::read_bool(input)?, enabled: rmp::decode::read_bool(input)? })
      as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Switch {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let (new_enabled, new_state) = match (name, value) {
      (puzzleverse_core::PuzzleCommand::Down, puzzleverse_core::asset::rules::PieceValue::Empty) => (self.enabled, true),
      (puzzleverse_core::PuzzleCommand::Up, puzzleverse_core::asset::rules::PieceValue::Empty) => (self.enabled, false),
      (puzzleverse_core::PuzzleCommand::Toggle, puzzleverse_core::asset::rules::PieceValue::Empty) => (self.enabled, !self.state),
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Empty) => (true, self.state),
      (puzzleverse_core::PuzzleCommand::Disable, puzzleverse_core::asset::rules::PieceValue::Empty) => (false, self.state),
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Bool(v)) => (self.enabled, *v),
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Bool(v)) => (*v, self.state),
      _ => (self.enabled, self.state),
    };
    let mut results = Vec::new();

    if new_enabled != self.enabled {
      self.enabled = new_enabled;
      results.push(crate::puzzle::OutputEvent::Event(
        puzzleverse_core::PuzzleEvent::Sensitive,
        puzzleverse_core::asset::rules::PieceValue::Bool(self.enabled),
      ));
    }
    if new_state != self.state {
      self.state = new_state;
      results.push(crate::puzzle::OutputEvent::Event(
        puzzleverse_core::PuzzleEvent::Changed,
        puzzleverse_core::asset::rules::PieceValue::Bool(self.state),
      ));
    }
    results
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
          self.state = !self.state;
          (
            puzzleverse_core::InteractionResult::Accepted,
            vec![(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Bool(self.state))],
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
    rmp::encode::write_bool(output, self.state).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)?;
    rmp::encode::write_bool(output, self.enabled).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)
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
