use crate::controller::puzzle::state_machine::edge_action::EdgeAction;
use crate::controller::puzzle::state_machine::error::StateMachineError;
use crate::controller::puzzle::state_machine::state::{LocalData, LocalVariableName, TransitionVariableGenerator};
use crate::controller::puzzle::Value;
use chrono::{DateTime, NaiveDate, Utc};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum TimerDuration<Variable> {
  TimerDuration,
  #[serde(untagged)]
  Variable(Variable),
}

pub struct TimerDurationData<D> {
  pub data: D,
  pub duration: u32,
}

pub struct TimerDurationDataGenerator(pub u32);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TimerTransition<Variable> {
  None,
  Reset {
    duration: super::expression::NumberExpression<Variable>,
    next: super::StateId,
    actions: Vec<EdgeAction<TimerDuration<Variable>>>,
  },
  Rollover {
    duration: super::expression::NumberExpression<TimerDuration<Variable>>,
    next: super::StateId,
    actions: Vec<EdgeAction<TimerDuration<Variable>>>,
  },
}

impl<Variable, Data: super::expression::ExpressionData<Variable>> super::expression::ExpressionData<TimerDuration<Variable>>
  for TimerDurationData<Data>
{
  fn date(&self) -> NaiveDate {
    self.data.date()
  }

  fn seed(&self) -> u32 {
    self.data.seed()
  }

  fn variable(&self, name: &TimerDuration<Variable>) -> Result<Value, StateMachineError> {
    match name {
      TimerDuration::TimerDuration => Ok(Value::Num(self.duration)),
      TimerDuration::Variable(name) => self.data.variable(name),
    }
  }
}

impl TransitionVariableGenerator for TimerDurationDataGenerator {
  type Data<'a> = TimerDurationData<LocalData<'a>>;
  type Variable = TimerDuration<LocalVariableName>;

  fn data_for<'a>(&self, local_data: LocalData<'a>) -> Self::Data<'a> {
    TimerDurationData { data: local_data, duration: self.0 }
  }
}

impl<Variable> TimerTransition<Variable> {
  pub fn prepare<Data: super::expression::ExpressionData<Variable>>(
    &self,
    start: DateTime<Utc>,
    old: Option<DateTime<Utc>>,
    data: Data,
  ) -> Result<Option<DateTime<Utc>>, StateMachineError> {
    Ok(match self {
      TimerTransition::None => None,
      TimerTransition::Reset { duration, .. } => Some(start + chrono::Duration::seconds(duration.evaluate(&data)? as i64)),
      TimerTransition::Rollover { duration, .. } => Some(
        start
          + chrono::Duration::seconds(
            duration.evaluate(&TimerDurationData { data, duration: old.map(|o| (start - o).num_seconds().try_into().unwrap_or(0)).unwrap_or(0) })?
              as i64,
          ),
      ),
    })
  }
}
