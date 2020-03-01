struct EventSink {
  values: std::collections::BTreeSet<chrono::DateTime<chrono::Utc>>,
  name: spadina_core::realm::PropertyKey<crate::shstr::ShStr>,
}

pub struct EventSinkAsset(pub crate::shstr::ShStr);

impl crate::realm::puzzle::PuzzleAsset for EventSinkAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(EventSink { name: spadina_core::realm::PropertyKey::EventSink(self.0), values: std::collections::BTreeSet::new() })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    _: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    Ok(Box::new(EventSink { name: spadina_core::realm::PropertyKey::EventSink(self.0), values: std::collections::BTreeSet::new() })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}
impl crate::realm::puzzle::PuzzlePiece for EventSink {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    _: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    time: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    if name == &spadina_core::puzzle::PuzzleCommand::Set {
      self.values.insert(time.clone());
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
    Ok(serde_json::Value::Null)
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    self.values.retain(|t| *time - *t < chrono::Duration::minutes(5));
    vec![]
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    None
  }
  fn reset(&self) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    Some(super::PuzzleConsequence(
      &self.name,
      crate::realm::output::Multi::Single(spadina_core::realm::PropertyValue::Ticks(self.values.iter().cloned().collect())),
    ))
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
