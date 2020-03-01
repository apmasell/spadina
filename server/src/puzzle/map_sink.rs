const SERIALIZATION_LENGTH: u32 = 1;
struct MapSink {
  map: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub struct MapSinkAsset(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

impl crate::puzzle::PuzzleAsset for MapSinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(MapSink { map: self.0 }) as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    self.0.store(rmp::decode::read_bool(input)?, std::sync::atomic::Ordering::Relaxed);
    Ok(Box::new(MapSink { map: self.0 }) as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}
impl crate::puzzle::PuzzlePiece for MapSink {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    if name == &puzzleverse_core::PuzzleCommand::Set {
      match value {
        puzzleverse_core::asset::rules::PieceValue::Bool(v) => self.map.store(*v, std::sync::atomic::Ordering::Relaxed),
        _ => (),
      }
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
    rmp::encode::write_bool(output, self.map.load(std::sync::atomic::Ordering::Relaxed)).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)?;
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
