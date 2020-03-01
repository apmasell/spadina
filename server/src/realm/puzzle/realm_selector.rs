use crate::realm::puzzle::PuzzlePiece;

pub(crate) struct RealmSelector {
  matcher: spadina_core::asset::rules::PlayerMarkMatcher,
  realm: spadina_core::asset::rules::LinkOut<crate::shstr::ShStr>,
}

pub(crate) struct RealmSelectorAsset(pub spadina_core::asset::rules::PlayerMarkMatcher);

impl crate::realm::puzzle::PuzzleAsset for RealmSelectorAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    std::boxed::Box::new(RealmSelector { matcher: self.0, realm: spadina_core::asset::rules::LinkOut::Realm(spadina_core::realm::RealmTarget::Home) })
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    Ok(Box::new(RealmSelector { matcher: self.0, realm: serde_json::from_value(input)? }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for RealmSelector {
  fn accept(
    self: &mut Self,
    _: &spadina_core::puzzle::PuzzleCommand,
    _: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    vec![]
  }
  fn interact(
    self: &mut Self,
    interaction: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    state: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    if let spadina_core::realm::InteractionType::Realm(realm) = interaction {
      if self.matcher.matches(state) {
        (
          spadina_core::realm::InteractionResult::Accepted,
          vec![(
            spadina_core::puzzle::PuzzleEvent::Changed,
            spadina_core::asset::rules::PieceValue::Realm(spadina_core::asset::rules::LinkOut::Realm(realm.clone().into())),
          )],
        )
      } else {
        (spadina_core::realm::InteractionResult::Failed, vec![])
      }
    } else {
      (spadina_core::realm::InteractionResult::Invalid, vec![])
    }
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&self.realm)
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
