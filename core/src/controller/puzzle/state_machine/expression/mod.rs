use crate::controller::puzzle::area::PlayerStateCondition;
use crate::controller::puzzle::state_machine::error::StateMachineError;
use crate::controller::puzzle::{MultiStateValue, Value};

pub mod holiday;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum BoolExpression<Variable> {
  And(Box<BoolExpression<Variable>>, Box<BoolExpression<Variable>>),
  Constant(bool),
  Equal(NumberExpression<Variable>, NumberExpression<Variable>),
  GreaterThan(NumberExpression<Variable>, NumberExpression<Variable>),
  GreaterThanOrEqual(NumberExpression<Variable>, NumberExpression<Variable>),
  If(Box<BoolExpression<Variable>>, Box<BoolExpression<Variable>>, Box<BoolExpression<Variable>>),
  IsHoliday(Vec<holiday::Holiday>),
  LessThan(NumberExpression<Variable>, NumberExpression<Variable>),
  LessThanOrEqual(NumberExpression<Variable>, NumberExpression<Variable>),
  Not(Box<BoolExpression<Variable>>),
  NotEqual(NumberExpression<Variable>, NumberExpression<Variable>),
  Or(Box<BoolExpression<Variable>>, Box<BoolExpression<Variable>>),
  Variable(Variable),
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum NumberExpression<Variable> {
  Add(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  And(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Case(Box<NumberExpression<Variable>>, Vec<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Constant(u32),
  Divide(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  If(Box<BoolExpression<Variable>>, Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Mask { offset: u8, length: u8 },
  Max(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Min(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Modulo(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Multiply(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Or(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Permutation { seed: Box<NumberExpression<Variable>>, index: Box<NumberExpression<Variable>>, length: Box<NumberExpression<Variable>> },
  Seed,
  Subtract(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
  Variable(Variable),
  XOr(Box<NumberExpression<Variable>>, Box<NumberExpression<Variable>>),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VariableExpression<Variable> {
  Bool(BoolExpression<Variable>),
  Num(NumberExpression<Variable>),
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MultiStateVariableExpression<Variable> {
  Bool(BoolExpression<Variable>),
  Num(NumberExpression<Variable>),
  MultiBool { default: BoolExpression<Variable>, expressions: Vec<(PlayerStateCondition, BoolExpression<Variable>)> },
  MultiNum { default: NumberExpression<Variable>, expressions: Vec<(PlayerStateCondition, NumberExpression<Variable>)> },
}
pub trait ExpressionData<Variable> {
  fn date(&self) -> chrono::NaiveDate;
  fn seed(&self) -> u32;
  fn variable(&self, name: &Variable) -> Result<Value, StateMachineError>;
}

impl<Variable> BoolExpression<Variable> {
  pub fn evaluate<Data: ExpressionData<Variable>>(&self, data: &Data) -> Result<bool, StateMachineError> {
    match self {
      BoolExpression::And(left, right) => Ok(left.evaluate(data)? && right.evaluate(data)?),
      BoolExpression::Constant(value) => Ok(*value),
      BoolExpression::Equal(left, right) => Ok(left.evaluate(data)? == right.evaluate(data)?),
      BoolExpression::GreaterThan(left, right) => Ok(left.evaluate(data)? > right.evaluate(data)?),
      BoolExpression::GreaterThanOrEqual(left, right) => Ok(left.evaluate(data)? >= right.evaluate(data)?),
      BoolExpression::If(condition, when_true, when_false) => {
        if condition.evaluate(data)? {
          when_true.evaluate(data)
        } else {
          when_false.evaluate(data)
        }
      }
      BoolExpression::IsHoliday(holidays) => {
        let date = data.date();
        Ok(holidays.iter().any(|h| h.is_holiday(&date)))
      }
      BoolExpression::LessThan(left, right) => Ok(left.evaluate(data)? < right.evaluate(data)?),
      BoolExpression::LessThanOrEqual(left, right) => Ok(left.evaluate(data)? <= right.evaluate(data)?),
      BoolExpression::Not(expression) => Ok(!expression.evaluate(data)?),
      BoolExpression::NotEqual(left, right) => Ok(left.evaluate(data)? != right.evaluate(data)?),
      BoolExpression::Or(left, right) => Ok(left.evaluate(data)? || right.evaluate(data)?),
      BoolExpression::Variable(name) => Ok(data.variable(name)?.into()),
    }
  }
}
impl<Variable> NumberExpression<Variable> {
  pub fn evaluate<Data: ExpressionData<Variable>>(&self, data: &Data) -> Result<u32, StateMachineError> {
    match self {
      NumberExpression::Add(left, right) => Ok(left.evaluate(data)?.saturating_add(right.evaluate(data)?)),
      NumberExpression::And(left, right) => Ok(left.evaluate(data)? & right.evaluate(data)?),
      NumberExpression::Case(index, cases, default) => cases.get(index.evaluate(data)? as usize).unwrap_or(default).evaluate(data),
      NumberExpression::Constant(value) => Ok(*value),
      NumberExpression::Divide(left, right) => {
        let right = right.evaluate(data)?;
        Ok(if right == 0 { 0 } else { left.evaluate(data)?.saturating_div(right) })
      }
      NumberExpression::If(condition, when_true, when_false) => {
        if condition.evaluate(data)? {
          when_true.evaluate(data)
        } else {
          when_false.evaluate(data)
        }
      }
      NumberExpression::Mask { offset, length } => {
        let mask = 1_u32.checked_shl(*length as u32).map(|v| v - 1).unwrap_or(0);

        Ok(mask.checked_shl(*offset as u32).unwrap_or(0))
      }
      NumberExpression::Max(left, right) => Ok(left.evaluate(data)?.max(right.evaluate(data)?)),
      NumberExpression::Min(left, right) => Ok(left.evaluate(data)?.min(right.evaluate(data)?)),
      NumberExpression::Modulo(left, right) => {
        let right = right.evaluate(data)?;
        Ok(if right == 0 { 0 } else { left.evaluate(data)? % right })
      }
      NumberExpression::Multiply(left, right) => Ok(left.evaluate(data)?.saturating_add(right.evaluate(data)?)),
      NumberExpression::Or(left, right) => Ok(left.evaluate(data)? & right.evaluate(data)?),
      NumberExpression::Permutation { seed, index, length } => {
        let seed = seed.evaluate(data)? as u64;
        let index = index.evaluate(data)? as usize;
        let length = length.evaluate(data)? as usize;
        Ok(if length == 0 || index >= length {
          0
        } else {
          use rand::SeedableRng;
          let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
          rand::seq::index::sample(&mut rng, length, length).index(index).try_into().unwrap_or(0)
        })
      }
      NumberExpression::Seed => Ok(data.seed()),
      NumberExpression::Subtract(left, right) => Ok(left.evaluate(data)?.saturating_sub(right.evaluate(data)?)),
      NumberExpression::Variable(name) => Ok(data.variable(name)?.into()),
      NumberExpression::XOr(left, right) => Ok(left.evaluate(data)? ^ right.evaluate(data)?),
    }
  }
}
impl<Variable> VariableExpression<Variable> {
  pub fn evaluate<D: ExpressionData<Variable>>(&self, data: &D) -> Result<Value, StateMachineError> {
    Ok(match self {
      VariableExpression::Bool(e) => Value::Bool(e.evaluate(data)?),
      VariableExpression::Num(e) => Value::Num(e.evaluate(data)?),
    })
  }
}
impl<Variable> MultiStateVariableExpression<Variable> {
  pub fn evaluate<D: ExpressionData<Variable>>(&self, data: &D) -> Result<MultiStateValue, StateMachineError> {
    Ok(match self {
      MultiStateVariableExpression::Bool(e) => MultiStateValue::Bool(e.evaluate(data)?),
      MultiStateVariableExpression::Num(e) => MultiStateValue::Num(e.evaluate(data)?),
      MultiStateVariableExpression::MultiBool { default, expressions } => MultiStateValue::MultiBool {
        default: default.evaluate(data)?,
        values: expressions.iter().map(|(condition, e)| Ok((condition.clone(), e.evaluate(data)?))).collect::<Result<_, _>>()?,
      },
      MultiStateVariableExpression::MultiNum { default, expressions } => MultiStateValue::MultiNum {
        default: default.evaluate(data)?,
        values: expressions.iter().map(|(condition, e)| Ok((condition.clone(), e.evaluate(data)?))).collect::<Result<_, _>>()?,
      },
    })
  }
}
