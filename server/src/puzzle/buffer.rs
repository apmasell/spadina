use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;

const SERIALIZATION_LENGTH: u32 = 2;

struct Buffer<T: Sync> {
  contents: std::collections::vec_deque::VecDeque<T>,
  capacity: usize,
}
pub struct BufferAsset {
  pub length: u32,
  pub buffer_type: puzzleverse_core::asset::puzzle::ListType,
}

impl crate::puzzle::PuzzleAsset for BufferAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    let length = self.length as usize;
    match self.buffer_type {
      puzzleverse_core::asset::puzzle::ListType::Bool => {
        std::boxed::Box::new(Buffer::<bool> { contents: std::collections::vec_deque::VecDeque::with_capacity(length), capacity: length })
          as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Int => {
        std::boxed::Box::new(Buffer::<u32> { contents: std::collections::vec_deque::VecDeque::with_capacity(length), capacity: length })
          as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Realm => std::boxed::Box::new(Buffer::<puzzleverse_core::asset::rules::RealmLink> {
        contents: std::collections::vec_deque::VecDeque::with_capacity(length),
        capacity: length,
      }) as Box<dyn crate::puzzle::PuzzlePiece>,
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    match self.buffer_type {
      puzzleverse_core::asset::puzzle::ListType::Bool => load::<bool, crate::puzzle::InputBuffer>(input),
      puzzleverse_core::asset::puzzle::ListType::Int => load::<u32, crate::puzzle::InputBuffer>(input),
      puzzleverse_core::asset::puzzle::ListType::Realm => load::<puzzleverse_core::asset::rules::RealmLink, crate::puzzle::InputBuffer>(input),
    }
  }
}

fn load<'a, T, R>(input: &mut crate::puzzle::InputBuffer) -> crate::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Clone,
  T: Send + Sync,
  Vec<T>: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::EncodeValue,
  T: crate::puzzle::DecodeSaved,
  T: crate::puzzle::ExtractValue,
{
  crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
  let length = rmp::decode::read_u32(input)? as usize;
  let mut buffer = Buffer::<T> { contents: std::collections::vec_deque::VecDeque::new(), capacity: length };
  for _ in 0..rmp::decode::read_array_len(input)? {
    buffer.contents.push_back(T::read(input)?);
  }
  Ok(Box::new(buffer) as Box<dyn crate::puzzle::PuzzlePiece>)
}

impl<'x, T> crate::puzzle::PuzzlePiece for Buffer<T>
where
  T: Clone,
  T: Send + Sync,
  Vec<T>: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::EncodeValue,
  T: crate::puzzle::ExtractValue,
{
  fn accept<'a>(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    if name == &puzzleverse_core::PuzzleCommand::Insert {
      T::extract_value(value)
        .map(|v| {
          self.contents.push_back(v.clone());
          while self.contents.len() > self.capacity {
            self.contents.pop_front();
          }
          vec![
            crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, self.contents.iter().cloned().collect::<Vec<T>>().into()),
            crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Selected, v.into()),
          ]
        })
        .unwrap_or_else(|| vec![])
    } else if name == &puzzleverse_core::PuzzleCommand::Clear && *value == puzzleverse_core::asset::rules::PieceValue::Empty {
      self.contents.clear();
      vec![
        crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, self.contents.iter().cloned().collect::<Vec<T>>().into()),
        crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Cleared, puzzleverse_core::asset::rules::PieceValue::Empty),
      ]
    } else if name == &puzzleverse_core::PuzzleCommand::Toggle
      && *value == puzzleverse_core::asset::rules::PieceValue::Empty
      && !self.contents.is_empty()
    {
      let mut contents: Vec<T> = self.contents.drain(..).collect();
      contents.shuffle(&mut rand::thread_rng());
      self.contents.extend(contents.drain(..));
      vec![crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, self.contents.iter().cloned().collect::<Vec<T>>().into())]
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
    rmp::encode::write_u32(output, self.contents.capacity() as u32)?;
    rmp::encode::write_array_len(output, self.contents.len() as u32)?;

    for v in self.contents.iter() {
      v.write(output)?;
    }
    Ok(())
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }

  fn next(self: &Self) -> Option<DateTime<Utc>> {
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
