use crate::realm::puzzle::OutputEvent;
use rand::Rng;

struct Proximity {
  matcher: spadina_core::asset::rules::PlayerMarkMatcher,
  players: std::collections::HashSet<crate::realm::puzzle::PlayerKey>,
}

pub struct ProximityAsset(pub spadina_core::asset::rules::PlayerMarkMatcher);

impl crate::realm::puzzle::PuzzleAsset for ProximityAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Proximity { matcher: self.0, players: std::collections::HashSet::new() }) as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    _: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    Ok(Box::new(Proximity { matcher: self.0, players: std::collections::HashSet::new() }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl crate::realm::puzzle::PuzzlePiece for Proximity {
  fn accept(
    self: &mut Self,
    name: &spadina_core::puzzle::PuzzleCommand,
    value: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    match (name, value) {
      (spadina_core::puzzle::PuzzleCommand::Send, spadina_core::asset::rules::PieceValue::Realm(realm)) => {
        let players = self.players.drain().collect();
        vec![
          crate::realm::puzzle::OutputEvent::Send(realm.clone(), players),
          crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::Changed, (self.players.len() as u32).into()),
        ]
      }
      (spadina_core::puzzle::PuzzleCommand::Send, spadina_core::asset::rules::PieceValue::RealmList(realms)) => {
        if realms.is_empty() {
          vec![]
        } else {
          let mut sends: Vec<OutputEvent> = self
            .players
            .drain()
            .flat_map(|p| {
              Some(crate::realm::puzzle::OutputEvent::Send(realms.get(rand::thread_rng().gen_range(0..realms.len()))?.clone(), vec![p.clone()]))
            })
            .collect();
          sends.push(crate::realm::puzzle::OutputEvent::Event(spadina_core::puzzle::PuzzleEvent::Changed, (self.players.len() as u32).into()));
          sends
        }
      }
      (spadina_core::puzzle::PuzzleCommand::Clear, spadina_core::asset::rules::PieceValue::Empty)
      | (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Empty) => {
        vec![crate::realm::puzzle::OutputEvent::Unmark(self.players.iter().copied().collect())]
      }
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::Num(state)) => {
        vec![crate::realm::puzzle::OutputEvent::Mark(u8::try_from(*state).unwrap_or(u8::MAX), self.players.iter().copied().collect())]
      }
      (spadina_core::puzzle::PuzzleCommand::Set, spadina_core::asset::rules::PieceValue::NumList(states)) => self
        .players
        .iter()
        .copied()
        .zip(states.iter().flat_map(|&state| u8::try_from(state).ok()).cycle())
        .map(|(player, state)| crate::realm::puzzle::OutputEvent::Mark(state, vec![player]))
        .collect(),
      (spadina_core::puzzle::PuzzleCommand::Clear, spadina_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::realm::puzzle::OutputEvent::BitClear(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      (spadina_core::puzzle::PuzzleCommand::Insert, spadina_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::realm::puzzle::OutputEvent::BitSet(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      (spadina_core::puzzle::PuzzleCommand::Toggle, spadina_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::realm::puzzle::OutputEvent::BitToggle(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      _ => vec![],
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
    Ok(serde_json::Value::Null)
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn reset(&self) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![(spadina_core::puzzle::PuzzleEvent::Changed, (self.players.len() as u32).into())]
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    None
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
  }
  fn walk(
    self: &mut Self,
    player: &crate::realm::puzzle::PlayerKey,
    state: Option<u8>,
    event: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::realm::puzzle::SimpleOutputEvents {
    let old_size = self.players.len();
    match event {
      crate::realm::navigation::PlayerNavigationEvent::Enter => {
        if self.matcher.matches(state) {
          self.players.insert(player.clone());
        }
      }
      crate::realm::navigation::PlayerNavigationEvent::Leave => {
        self.players.remove(&player);
      }
    }
    if old_size == self.players.len() {
      vec![]
    } else {
      vec![(spadina_core::puzzle::PuzzleEvent::Changed, (self.players.len() as u32).into())]
    }
  }
}
