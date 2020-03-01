use crate::puzzle::PuzzlePiece;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct RadioButton {
  matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  state: super::RadioSharedState,
  value: u32,
}

pub(crate) struct RadioButtonAsset {
  pub enabled: bool,
  pub initial: u32,
  pub matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  pub name: String,
  pub value: u32,
}

impl crate::puzzle::PuzzleAsset for RadioButtonAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(RadioButton {
      matcher: self.matcher,
      state: radio_states.entry(self.name.clone()).or_insert(std::sync::Arc::new((self.name, self.initial.into(), self.enabled.into()))).clone(),
      value: self.value,
    })
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    let current = rmp::decode::read_u32(input)?;
    let enabled = rmp::decode::read_bool(input)?;
    Ok(Box::new(RadioButton {
      matcher: self.matcher,
      state: radio_states.entry(self.name.clone()).or_insert(std::sync::Arc::new((self.name, current.into(), enabled.into()))).clone(),
      value: self.value,
    }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for RadioButton {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let mut results = Vec::new();
    if let Some(new_enabled) = match (name, value) {
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Empty) => Some(true),
      (puzzleverse_core::PuzzleCommand::Disable, puzzleverse_core::asset::rules::PieceValue::Empty) => Some(false),
      (puzzleverse_core::PuzzleCommand::Enable, puzzleverse_core::asset::rules::PieceValue::Bool(v)) => Some(*v),
      _ => None,
    } {
      if self.state.2.swap(new_enabled, std::sync::atomic::Ordering::Relaxed) != new_enabled {
        results.push(crate::puzzle::OutputEvent::Event(
          puzzleverse_core::PuzzleEvent::Sensitive,
          puzzleverse_core::asset::rules::PieceValue::Bool(new_enabled),
        ));
      }
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
        if self.matcher.matches(state)
          && self.state.2.load(std::sync::atomic::Ordering::Relaxed)
          && self.state.1.swap(self.value, std::sync::atomic::Ordering::Relaxed) != self.value
        {
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
    rmp::encode::write_u32(output, self.state.1.load(std::sync::atomic::Ordering::Relaxed))?;
    rmp::encode::write_bool(output, self.state.2.load(std::sync::atomic::Ordering::Relaxed)).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)
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
