use crate::puzzle::OutputEvent;
use rand::Rng;
const SERIALIZATION_LENGTH: u32 = 0;

struct Proximity {
  matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher,
  players: std::collections::HashSet<crate::PlayerKey>,
}

pub struct ProximityAsset(pub puzzleverse_core::asset::rules::PlayerMarkMatcher);

impl crate::puzzle::PuzzleAsset for ProximityAsset {
  fn create(
    self: Box<Self>,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Box::new(Proximity { matcher: self.0, players: std::collections::HashSet::new() }) as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Proximity { matcher: self.0, players: std::collections::HashSet::new() }) as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}

impl crate::puzzle::PuzzlePiece for Proximity {
  fn accept(
    self: &mut Self,
    name: &puzzleverse_core::PuzzleCommand,
    value: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    match (name, value) {
      (puzzleverse_core::PuzzleCommand::Send, puzzleverse_core::asset::rules::PieceValue::Realm(realm)) => {
        let players = self.players.drain().collect();
        vec![
          crate::puzzle::OutputEvent::Send(realm.clone(), players),
          crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, (self.players.len() as u32).into()),
        ]
      }
      (puzzleverse_core::PuzzleCommand::Send, puzzleverse_core::asset::rules::PieceValue::RealmList(realms)) => {
        if realms.is_empty() {
          vec![]
        } else {
          let mut sends: Vec<OutputEvent> = self
            .players
            .drain()
            .flat_map(|p| Some(crate::puzzle::OutputEvent::Send(realms.get(rand::thread_rng().gen_range(0..realms.len()))?.clone(), vec![p.clone()])))
            .collect();
          sends.push(crate::puzzle::OutputEvent::Event(puzzleverse_core::PuzzleEvent::Changed, (self.players.len() as u32).into()));
          sends
        }
      }
      (puzzleverse_core::PuzzleCommand::Clear, puzzleverse_core::asset::rules::PieceValue::Empty)
      | (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Empty) => {
        vec![crate::puzzle::OutputEvent::Unmark(self.players.iter().copied().collect())]
      }
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::Num(state)) => {
        vec![crate::puzzle::OutputEvent::Mark(u8::try_from(*state).unwrap_or(u8::MAX), self.players.iter().copied().collect())]
      }
      (puzzleverse_core::PuzzleCommand::Set, puzzleverse_core::asset::rules::PieceValue::NumList(states)) => self
        .players
        .iter()
        .copied()
        .zip(states.iter().flat_map(|&state| u8::try_from(state).ok()).cycle())
        .map(|(player, state)| crate::puzzle::OutputEvent::Mark(state, vec![player]))
        .collect(),
      (puzzleverse_core::PuzzleCommand::Clear, puzzleverse_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::puzzle::OutputEvent::BitClear(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      (puzzleverse_core::PuzzleCommand::Insert, puzzleverse_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::puzzle::OutputEvent::BitSet(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      (puzzleverse_core::PuzzleCommand::Toggle, puzzleverse_core::asset::rules::PieceValue::Num(state)) => match u8::try_from(*state) {
        Ok(state) => vec![crate::puzzle::OutputEvent::BitToggle(state, self.players.iter().copied().collect())],
        Err(_) => vec![],
      },
      _ => vec![],
    }
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
    Ok(())
  }
  fn tick(self: &mut Self, _: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn reset(&self) -> crate::puzzle::SimpleOutputEvents {
    vec![(puzzleverse_core::PuzzleEvent::Changed, (self.players.len() as u32).into())]
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    None
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
  }
  fn walk(
    self: &mut Self,
    player: &crate::PlayerKey,
    state: Option<u8>,
    event: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::puzzle::SimpleOutputEvents {
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
      vec![(puzzleverse_core::PuzzleEvent::Changed, (self.players.len() as u32).into())]
    }
  }
}
