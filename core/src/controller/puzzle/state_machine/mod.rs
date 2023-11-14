use crate::controller::puzzle::area::AreaCounter;
use crate::controller::puzzle::state_machine::timer_transition::TimerDurationDataGenerator;
use crate::controller::puzzle::state_machine::transition::InputTrigger;
use crate::controller::puzzle::Value;
use crate::controller::{puzzle, LoadError};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use error::StateMachineError;
use serde::{Deserializer, Serializer};
use state::{State, StateMachine};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{Debug, Display};
use std::time::Duration;

mod edge_action;
pub mod error;
pub mod expression;
pub mod state;
pub mod timer_transition;
pub mod transition;

pub type CounterId = u8;
pub type InternalEventId = u8;
pub type StateId = u8;

#[derive(Clone)]
pub struct RootData {
  seed: u32,
  date: NaiveDate,
}
#[derive(Clone)]
pub struct GlobalData<'a> {
  id: usize,
  root: RootData,
  globals: &'a [Value],
}

impl expression::ExpressionData<u8> for GlobalData<'_> {
  fn date(&self) -> NaiveDate {
    self.root.date()
  }

  fn seed(&self) -> u32 {
    self.root.seed()
  }

  fn variable(&self, name: &u8) -> Result<Value, StateMachineError> {
    Ok(self.globals.get(*name as usize).copied().ok_or(StateMachineError::NoGlobal { machine: self.id, name: *name })?.into())
  }
}

pub trait MachineIdentifier {
  fn machine(&self) -> usize;
}

pub struct StateMachinePuzzle<Template> {
  pub state: StateMachinePuzzleState,
  pub template: Template,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StateMachinePuzzleState {
  pub global_values: Vec<Value>,
  pub last_time: DateTime<Utc>,
  pub seed: u32,
  pub states: Vec<State>,
  pub timezone: Tz,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StateMachinePuzzleTemplate<InputIdentifier, OutputIdentifier, Area> {
  pub machines: Vec<StateMachine<InputIdentifier, OutputIdentifier, Area>>,
  pub variables: Vec<expression::VariableExpression<()>>,
}

impl MachineIdentifier for GlobalData<'_> {
  fn machine(&self) -> usize {
    self.id
  }
}

impl expression::ExpressionData<()> for RootData {
  fn date(&self) -> NaiveDate {
    self.date
  }

  fn seed(&self) -> u32 {
    self.seed
  }

  fn variable(&self, _: &()) -> Result<Value, StateMachineError> {
    Err(StateMachineError::VariableInGlobal)
  }
}

impl<T: StateMachinePuzzleTemplateSource + Clone> super::PuzzleTemplate for T
where
  T::Area: Eq,
  T::InputIdentifier: Eq,
{
  type Error = StateMachineError;
  type Puzzle = StateMachinePuzzle<Self>;

  fn blank(&self, now: DateTime<Tz>, seed: u32) -> Result<Self::Puzzle, Self::Error> {
    let date = now.naive_local().date();
    let timezone = now.timezone();
    let now = now.with_timezone(&Utc);
    let root_data = RootData { seed, date };
    let global_values = self.template().variables.iter().map(|v| v.evaluate(&root_data)).collect::<Result<Vec<_>, _>>()?;
    let states = self
      .template()
      .machines
      .iter()
      .enumerate()
      .map(|(id, s)| s.blank(id, now, GlobalData { id, root: root_data.clone(), globals: &global_values }))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(StateMachinePuzzle { state: StateMachinePuzzleState { global_values, last_time: now, seed, states, timezone }, template: self.clone() })
  }

  fn load<'de, D: Deserializer<'de>>(&self, de: D, _: DateTime<Utc>) -> Result<Self::Puzzle, LoadError<D::Error, Self::Error>> {
    use serde::Deserialize;
    let state = StateMachinePuzzleState::deserialize(de).map_err(LoadError::Deserialization)?;
    if state.global_values.len() != self.template().variables.len() || state.states.len() != self.template().machines.len() {
      return Err(LoadError::DeserializationMismatch);
    }
    for (definition, current) in self.template().machines.iter().zip(&state.states) {
      if !definition.states.get(current.current as usize).ok_or(LoadError::DeserializationMismatch)?.matches_definition(current)
        || !definition.matches_definition(current)
      {
        return Err(LoadError::DeserializationMismatch);
      }
    }
    Ok(StateMachinePuzzle { state, template: self.clone() })
  }
}

impl<Template> serde::Serialize for StateMachinePuzzle<Template> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.state.serialize(serializer)
  }
}

