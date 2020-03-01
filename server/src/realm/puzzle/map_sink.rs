struct MapSink {
  map: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub struct MapSinkAsset(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

impl crate::realm::puzzle::PuzzleAsset for MapSinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(MapSink { map: self.0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    self.0.store(serde_json::from_value(input)?, std::sync::atomic::Ordering::Relaxed);
    Ok(Box::new(MapSink { map: self.0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}
impl crate::realm::puzzle::PuzzlePiece for MapSink {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    if name == &spadina_core::puzzle::PuzzleCommand::Set {
      match value {
        spadina_core::asset::rules::PieceValue::Bool(v) => self.map.store(*v, std::sync::atomic::Ordering::Relaxed),
        _ => (),
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
    serde_json::to_value(self.map.load(std::sync::atomic::Ordering::Relaxed))
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
