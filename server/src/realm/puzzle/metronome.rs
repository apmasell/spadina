struct Metronome {
  frequency: u32,
  next: chrono::DateTime<chrono::Utc>,
}

pub(crate) struct MetronomeAsset {
  pub frequency: u32,
}

impl crate::realm::puzzle::PuzzleAsset for MetronomeAsset {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Metronome { frequency: self.frequency, next: *time + chrono::Duration::seconds(self.frequency.into()) })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let frequency = serde_json::from_value(input)?;
    Ok(Box::new(Metronome { frequency, next: *time + chrono::Duration::seconds(frequency.into()) }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Metronome {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    if name == &spadina_core::puzzle::PuzzleCommand::Frequency {
      if let spadina_core::asset::rules::PieceValue::Num(freq) = value {
        self.frequency = *freq;
      }
    }
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
    serde_json::to_value(self.frequency)
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    let emit = *time >= self.next;
    while self.next < *time {
      self.next = self.next + chrono::Duration::seconds(self.frequency.into());
    }
    if emit {
      vec![(spadina_core::puzzle::PuzzleEvent::Cleared, spadina_core::asset::rules::PieceValue::Empty)]
    } else {
      vec![]
    }
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    (self.next - chrono::Utc::now()).to_std().ok()
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