pub trait StateMachinePuzzleTemplateSource: Send + 'static {
  type InputIdentifier: Send + 'static;
  type OutputIdentifier: Send + 'static;
  type Area: Send + 'static;
  fn template(&self) -> &StateMachinePuzzleTemplate<Self::InputIdentifier, Self::OutputIdentifier, Self::Area>;
}

impl<InputIdentifier: Send + 'static, OutputIdentifier: Send + 'static, Area: Send + 'static> StateMachinePuzzleTemplateSource
  for StateMachinePuzzleTemplate<InputIdentifier, OutputIdentifier, Area>
{
  type InputIdentifier = InputIdentifier;
  type OutputIdentifier = OutputIdentifier;
  type Area = Area;

  fn template(&self) -> &StateMachinePuzzleTemplate<Self::InputIdentifier, Self::OutputIdentifier, Self::Area> {
    self
  }
}
impl<Template: StateMachinePuzzleTemplateSource> StateMachinePuzzle<Template>
where
  Template::InputIdentifier: Eq,
  Template::Area: Eq,
{
  fn process_triggers(
    &mut self,
    now: DateTime<Utc>,
    mut triggers: BTreeSet<InternalEventId>,
    mut serviced: BTreeSet<StateId>,
  ) -> Result<(), StateMachineError> {
    let root_data = self.state.get_root_variable_data(now);
    while !triggers.is_empty() {
      let mut new_triggers = BTreeSet::new();
      let new_serviced = self
        .template
        .template()
        .machines
        .iter()
        .zip(&mut self.state.states)
        .enumerate()
        .filter(|(machine, _)| !serviced.contains(&(*machine as u8)))
        .flat_map(|(machine, (definition, state))| {
          let Some(state_definition) = definition.states.get(state.current as usize) else {
            return Some(Err(StateMachineError::InvalidState { machine, state: state.current }));
          };
          let local_data = state.get_local_variable_data(GlobalData { id: machine, root: root_data.clone(), globals: &self.state.global_values });
          let transition = match state_definition
            .user_transitions
            .iter()
            .filter_map(|transition| transition.should_trigger_internal(&triggers, &local_data))
            .next()
            .transpose()
          {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
          };
          if let Some(transition) = transition {
            match state.perform_transition(
              machine,
              transition.next,
              &transition.actions,
              now,
              definition,
              GlobalData { id: machine, root: root_data.clone(), globals: &self.state.global_values },
              &(),
            ) {
              Ok(triggers) => {
                new_triggers.extend(triggers);
                Some(Ok(machine as StateId))
              }
              Err(e) => Some(Err(e)),
            }
          } else {
            None
          }
        })
        .collect::<Result<Vec<_>, _>>()?;

      serviced.extend(new_serviced);
      triggers = new_triggers;
    }
    Ok(())
  }
}
impl<Template: StateMachinePuzzleTemplateSource> super::Puzzle for StateMachinePuzzle<Template>
where
  Template::InputIdentifier: Eq,
  Template::Area: Eq,
{
  type Area = Template::Area;
  type Error = StateMachineError;
  type InputIdentifier = Template::InputIdentifier;
  type OutputIdentifier = Template::OutputIdentifier;

  fn next_timer(&self) -> Option<Duration> {
    self.state.states.iter().flat_map(|s| s.timer.as_ref()).min().map(|&t| (t - Utc::now()).to_std().ok()).flatten()
  }

  fn outputs(&self) -> Box<dyn Iterator<Item = (&Self::OutputIdentifier, &puzzle::MultiStateValue)> + '_> {
    Box::new(
      self.state.states.iter().zip(&self.template.template().machines).flat_map(|(state, definition)| definition.outputs.iter().zip(&state.outputs)),
    )
  }

  fn process<Counter: AreaCounter>(
    &mut self,
    input: puzzle::PuzzleInput<Self::InputIdentifier, Self::Area, Counter>,
    now: DateTime<Utc>,
  ) -> Result<super::ProcessingResult, Self::Error> {
    let mut triggers = BTreeSet::new();
    let root_data = self.state.get_root_variable_data(now);
    let serviced = self
      .template
      .template()
      .machines
      .iter()
      .zip(&mut self.state.states)
      .enumerate()
      .flat_map(|(machine, (definition, state))| {
        let Some(state_definition) = definition.states.get(state.current as usize) else {
          return Some(Err(StateMachineError::InvalidState { machine, state: state.current }));
        };
        let local_data = state.get_local_variable_data(GlobalData { id: machine, root: root_data.clone(), globals: &self.state.global_values });
        let transition =
          match state_definition.user_transitions.iter().filter_map(|transition| transition.should_trigger(&input, &local_data)).next().transpose() {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
          };
        if let Some(transition) = transition {
          match state.perform_transition(
            machine,
            transition.next,
            &transition.actions,
            now,
            definition,
            GlobalData { id: machine, root: root_data.clone(), globals: &self.state.global_values },
            &(),
          ) {
            Ok(t) => {
              triggers.extend(t);
              Some(Ok(machine as StateId))
            }
            Err(e) => Some(Err(e)),
          }
        } else {
          None
        }
      })
      .collect::<Result<BTreeSet<_>, _>>()?;
    let output = if serviced.is_empty() { super::ProcessingResult::Unchanged } else { super::ProcessingResult::Updated };
    self.process_triggers(now, triggers, serviced)?;
    Ok(output)
  }

  fn process_timer(&mut self, now: DateTime<Utc>) -> Result<super::ProcessingResult, Self::Error> {
    let mut triggers = BTreeSet::new();
    let root_data = self.state.get_root_variable_data(now);
    let serviced = self
      .template
      .template()
      .machines
      .iter()
      .zip(&mut self.state.states)
      .enumerate()
      .flat_map(|(machine, (definition, state))| {
        if state.timer.map(|t| t < now).unwrap_or(false) {
          let Some(state_definition) = definition.states.get(state.current as usize) else {
            return Some(Err(StateMachineError::InvalidState { machine, state: state.current }));
          };
          let (next, actions) = match &state_definition.timer_transition {
            timer_transition::TimerTransition::Reset { next, actions, .. } => (next, actions),
            timer_transition::TimerTransition::Rollover { next, actions, .. } => (next, actions),
            timer_transition::TimerTransition::None => return None,
          };
          match state.perform_transition(
            machine,
            *next,
            actions,
            now,
            definition,
            GlobalData { id: machine, root: root_data.clone(), globals: &self.state.global_values },
            &TimerDurationDataGenerator(state.timer.clone().map(|t| u32::try_from((t - now).num_seconds()).unwrap_or(0)).unwrap_or(0)),
          ) {
            Ok(new_triggers) => {
              triggers.extend(new_triggers);
              Some(Ok(machine as StateId))
            }
            Err(e) => Some(Err(e)),
          }
        } else {
          None
        }
      })
      .collect::<Result<BTreeSet<_>, _>>()?;

    let output = if serviced.is_empty() { super::ProcessingResult::Unchanged } else { super::ProcessingResult::Updated };
    self.process_triggers(now, triggers, serviced)?;
    Ok(output)
  }
}

