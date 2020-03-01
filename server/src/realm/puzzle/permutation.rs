use rand::seq::SliceRandom;
pub(crate) struct Permutation {
  pbox: Vec<u32>,
  selected: u32,
}

pub(crate) struct PermutationAsset {
  pub length: u8,
}

impl crate::realm::puzzle::PuzzleAsset for PermutationAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    let mut pbox: Vec<u32> = (0..self.length as u32).collect();
    pbox.shuffle(&mut rand::thread_rng());
    Box::new(Permutation { pbox, selected: 0 }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    let (pbox, selected) = serde_json::from_value(input)?;
    Ok(Box::new(Permutation { pbox, selected }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Permutation {
  fn accept<'a>(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    let (shuffled, new_selected) = match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Num(v)) => {
        (false, std::cmp::max(0, std::cmp::min(*v, self.pbox.len() as u32)))
      }
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Empty) => {
        self.pbox.shuffle(&mut rand::thread_rng());
        (true, self.selected)
      }
      _ => (false, self.selected),
    };

    if shuffled {
      self.selected = new_selected;
      vec![
        crate::realm::puzzle::OutputEvent::Event(
          spadina_core::puzzle::PuzzleEvent::Changed,
          spadina_core::asset::rules::PieceValue::NumList(self.pbox.clone()),
        ),
        crate::realm::puzzle::OutputEvent::Event(
          spadina_core::puzzle::PuzzleEvent::Selected,
          spadina_core::asset::rules::PieceValue::Num(self.pbox[self.selected as usize]),
        ),
      ]
    } else if new_selected == self.selected {
      vec![]
    } else {
      self.selected = new_selected;
      vec![crate::realm::puzzle::OutputEvent::Event(
        spadina_core::puzzle::PuzzleEvent::Selected,
        spadina_core::asset::rules::PieceValue::Num(self.pbox[self.selected as usize]),
      )]
    }
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }
  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(&(&self.pbox, self.selected))
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
