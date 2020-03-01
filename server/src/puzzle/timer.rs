const SERIALIZATION_LENGTH: u32 = 2;
struct Timer {
  frequency: u32,
  counter: u32,
  next: chrono::DateTime<chrono::Utc>,
}

pub struct TimerAsset {
  pub frequency: u32,
  pub initial_counter: u32,
}

impl crate::puzzle::PuzzleAsset for TimerAsset {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Box::new(Timer { frequency: self.frequency, counter: self.initial_counter, next: *time + chrono::Duration::seconds(self.frequency.into()) })
      as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    let frequency = rmp::decode::read_u32(input)?;
    let counter = rmp::decode::read_u32(input)?;
    Ok(Box::new(Timer { frequency, counter, next: *time + chrono::Duration::seconds(frequency.into()) }) as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}

impl crate::puzzle::PuzzlePiece for Timer {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    match (name, value) {
      (puzzleverse_core::PuzzleCommand::Frequency, puzzleverse_core::asset::rules::PieceValue::Num(freq)) => {
        self.frequency = *freq;
      }
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Num(counter)) => {
        self.counter = *counter;
      }
      (puzzleverse_core::PuzzleCommand::Up, puzzleverse_core::asset::rules::PieceValue::Empty) => {
        self.counter += 1;
      }
      (puzzleverse_core::PuzzleCommand::Up, puzzleverse_core::asset::rules::PieceValue::Num(delta)) => {
        self.counter += *delta;
      }
      (puzzleverse_core::PuzzleCommand::Down, puzzleverse_core::asset::rules::PieceValue::Empty) => {
        if self.counter > 0 {
          self.counter -= 1
        }
      }
      (puzzleverse_core::PuzzleCommand::Down, puzzleverse_core::asset::rules::PieceValue::Num(delta)) => {
        self.counter = if self.counter < *delta { 0 } else { self.counter - delta };
      }
      _ => (),
    }
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
    rmp::encode::write_u32(output, self.frequency)?;
    rmp::encode::write_u32(output, self.counter)
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
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
        (puzzleverse_core::PuzzleEvent::AtMin, puzzleverse_core::asset::rules::PieceValue::Empty),
        (puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.counter)),
      ]
    } else if tick {
      vec![(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Num(self.counter))]
    } else {
      vec![]
    }
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    if self.counter > 0 {
      Some(self.next)
    } else {
      None
    }
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
