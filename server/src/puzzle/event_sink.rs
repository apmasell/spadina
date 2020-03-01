const SERIALIZATION_LENGTH: u32 = 0;
struct EventSink {
  values: std::collections::BTreeSet<chrono::DateTime<chrono::Utc>>,
  name: puzzleverse_core::PropertyKey,
}

pub struct EventSinkAsset(pub String);

impl crate::puzzle::PuzzleAsset for EventSinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(EventSink { name: puzzleverse_core::PropertyKey::EventSink(self.0), values: std::collections::BTreeSet::new() })
      as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(EventSink { name: puzzleverse_core::PropertyKey::EventSink(self.0), values: std::collections::BTreeSet::new() })
      as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}
impl crate::puzzle::PuzzlePiece for EventSink {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    _: &puzzleverse_core::asset::rules::PieceValue,
    time: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    if name == &puzzleverse_core::PuzzleCommand::Set {
      self.values.insert(time.clone());
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
    Ok(())
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    self.values.retain(|t| *time - *t < chrono::Duration::minutes(5));
    vec![]
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    None
  }
  fn reset(&self) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    Some(super::PuzzleConsequence(
      &self.name,
      crate::realm::Multi::Single(puzzleverse_core::PropertyValue::Ticks(self.values.iter().cloned().collect())),
    ))
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