impl StateMachinePuzzleState {
  pub fn get_root_variable_data(&self, now: DateTime<Utc>) -> RootData {
    let date = now.with_timezone(&self.timezone).date_naive();
    RootData { seed: self.seed, date }
  }
  pub fn get_variable_data(&self, now: DateTime<Utc>, id: usize) -> GlobalData {
    GlobalData { id, root: self.get_root_variable_data(now), globals: &self.global_values }
  }
}
impl<InputIdentifier: Ord + Display, OutputIdentifier: Ord + Display, Area: Ord + Display>
  StateMachinePuzzleTemplate<InputIdentifier, OutputIdentifier, Area>
{
  pub fn validate(
    &self,
    areas: BTreeSet<&Area>,
    inputs: BTreeSet<&InputIdentifier>,
    outputs: BTreeSet<&OutputIdentifier>,
  ) -> Result<(), Cow<'static, str>> {
    let mut known_outputs = BTreeSet::new();
    for (index, machine) in self.machines.iter().enumerate() {
      for output in &machine.outputs {
        if !known_outputs.insert(output) {
          return Err(Cow::Owned(format!("Machine {} has output {}, but this is already produced by another machine", index, output)));
        }
      }
      for (state_id, state) in machine.states.iter().enumerate() {
        for transition in &state.user_transitions {
          match &transition.input {
            InputTrigger::Click { source, .. } => {
              if !inputs.contains(source) {
                return Err(Cow::Owned(format!(
                  "In machine {}, state {} expects input {}, but this does not exist in the world",
                  index, state_id, source
                )));
              }
            }
            InputTrigger::Count { area, .. } => {
              if !areas.contains(area) {
                return Err(Cow::Owned(format!(
                  "In machine {}, state {} expects area {}, but this does not exist in the world",
                  index, state_id, area
                )));
              }
            }
            InputTrigger::Internal { .. } => {}
          }
        }
      }
    }

    for output in outputs {
      if !known_outputs.contains(output) {
        return Err(Cow::Owned(format!("World uses output {}, but no machine produces it", output)));
      }
    }

    Ok(())
  }
}
