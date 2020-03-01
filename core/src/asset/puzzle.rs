#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ArithmeticOperation {
  Add,
  Subtract,
  Multiply,
  Divide,
  Modulo,
  AbsoluteDifference,
}

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ComparatorType {
  Bool,
  Int,
}

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ComparatorOperation {
  Equals,
  NotEquals,
  LessThan,
  LessThanOrEqualTo,
  GreaterThan,
  GreaterThanOrEqualTo,
}

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ListType {
  Bool,
  Int,
  Realm,
}
#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LogicOperation {
  And,
  Or,
  ExclusiveOr,
  NAnd,
  NOr,
}

impl ArithmeticOperation {
  pub fn perform(&self, left: u32, right: u32) -> u32 {
    match self {
      ArithmeticOperation::Add => left + right,
      ArithmeticOperation::Subtract => left - right,
      ArithmeticOperation::Multiply => left * right,
      ArithmeticOperation::Divide => {
        if right == 0 {
          left
        } else {
          left / right
        }
      }
      ArithmeticOperation::Modulo => {
        if right == 0 {
          left
        } else {
          left % right
        }
      }
      ArithmeticOperation::AbsoluteDifference => crate::abs_difference(left, right),
    }
  }
}
impl ComparatorOperation {
  pub fn compare<T: PartialOrd>(&self, left: T, right: T) -> bool {
    match self {
      ComparatorOperation::Equals => left == right,
      ComparatorOperation::NotEquals => left != right,
      ComparatorOperation::LessThan => left < right,
      ComparatorOperation::LessThanOrEqualTo => left <= right,
      ComparatorOperation::GreaterThan => left > right,
      ComparatorOperation::GreaterThanOrEqualTo => left >= right,
    }
  }
}

impl LogicOperation {
  pub fn perform(&self, left: bool, right: bool) -> bool {
    match self {
      LogicOperation::And => left && right,
      LogicOperation::Or => left || right,
      LogicOperation::ExclusiveOr => left ^ right,
      LogicOperation::NAnd => !(left && right),
      LogicOperation::NOr => !(left || right),
    }
  }
}
