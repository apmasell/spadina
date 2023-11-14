use crate::controller::puzzle;
use crate::controller::puzzle::state_machine::edge_action::EdgeAction;
use crate::controller::puzzle::state_machine::error::StateMachineError;
use crate::controller::puzzle::state_machine::expression::ExpressionData;
use crate::controller::puzzle::state_machine::{expression, timer_transition, transition, GlobalData, MachineIdentifier, StateId};
use crate::controller::puzzle::Value;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::BTreeSet;

#[derive(Clone)]
pub struct LocalData<'a> {
  state_data: StateData<'a>,
  locals: &'a [Value],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LocalVariableName {
  Local(u8),
  #[serde(untagged)]
  Other(StateVariableName),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct State {
  pub counters: Vec<u32>,
  pub current: StateId,
  pub local_values: Vec<Value>,
  pub outputs: Vec<puzzle::MultiStateValue>,
  pub timer: Option<DateTime<Utc>>,
}
#[derive(Clone)]
pub struct StateData<'a> {
  global_data: GlobalData<'a>,
  counters: &'a [u32],
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StateDefinition<InputIdentifier, Area> {
  pub outputs: Vec<expression::MultiStateVariableExpression<LocalVariableName>>,
  pub user_transitions: Vec<transition::Transition<InputIdentifier, Area, LocalVariableName>>,
  pub timer_transition: timer_transition::TimerTransition<LocalVariableName>,
  pub variables: Vec<expression::VariableExpression<StateVariableName>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StateMachine<InputIdentifier, OutputIdentifier, Area> {
  pub counters: u8,
  pub outputs: Vec<OutputIdentifier>,
  pub states: Vec<StateDefinition<InputIdentifier, Area>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum StateVariableName {
  Counter(u8),
  Global(u8),
}
pub trait TransitionVariableGenerator {
  type Data<'a>: ExpressionData<Self::Variable>;
  type Variable;
  fn data_for<'a>(&self, local_data: LocalData<'a>) -> Self::Data<'a>;
}

impl ExpressionData<LocalVariableName> for LocalData<'_> {
  fn date(&self) -> NaiveDate {
    self.state_data.date()
  }

  fn seed(&self) -> u32 {
    self.state_data.seed()
  }
  fn variable(&self, name: &LocalVariableName) -> Result<Value, StateMachineError> {
    match name {
      LocalVariableName::Local(l) => {
        self.locals.get(*l as usize).copied().ok_or(StateMachineError::NoLocal { machine: self.state_data.machine(), name: *l })
      }
      LocalVariableName::Other(o) => self.state_data.variable(o),
    }
  }
}
impl MachineIdentifier for LocalData<'_> {
  fn machine(&self) -> usize {
    self.state_data.machine()
  }
}

impl State {
  pub fn get_state_variable_data<'a>(&'a self, global_data: GlobalData<'a>) -> StateData<'a> {
    StateData { global_data, counters: &self.counters }
  }
  pub fn get_local_variable_data<'a>(&'a self, global_data: GlobalData<'a>) -> LocalData<'a> {
    LocalData { state_data: self.get_state_variable_data(global_data), locals: &self.local_values }
  }
  pub fn perform_transition<InputIdentifier, OutputIdentifier, Area, Generator: TransitionVariableGenerator>(
    &mut self,
    machine: usize,
    next: u8,
    actions: &[EdgeAction<Generator::Variable>],
    now: DateTime<Utc>,
    definition: &StateMachine<InputIdentifier, OutputIdentifier, Area>,
    global_data: GlobalData,
    transition_data: &Generator,
  ) -> Result<BTreeSet<super::InternalEventId>, StateMachineError> {
    let next_state = definition.states.get(next as usize).ok_or(StateMachineError::InvalidState { machine, state: next })?;
    let mut new_counters = self.counters.clone();
    let local_data = self.get_local_variable_data(global_data.clone());
    let mut triggers = BTreeSet::new();
    for action in actions {
      match action {
        EdgeAction::Set { counter, condition, value } => {
          let data = transition_data.data_for(local_data.clone());
          if condition.evaluate(&data)? {
            *new_counters.get_mut(*counter as usize).ok_or(StateMachineError::NoCounter { machine, name: *counter })? = value.evaluate(&data)?;
          }
        }
        EdgeAction::Trigger { target, condition } => {
          let data = transition_data.data_for(local_data.clone());
          if condition.evaluate(&data)? {
            triggers.insert(*target);
          }
        }
      }
    }
    let state_data = self.get_state_variable_data(global_data.clone());
    let new_locals = next_state.variables.iter().map(|v| v.evaluate(&state_data)).collect::<Result<Vec<_>, _>>()?;
    self.local_values = new_locals;

    let local_data = self.get_local_variable_data(global_data.clone());

    let new_outputs = next_state.outputs.iter().map(|v| v.evaluate(&local_data)).collect::<Result<Vec<_>, _>>()?;
    let new_timer = next_state.timer_transition.prepare(now, self.timer.clone(), local_data)?;
    self.timer = new_timer;
    self.outputs = new_outputs;
    self.current = next;
    Ok(triggers)
  }
}

impl ExpressionData<StateVariableName> for StateData<'_> {
  fn date(&self) -> NaiveDate {
    self.global_data.date()
  }

  fn seed(&self) -> u32 {
    self.global_data.seed()
  }
  fn variable(&self, name: &StateVariableName) -> Result<Value, StateMachineError> {
    match name {
      StateVariableName::Counter(c) => {
        self.counters.get(*c as usize).copied().map(Value::Num).ok_or(StateMachineError::NoCounter { machine: self.machine(), name: *c })
      }
      StateVariableName::Global(g) => self.global_data.variable(g),
    }
  }
}
impl MachineIdentifier for StateData<'_> {
  fn machine(&self) -> usize {
    self.global_data.machine()
  }
}

impl<InputIdentifier, OutputIdentifier, Area> StateMachine<InputIdentifier, OutputIdentifier, Area> {
  pub fn blank(&self, id: usize, now: DateTime<Utc>, global_data: GlobalData) -> Result<State, StateMachineError> {
    let counters = vec![0; self.counters as usize];
    let state_data = StateData { global_data, counters: &counters };
    let local_values: Vec<Value> =
      self.states.get(0).ok_or(StateMachineError::NoStates(id))?.variables.iter().map(|v| v.evaluate(&state_data)).collect::<Result<Vec<_>, _>>()?;
    let local_data = LocalData { state_data, locals: &local_values };
    let outputs = self.states[0].outputs.iter().map(|v| v.evaluate(&local_data)).collect::<Result<Vec<_>, _>>()?;
    let timer = self.states[0].timer_transition.prepare(now, None, local_data)?;
    Ok(State { counters, current: 0, local_values, outputs, timer })
  }
  pub fn matches_definition(&self, state: &State) -> bool {
    state.counters.len() == self.counters as usize
  }
}

impl<InputIdentifier, Area> StateDefinition<InputIdentifier, Area> {
  pub fn matches_definition(&self, state: &State) -> bool {
    state.local_values.len() == self.variables.len() && state.outputs.len() == self.outputs.len()
  }
}

impl TransitionVariableGenerator for () {
  type Data<'a> = LocalData<'a>;
  type Variable = LocalVariableName;

  fn data_for<'a>(&self, local_data: LocalData<'a>) -> Self::Data<'a> {
    local_data
  }
}
