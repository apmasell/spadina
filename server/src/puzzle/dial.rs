use crate::puzzle::PuzzlePiece;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct Dial {
  state: puzzleverse_core::Direction,
  start: puzzleverse_core::Direction,
  clockwise: bool,
  enabled: bool,
}

pub(crate) struct DialAsset {
  initial: puzzleverse_core::Direction,
  start: puzzleverse_core::Direction,
  clockwise: bool,
  enabled: bool,
}

impl crate::puzzle::PuzzleAsset for DialAsset {
  fn create(self: &Self, _: &chrono::DateTime<chrono::Utc>) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Dial { state: self.initial, start: self.start, clockwise: self.clockwise, enabled: self.enabled })
  }
  fn load<'a>(self: &Self, input: &mut crate::puzzle::InputBuffer, _: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Dial {
      state: rmp::decode::read_u32(input)?.into(),
      start: self.start,
      clockwise: self.clockwise,
      enabled: rmp::decode::read_bool(input)?,
    }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Dial {
  fn accept(self: &mut Self, name: &puzzleverse_core::PuzzleCommand, value: &crate::puzzle::PieceValue) -> crate::puzzle::OutputEvents {
    let (new_enabled, new_state) = match (name, value) {
      (puzzleverse_core::PuzzleCommand::Set, crate::puzzle::PieceValue::Num(v)) => (
        self.enabled,
        if self.clockwise { puzzleverse_core::Direction::from(*v) - self.start } else { self.start - puzzleverse_core::Direction::from(*v) },
      ),
      (puzzleverse_core::PuzzleCommand::Enable, crate::puzzle::PieceValue::Empty) => (true, self.state),
      (puzzleverse_core::PuzzleCommand::Disable, crate::puzzle::PieceValue::Empty) => (false, self.state),
      (puzzleverse_core::PuzzleCommand::Enable, crate::puzzle::PieceValue::Bool(v)) => (*v, self.state),
      _ => (self.enabled, self.state),
    };
    let mut results = Vec::new();

    if new_enabled != self.enabled {
      self.enabled = new_enabled;
      results.push(crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Sensitive, crate::puzzle::PieceValue::Bool(self.enabled)));
    }
    if new_state != self.state {
      self.state = new_state;
      results.push(crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, crate::puzzle::PieceValue::Num(self.state.into())));
    }
    results
  }
  fn interact(
    self: &mut Self,
    interaction: &puzzleverse_core::InteractionType,
  ) -> (puzzleverse_core::InteractionResult, crate::puzzle::SimpleOutputEvents) {
    match interaction {
      puzzleverse_core::InteractionType::Swiped(direction) => {
        if self.enabled {
          self.state = if self.clockwise { self.start + *direction } else { self.start - *direction }.into();
          (
            puzzleverse_core::InteractionResult::Accepted,
            vec![(puzzleverse_core::PuzzleEvent::Changed, crate::puzzle::PieceValue::Num(self.state.into()))],
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
    rmp::encode::write_u32(output, self.state.into())?;
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
  fn update_check<'a, 's>(self: &'s Self, state: &'a crate::puzzle::ConsequenceValueMatcher) -> Option<crate::puzzle::PuzzleConsequence<'a>> {
    state.apply(&u32::from(self.state).into())
  }
  fn walk(self: &mut Self, _: &crate::PlayerKey, _: crate::realm::navigation::PlayerNavigationEvent) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
}
