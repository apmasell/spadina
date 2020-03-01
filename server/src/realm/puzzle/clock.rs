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

impl crate::realm::puzzle::PuzzleAsset for ClockAsset {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Clock { period: self.period, shift: self.shift.unwrap_or((time.timestamp() % self.period as i64) as u32), max: self.max, last: *time })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (period, shift) = serde_json::from_value(input)?;
    Ok(Box::new(Clock { period, shift, max: self.max, last: *time }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Clock {
  fn accept<'a>(
    self: &mut Self,
    _: &spadina_core::puzzle::PuzzleCommand,
    _: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    vec![]
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&(self.period, self.shift))
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    let counter = ((time.timestamp() - self.shift as i64) / self.period as i64) as u32 % self.max;
    let last_counter = ((self.last.timestamp() - self.shift as i64) / self.period as i64) as u32 % self.max;
    if counter != last_counter {
      self.last = *time;
      vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Num(counter))]
    } else {
      vec![]
    }
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    (self.last - chrono::Utc::now() + chrono::Duration::seconds(self.period.into())).to_std().ok()
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
