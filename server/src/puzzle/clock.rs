const SERIALIZATION_LENGTH: u32 = 2;
struct Clock {
  period: u32,
  shift: u32,
  max: u32,
  last: chrono::DateTime<chrono::Utc>,
}

pub struct ClockAsset {
  pub period: u32,
  pub max: u32,
  pub shift: Option<u32>,
}

impl crate::puzzle::PuzzleAsset for ClockAsset {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Box::new(Clock { period: self.period, shift: self.shift.unwrap_or((time.timestamp() % self.period as i64) as u32), max: self.max, last: *time })
      as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Clock { period: rmp::decode::read_u32(input)?, shift: rmp::decode::read_u32(input)?, max: self.max, last: *time })
      as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}

impl crate::puzzle::PuzzlePiece for Clock {
  fn accept<'a>(
    self: &mut Self,
    _: &puzzleverse_core::PuzzleCommand,
    _: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    vec![]
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
    rmp::encode::write_u32(output, self.period)?;
    rmp::encode::write_u32(output, self.shift)
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    let counter = ((time.timestamp() - self.shift as i64) / self.period as i64) as u32 % self.max;
    let last_counter = ((self.last.timestamp() - self.shift as i64) / self.period as i64) as u32 % self.max;
    if counter != last_counter {
      self.last = *time;
      vec![(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(counter))]
    } else {
      vec![]
    }
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    Some(self.last + chrono::Duration::seconds(self.period.into()))
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
