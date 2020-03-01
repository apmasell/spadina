use rand::seq::SliceRandom;

struct Buffer<T: Sync> {
  contents: std::collections::vec_deque::VecDeque<T>,
  capacity: usize,
}
pub struct BufferAsset {
  pub length: u32,
  pub buffer_type: spadina_core::asset::puzzle::ListType,
}

impl crate::realm::puzzle::PuzzleAsset for BufferAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    let length = self.length as usize;
    match self.buffer_type {
      spadina_core::asset::puzzle::ListType::Bool => {
        std::boxed::Box::new(Buffer::<bool> { contents: std::collections::vec_deque::VecDeque::with_capacity(length), capacity: length })
          as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
      spadina_core::asset::puzzle::ListType::Int => {
        std::boxed::Box::new(Buffer::<u32> { contents: std::collections::vec_deque::VecDeque::with_capacity(length), capacity: length })
          as Box<dyn crate::realm::puzzle::PuzzlePiece>
      }
      spadina_core::asset::puzzle::ListType::Realm => std::boxed::Box::new(Buffer::<spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>> {
        contents: std::collections::vec_deque::VecDeque::with_capacity(length),
        capacity: length,
      }) as Box<dyn crate::realm::puzzle::PuzzlePiece>,
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    match self.buffer_type {
      spadina_core::asset::puzzle::ListType::Bool => load::<bool>(input, self.length),
      spadina_core::asset::puzzle::ListType::Int => load::<u32>(input, self.length),
      spadina_core::asset::puzzle::ListType::Realm => load::<spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>>(input, self.length),
    }
  }
}

fn load<'a, T>(input: serde_json::Value, capacity: u32) -> crate::realm::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Clone,
  T: Send + Sync,
  T: serde::de::DeserializeOwned,
  T: serde::Serialize,
  Vec<T>: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: crate::realm::puzzle::ExtractValue,
{
  let mut buffer = Buffer::<T> { contents: serde_json::from_value(input)?, capacity: capacity as usize };
  buffer.contents.truncate(capacity as usize);
  Ok(Box::new(buffer) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
}

impl<'x, T> crate::realm::puzzle::PuzzlePiece for Buffer<T>
where
  T: Clone,
  T: Send + Sync,
  T: serde::Serialize,
  Vec<T>: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: Into<spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>>,
  T: crate::realm::puzzle::ExtractValue,
{
  fn accept<'a>(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    if name == &spadina_core::puzzle::PuzzleCommand::Insert {
      T::extract_value(value)
        .map(|v| {
          self.contents.push_back(v.clone());
          while self.contents.len() > self.capacity {
            self.contents.pop_front();
          }
          vec![
            crate::realm::puzzle::OutputEvent::Event(
              spadina_core::puzzle::PuzzleEvent::Changed,
              self.contents.iter().cloned().collect::<Vec<T>>().into(),
            ),
            crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::Selected, v.into()),
          ]
        })
        .unwrap_or_else(|| vec![])
    } else if name == &spadina_core::puzzle::PuzzleCommand::Clear && *value == spadina_core::asset::rules::PieceValue::Empty {
      self.contents.clear();
      vec![
        crate::realm::puzzle::OutputEvent::Event(
          spadina_core::puzzle::PuzzleEvent::Changed,
          self.contents.iter().cloned().collect::<Vec<T>>().into(),
        ),
        crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::Cleared, spadina_core::asset::rules::PieceValue::Empty),
      ]
    } else if name == &spadina_core::puzzle::PuzzleCommand::Toggle
      && *value == spadina_core::asset::rules::PieceValue::Empty
      && !self.contents.is_empty()
    {
      let mut contents: Vec<T> = self.contents.drain(..).collect();
      contents.shuffle(&mut rand::thread_rng());
      self.contents.extend(contents.drain(..));
      vec![crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Changed,
        self.contents.iter().cloned().collect::<Vec<T>>().into(),
      )]
    } else {
      vec![]
    }
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }
  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&self.contents)
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }

  fn next(self: &Self) -> Option<std::time::Duration> {
    None
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
