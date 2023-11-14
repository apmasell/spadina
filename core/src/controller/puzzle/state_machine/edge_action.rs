#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EdgeAction<Variable> {
  Set { counter: super::CounterId, condition: super::expression::BoolExpression<Variable>, value: super::expression::NumberExpression<Variable> },
  Trigger { target: super::InternalEventId, condition: super::expression::BoolExpression<Variable> },
}
