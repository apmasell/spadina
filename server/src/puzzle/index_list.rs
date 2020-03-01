use puzzleverse_core::asset::rules::RealmLink;
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct IndexList<T> {
  items: Vec<T>,
  selected: Vec<u32>,
}

pub(crate) struct IndexListAsset(pub puzzleverse_core::asset::puzzle::ListType);

impl crate::puzzle::PuzzleAsset for IndexListAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    match self.0 {
      puzzleverse_core::asset::puzzle::ListType::Bool => {
        Box::new(IndexList::<bool> { items: Vec::new(), selected: vec![] }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Int => {
        Box::new(IndexList::<u32> { items: Vec::new(), selected: vec![] }) as Box<dyn crate::puzzle::PuzzlePiece>
      }
      puzzleverse_core::asset::puzzle::ListType::Realm => {
        Box::new(IndexList::<RealmLink> { items: Vec::new(), selected: vec![] }) as Box<dyn crate::puzzle::PuzzlePiece>
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
  T: Send + Sync,
  T: Clone,
  T: crate::puzzle::DecodeSaved,
  T: crate::puzzle::EncodeValue,
  Vec<T>: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::ExtractValue,
  T: crate::puzzle::ExtractList,
{
  crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
  let array_len = rmp::decode::read_array_len(input)?;
  let mut array = Vec::with_capacity(array_len as usize);

  for _ in 0..array_len {
    array.push(T::read(input)?);
  }

  let selected_len = rmp::decode::read_array_len(input)?;
  let mut selected = Vec::with_capacity(selected_len as usize);
  for _ in 0..array_len {
    selected.push(rmp::decode::read_u32(input)?);
  }

  Ok(Box::new(IndexList { items: array, selected }) as Box<dyn crate::puzzle::PuzzlePiece>)
}

impl<T> crate::puzzle::PuzzlePiece for IndexList<T>
where
  T: Clone,
  T: Send + Sync,
  T: crate::puzzle::DecodeSaved,
  T: crate::puzzle::EncodeValue,
  Vec<T>: Into<puzzleverse_core::asset::rules::PieceValue>,
  T: crate::puzzle::ExtractList,
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
          self.items = v.to_vec();
          true
        }
        None => false,
      },
      puzzleverse_core::PuzzleCommand::Set => match value {
        puzzleverse_core::asset::rules::PieceValue::NumList(s) => {
          self.selected = s.to_vec();
          true
        }
        _ => false,
      },
      _ => false,
    };

    if changed {
      vec![crate::puzzle::OutputEvent::Event(
        puzzleverse_core::PuzzleEvent::Changed,
        if self.items.is_empty() {
          puzzleverse_core::asset::rules::PieceValue::Empty
        } else {
          self.selected.iter().flat_map(|&i| self.items.get(i as usize % self.items.len())).cloned().collect::<Vec<T>>().into()
        },
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
    rmp::encode::write_array_len(output, self.selected.len() as u32)?;
    for &s in &self.selected {
      rmp::encode::write_u32(output, s)?;
    }
    Ok(())
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
