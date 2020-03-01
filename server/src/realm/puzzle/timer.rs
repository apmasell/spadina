struct Timer {
  frequency: u32,
  counter: u32,
  next: chrono::DateTime<chrono::Utc>,
}

pub struct TimerAsset {
  pub frequency: u32,
  pub initial_counter: u32,
}

impl crate::realm::puzzle::PuzzleAsset for TimerAsset {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Timer { frequency: self.frequency, counter: self.initial_counter, next: *time + chrono::Duration::seconds(self.frequency.into()) })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (frequency, counter) = serde_json::from_value(input)?;
    Ok(Box::new(Timer { frequency, counter, next: *time + chrono::Duration::seconds(frequency.into()) }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Timer {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Frequency, spadina_core::asset::rules::PieceValue::Num(freq)) => {
        self.frequency = *freq;
      }
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Num(counter)) => {
        self.counter = *counter;
      }
      (spadina_core::puzzle::PuzzleCommand::Up, spadina_core::asset::rules::PieceValue::Empty) => {
        self.counter += 1;
      }
      (spadina_core::puzzle::PuzzleCommand::Up, spadina_core::asset::rules::PieceValue::Num(delta)) => {
        self.counter += *delta;
      }
      (spadina_core::puzzle::PuzzleCommand::Down, spadina_core::asset::rules::PieceValue::Empty) => {
        if self.counter > 0 {
          self.counter -= 1
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Down, spadina_core::asset::rules::PieceValue::Num(delta)) => {
        self.counter = if self.counter < *delta { 0 } else { self.counter - delta };
      }
      _ => (),
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
    serde_json::to_value(&(self.frequency, self.counter))
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    let tick = *time >= self.next;
    while self.next < *time {
      self.next = self.next + chrono::Duration::seconds(self.frequency.into());
    }
    let emit = if tick && self.counter > 0 {
      self.counter -= 1;
      self.counter == 0
    } else {
      false
    };

    if emit && tick {
      vec![
        (spadina_core::puzzle::PuzzleEvent::AtMin, spadina_core::asset::rules::PieceValue::Empty),
        (spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Num(self.counter)),
      ]
    } else if tick {
      vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Num(self.counter))]
    } else {
      vec![]
    }
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    if self.counter > 0 {
      (self.next - chrono::Utc::now()).to_std().ok()
    } else {
      None
    }
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
