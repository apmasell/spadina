use crate::realm::puzzle::PuzzlePiece;

pub(crate) struct Switch {
  state: bool,
  enabled: bool,
  matcher: spadina_core::asset::rules::PlayerMarkMatcher,
}

pub(crate) struct SwitchAsset {
  pub initial: bool,
  pub enabled: bool,
  pub matcher: spadina_core::asset::rules::PlayerMarkMatcher,
}

impl crate::realm::puzzle::PuzzleAsset for SwitchAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(Switch { matcher: self.matcher, state: self.initial, enabled: self.enabled })
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (state, enabled) = serde_json::from_value(input)?;
    Ok(Box::new(Switch { matcher: self.matcher, state, enabled }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for Switch {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let (new_enabled, new_state) = match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Down, spadina_core::asset::rules::PieceValue::Empty) => (self.enabled, true),
      (spadina_core::puzzle::PuzzleCommand::Up, spadina_core::asset::rules::PieceValue::Empty) => (self.enabled, false),
      (spadina_core::puzzle::PuzzleCommand::Toggle, spadina_core::asset::rules::PieceValue::Empty) => (self.enabled, !self.state),
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Empty) => (true, self.state),
      (spadina_core::puzzle::PuzzleCommand::Disable, spadina_core::asset::rules::PieceValue::Empty) => (false, self.state),
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Bool(v)) => (self.enabled, *v),
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Bool(v)) => (*v, self.state),
      _ => (self.enabled, self.state),
    };
    let mut results = Vec::new();

    if new_enabled != self.enabled {
      self.enabled = new_enabled;
      results.push(crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Sensitive,
        spadina_core::asset::rules::PieceValue::Bool(self.enabled),
      ));
    }
    if new_state != self.state {
      self.state = new_state;
      results.push(crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Changed,
        spadina_core::asset::rules::PieceValue::Bool(self.state),
      ));
    }
    results
  }
  fn interact(
    self: &mut Self,
    interaction: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    state: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    match interaction {
      spadina_core::realm::InteractionType::Click => {
        if self.enabled && self.matcher.matches(state) {
          self.state = !self.state;
          (
            spadina_core::realm::InteractionResult::Accepted,
            vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Bool(self.state))],
          )
        } else {
          (spadina_core::realm::InteractionResult::Failed, vec![])
        }
      }
      _ => (spadina_core::realm::InteractionResult::Invalid, vec![]),
    }
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&(self.state, self.enabled))
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
