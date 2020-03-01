use puzzleverse_core::asset::rules::RealmLink;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct Index<T> {
  items: Vec<T>,
  selected: u32,
}

pub(crate) struct IndexAsset(puzzleverse_core::asset::puzzle::ListType);

impl crate::puzzle::PuzzleAsset for IndexAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    match self.0 {
      puzzleverse_core::asset::puzzle::ListType::Bool => {
        Box::new(Index::<bool> { items: Vec::new(), selected: 0 }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Int => {
        Box::new(Index::<u32> { items: Vec::new(), selected: 0 }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Realm => {
        Box::new(Index::<RealmLink> { items: Vec::new(), selected: 0 }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
    }
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    match self.0 {
      puzzleverse_core::asset::puzzle::ListType::Bool => load::<bool>(input),
      puzzleverse_core::asset::puzzle::ListType::Int => load::<u32>(input),
      puzzleverse_core::asset::puzzle::ListType::Realm => load::<RealmLink>(input),
    }
  }
}
fn load<'a, T>(input: &mut crate::puzzle::InputBuffer) -> crate::puzzle::DeserializationResult<'a>
where
  T: 'a,
  T: Clone,
  T: Send + Sync,
  T: crate::puzzle::DecodeSaved,
  T: crate::puzzle::EncodeValue,
  T: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::ExtractList,
  T: crate::puzzle::ExtractValue,
{
  crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
  let array_len = rmp::decode::read_array_len(input)?;
  let mut array = Vec::with_capacity(array_len as usize);

  for _ in 0..array_len {
    array.push(T::read(input)?);
  }

  Ok(Box::new(Index { items: array, selected: rmp::decode::read_u32(input)? }) as Box<dyn crate::puzzle::PuzzlePiece>)
}

impl<T> crate::puzzle::PuzzlePiece for Index<T>
where
  T: Clone,
  T: Send + Sync,
  T: crate::puzzle::DecodeSaved,
  T: crate::puzzle::EncodeValue,
  T: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::ExtractList,
  T: crate::puzzle::ExtractValue,
{
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    let changed = match name {
      puzzleverse_core::PuzzleCommand::Insert => match T::extract_list(value) {
        Some(v) => {
          self.items = v.iter().cloned().collect();
          true
        }
        None => false,
      },
      puzzleverse_core::PuzzleCommand::Set => match value {
        puzzleverse_core::asset::rules::PieceValue::Num(s) => {
          self.selected = *s;
          true
        }
        _ => false,
      },
      _ => false,
    };

    if changed {
      vec![crate::puzzle::OutputEvent::Event(
        puzzleverse_core::PuzzleEvent::Changed,
        self.items.get(self.selected as usize).map(|v| v.clone().into()).unwrap_or(puzzleverse_core::asset::rules::PieceValue::Empty),
      )]
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
    rmp::encode::write_array_len(output, self.items.len() as u32)?;
    for i in &self.items {
      i.write(output)?;
    }
    rmp::encode::write_u32(output, self.selected)
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
