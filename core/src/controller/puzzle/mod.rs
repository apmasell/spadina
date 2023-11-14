pub mod action;
pub mod area;
pub mod platforms;
pub mod state_machine;

use crate::controller::puzzle::ProcessingResult::Updated;
use crate::controller::{ControllerInput, ControllerOutput, MessagePackSerializer};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserializer, Serializer};
use std::collections::BTreeSet;
use std::error::Error;
use std::ops::BitOr;
use std::time::Duration;

pub trait PuzzleTemplate {
  type Error: Error + 'static;
  type Puzzle: Puzzle + 'static;
  fn blank(&self, now: DateTime<Tz>, seed: u32) -> Result<Self::Puzzle, Self::Error>;
  fn load<'de, D: Deserializer<'de>>(&self, de: D, now: DateTime<Utc>) -> Result<Self::Puzzle, crate::controller::LoadError<D::Error, Self::Error>>;
}
pub trait Puzzle: serde::Serialize + Send + 'static {
  type Area: 'static;
  type Error: Error + 'static;
  type InputIdentifier: 'static;
  type OutputIdentifier: 'static;
  fn next_timer(&self) -> Option<Duration>;
  fn outputs(&self) -> Box<dyn Iterator<Item = (&Self::OutputIdentifier, &MultiStateValue)> + '_>;
  fn process<AreaCounter: area::AreaCounter>(
    &mut self,
    input: PuzzleInput<Self::InputIdentifier, Self::Area, AreaCounter>,
    now: DateTime<Utc>,
  ) -> Result<ProcessingResult, Self::Error>;
  fn process_timer(&mut self, now: DateTime<Utc>) -> Result<ProcessingResult, Self::Error>;
}

pub trait PlayerPositions: Send + 'static {
  type Area: 'static;
  type Error: Error + 'static;
  type OutputIdentifier: 'static;
  fn create(now: DateTime<Utc>, seed: u32) -> Self;
  fn add(&mut self, player_id: u32) -> Result<(), Self::Error>;
  fn modify(&mut self, area: &Self::Area, condition: &area::PlayerStateCondition, modification: &area::PlayerStateModification);
  fn next_timer(&self) -> Option<Duration>;
  fn remove(&mut self, area: &Self::Area, condition: &area::PlayerStateCondition) -> Vec<u32>;
  fn set_states(&mut self, inputs: Box<dyn Iterator<Item = (&Self::OutputIdentifier, &MultiStateValue)>>) -> Result<(), Self::Error>;
  fn transfer(&mut self, source: &Self::Area, condition: &area::PlayerStateCondition, target: &Self::Area) -> Result<(), Self::Error>;
  fn update(&mut self) -> Result<(), Self::Error>;
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MultiStateValue {
  Bool(bool),
  Num(u32),
  MultiBool { default: bool, values: Vec<(area::PlayerStateCondition, bool)> },
  MultiNum { default: u32, values: Vec<(area::PlayerStateCondition, u32)> },
}

#[derive(Clone, Debug)]
pub enum PuzzleInput<InputIdentifier, Area, AreaCounter> {
  Click(InputIdentifier, u32),
  Area(Area, AreaCounter),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ProcessingResult {
  Updated,
  Unchanged,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Value {
  Bool(bool),
  Num(u32),
}

impl ProcessingResult {
  pub fn update(&mut self) {
    *self = Updated;
  }
}

impl BitOr for ProcessingResult {
  type Output = ProcessingResult;

  fn bitor(self, rhs: Self) -> Self::Output {
    if self == Updated {
      Updated
    } else {
      rhs
    }
  }
}

pub struct PuzzleRealm<PuzzleState: Puzzle, PlayerState: PlayerPositions<Area = PuzzleState::Area, OutputIdentifier = PuzzleState::OutputIdentifier>>
{
  puzzle: PuzzleState,
  players: PlayerState,
}

impl<PuzzleState: Puzzle, PlayerState: PlayerPositions<Area = PuzzleState::Area, OutputIdentifier = PuzzleState::OutputIdentifier>> super::Controller
  for PuzzleRealm<PuzzleState, PlayerState>
{
  type Input = ();
  type Output = ();

  fn capabilities(&self) -> &BTreeSet<&'static str> {
    todo!()
  }

  fn next_timer(&self) -> Option<Duration> {
    match (self.puzzle.next_timer(), self.players.next_timer()) {
      (None, None) => None,
      (Some(t), None) | (None, Some(t)) => Some(t),
      (Some(z), Some(l)) => Some(z.min(l)),
    }
  }

  fn process(&mut self, input: ControllerInput<Self::Input, &str>) -> Vec<ControllerOutput<Self::Output>> {
    match input {
      ControllerInput::Add { .. } => todo!(),
      ControllerInput::Input { .. } => todo!(),
      ControllerInput::Remove { .. } => todo!(),
      ControllerInput::Timer => todo!(),
    }
  }

  fn serialize_message_pack(
    &self,
    serializer: MessagePackSerializer,
  ) -> Result<<MessagePackSerializer as Serializer>::Ok, <MessagePackSerializer as Serializer>::Error> {
    self.puzzle.serialize(serializer)
  }

  fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(&self.puzzle)
  }
}
impl From<Value> for bool {
  fn from(value: Value) -> Self {
    match value {
      Value::Bool(b) => b,
      Value::Num(n) => n != 0,
    }
  }
}
impl From<Value> for u32 {
  fn from(value: Value) -> Self {
    match value {
      Value::Bool(b) => {
        if b {
          1
        } else {
          0
        }
      }
      Value::Num(n) => n,
    }
  }
}
