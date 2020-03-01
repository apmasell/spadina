use crate::realm::puzzle::PuzzlePiece;
pub(crate) struct RadioButton {
  matcher: spadina_core::asset::rules::PlayerMarkMatcher,
  state: super::RadioSharedState,
  value: u32,
}

pub(crate) struct RadioButtonAsset {
  pub enabled: bool,
  pub initial: u32,
  pub matcher: spadina_core::asset::rules::PlayerMarkMatcher,
  pub name: crate::shstr::ShStr,
  pub value: u32,
}

impl crate::realm::puzzle::PuzzleAsset for RadioButtonAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(RadioButton {
      matcher: self.matcher,
      state: radio_states.entry(self.name.clone()).or_insert(std::sync::Arc::new((self.name, self.initial.into(), self.enabled.into()))).clone(),
      value: self.value,
    })
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    radio_states: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (current, enabled) = serde_json::from_value::<(u32, bool)>(input)?;
    Ok(Box::new(RadioButton {
      matcher: self.matcher,
      state: radio_states.entry(self.name.clone()).or_insert(std::sync::Arc::new((self.name, current.into(), enabled.into()))).clone(),
      value: self.value,
    }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for RadioButton {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let mut results = Vec::new();
    if let Some(new_enabled) = match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Empty) => Some(true),
      (spadina_core::puzzle::PuzzleCommand::Disable, spadina_core::asset::rules::PieceValue::Empty) => Some(false),
      (spadina_core::puzzle::PuzzleCommand::Enable, spadina_core::asset::rules::PieceValue::Bool(v)) => Some(*v),
      _ => None,
    } {
      if self.state.2.swap(new_enabled, std::sync::atomic::Ordering::Relaxed) != new_enabled {
        results.push(crate::realm::puzzle::OutputEvent::Event(
          spadina_core::puzzle::PuzzleEvent::Sensitive,
          spadina_core::asset::rules::PieceValue::Bool(new_enabled),
        ));
      }
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
        if self.matcher.matches(state)
          && self.state.2.load(std::sync::atomic::Ordering::Relaxed)
          && self.state.1.swap(self.value, std::sync::atomic::Ordering::Relaxed) != self.value
        {
          (
            spadina_core::realm::InteractionResult::Accepted,
            vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Num(self.value))],
          )
        } else {
          (spadina_core::realm::InteractionResult::Failed, vec![])
        }
      }
      _ => (spadina_core::realm::InteractionResult::Invalid, vec![]),
    }
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&(self.state.1.load(std::sync::atomic::Ordering::Relaxed), self.state.2.load(std::sync::atomic::Ordering::Relaxed)))
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
