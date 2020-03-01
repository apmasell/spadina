use crate::puzzle::{DecodeSaved, EncodeValue, PuzzlePiece};
const SERIALIZATION_LENGTH: u32 = 2;

pub(crate) struct RealmSelector {
  matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  realm: puzzleverse_core::asset::rules::RealmLink,
}

pub(crate) struct RealmSelectorAsset(pub puzzleverse_core::asset::rules::PlayerMarkMatcher);

impl crate::puzzle::PuzzleAsset for RealmSelectorAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    std::boxed::Box::new(RealmSelector { matcher: self.0, realm: puzzleverse_core::asset::rules::RealmLink::Home })
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(RealmSelector { matcher: self.0, realm: puzzleverse_core::asset::rules::RealmLink::read(input)? }) as Box<dyn PuzzlePiece>)
  }
}

impl PuzzlePiece for RealmSelector {
  fn accept(
    self: &mut Self,
    _: &puzzleverse_core::PuzzleCommand,
    _: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    vec![]
  }
  fn interact(
    self: &mut Self,
    interaction: &puzzleverse_core::InteractionType,
    player_server: &str,
    state: Option<u8>,
  ) -> (puzzleverse_core::InteractionResult, crate::puzzle::SimpleOutputEvents) {
    if let puzzleverse_core::InteractionType::Realm(realm) = interaction {
      if self.matcher.matches(state) {
        (
          puzzleverse_core::InteractionResult::Accepted,
          vec![(
            puzzleverse_core::PuzzleEvent::Changed,
            puzzleverse_core::asset::rules::PieceValue::Realm(match realm {
              puzzleverse_core::RealmTarget::Home => puzzleverse_core::asset::rules::RealmLink::Home,
              puzzleverse_core::RealmTarget::RemoteRealm { realm, server } => {
                puzzleverse_core::asset::rules::RealmLink::Global(realm.clone(), server.clone())
              }
              puzzleverse_core::RealmTarget::PersonalRealm(asset) => puzzleverse_core::asset::rules::RealmLink::Owner(asset.clone()),
              puzzleverse_core::RealmTarget::LocalRealm(realm) => {
                puzzleverse_core::asset::rules::RealmLink::Global(realm.clone(), player_server.to_string())
              }
            }),
          )],
        )
      } else {
        (puzzleverse_core::InteractionResult::Failed, vec![])
      }
    } else {
      (puzzleverse_core::InteractionResult::Invalid, vec![])
    }
  }

  fn serialize(self: &Self, output: &mut crate::puzzle::OutputBuffer) -> crate::puzzle::SerializationResult {
    rmp::encode::write_array_len(output, SERIALIZATION_LENGTH)?;
    self.realm.write(output)
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
