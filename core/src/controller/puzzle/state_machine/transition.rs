use crate::controller::puzzle;
use crate::controller::puzzle::area::{AreaCounter, PlayerStateCondition};
use crate::controller::puzzle::state_machine::edge_action::EdgeAction;
use crate::controller::puzzle::state_machine::error::StateMachineError;
use crate::controller::puzzle::state_machine::state::{LocalData, LocalVariableName};
use crate::controller::puzzle::state_machine::InternalEventId;
use crate::controller::puzzle::Value;
use chrono::NaiveDate;
use std::collections::BTreeSet;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum AreaCount<Variable> {
  PlayerCount(PlayerStateCondition),
  #[serde(untagged)]
  Variable(Variable),
}

pub struct AreaCountData<'a, C: AreaCounter, D>(pub &'a C, pub D);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum InputTrigger<InputIdentifier, Area, Variable> {
  Click { source: InputIdentifier, player_condition: PlayerStateCondition, condition: super::expression::BoolExpression<Variable> },
  Count { area: Area, condition: super::expression::BoolExpression<AreaCount<Variable>> },
  Internal { source: InternalEventId, condition: super::expression::BoolExpression<Variable> },
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Transition<InputIdentifier, Area, Variable> {
  pub input: InputTrigger<InputIdentifier, Area, Variable>,
  pub next: super::StateId,
  pub actions: Vec<EdgeAction<Variable>>,
}

impl<'a, Variable, Counter: AreaCounter, Data: super::expression::ExpressionData<Variable>> super::expression::ExpressionData<AreaCount<Variable>>
  for AreaCountData<'a, Counter, Data>
{
  fn date(&self) -> NaiveDate {
    self.1.date()
  }

  fn seed(&self) -> u32 {
    self.1.seed()
  }

  fn variable(&self, name: &AreaCount<Variable>) -> Result<Value, StateMachineError> {
    match name {
      AreaCount::PlayerCount(filter) => Ok(Value::Num(self.0.count_players(filter))),
      AreaCount::Variable(name) => self.1.variable(name),
    }
  }
}

impl<InputIdentifier: Eq, Area: Eq> Transition<InputIdentifier, Area, LocalVariableName> {
  pub fn should_trigger<AreaCount: AreaCounter>(
    &self,
    input: &puzzle::PuzzleInput<InputIdentifier, Area, AreaCount>,
    local_data: &LocalData,
  ) -> Option<Result<&Self, StateMachineError>> {
    let keep = match (input, &self.input) {
      (puzzle::PuzzleInput::Click(name, player_state), InputTrigger::Click { source, player_condition, condition })
        if source == name && player_condition.check(*player_state) =>
      {
        match condition.evaluate(local_data) {
          Err(e) => return Some(Err(e)),
          Ok(keep) => Some(keep),
        }
      }
      (puzzle::PuzzleInput::Area(name, counter), InputTrigger::Count { area, condition }) if area == name => {
        match condition.evaluate(&AreaCountData(counter, local_data.clone())) {
          Err(e) => return Some(Err(e)),
          Ok(keep) => Some(keep),
        }
      }
      _ => None,
    }?;
    if keep {
      Some(Ok(self))
    } else {
      None
    }
  }
  pub fn should_trigger_internal(&self, input: &BTreeSet<InternalEventId>, local_data: &LocalData) -> Option<Result<&Self, StateMachineError>> {
    if let InputTrigger::Internal { source, condition } = &self.input {
      if input.contains(source) {
        match condition.evaluate(local_data) {
          Err(e) => Some(Err(e)),
          Ok(true) => Some(Ok(self)),
          Ok(false) => None,
        }
      } else {
        None
      }
    } else {
      None
    }
  }
}
