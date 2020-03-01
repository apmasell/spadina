use crate::realm::puzzle::PuzzlePiece;

pub(crate) struct Button {
  enabled: bool,
  matcher: spadina_core::asset::rules::PlayerMarkMatcher,
}

pub(crate) struct ButtonAsset {
  pub enabled: bool,
  pub matcher: spadina_core::asset::rules::PlayerMarkMatcher,
}

impl crate::realm::puzzle::PuzzleAsset for ButtonAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Button { enabled: self.enabled, matcher: self.matcher })
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    Ok(Box::new(Button { enabled: serde_json::from_value(input)?, matcher: self.matcher }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Button {
  fn accept<'a>(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let new_enabled = match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Empty) => true,
      (spadina_core::puzzle::PuzzleCommand::Disable, spadina_core::asset::rules::PieceValue::Empty) => false,
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Bool(v)) => *v,
      _ => self.enabled,
    };
    if new_enabled == self.enabled {
      vec![]
    } else {
      self.enabled = new_enabled;
      vec![crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Sensitive,
        spadina_core::asset::rules::PieceValue::Bool(self.enabled),
      )]
    }
  }
  fn interact(
    self: &mut Self,
    interaction: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    state: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    match interaction {
      spadina_core::realm::InteractionType::Click => {
        if self.enabled && self.matcher.matches(state) {
          (
            spadina_core::realm::InteractionResult::Accepted,
            vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Empty)],
          )
        } else {
          (spadina_core::realm::InteractionResult::Failed, vec![])
        }
      }
      _ => (spadina_core::realm::InteractionResult::Invalid, vec![]),
    }
  }

  fn serialize(&self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(self.enabled)
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
