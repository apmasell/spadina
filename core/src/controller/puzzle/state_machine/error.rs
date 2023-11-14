use crate::controller::LoadError;
use std::fmt::{Debug, Display, Formatter};

pub enum StateMachineError {
  InvalidState { machine: usize, state: u8 },
  NoCounter { machine: usize, name: u8 },
  NoGlobal { machine: usize, name: u8 },
  NoLocal { machine: usize, name: u8 },
  NoStates(usize),
  VariableInGlobal,
}

impl Debug for StateMachineError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Display::fmt(self, f)
  }
}

impl Display for StateMachineError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      StateMachineError::InvalidState { machine, state } => {
        f.write_str("Invalid state “")?;
        Display::fmt(state, f)?;
        f.write_str("” for machine #")?;
        Display::fmt(machine, f)
      }
      StateMachineError::NoCounter { machine, name } => {
        f.write_str("Unknown counter “")?;
        Display::fmt(name, f)?;
        f.write_str("” in machine #")?;
        Display::fmt(machine, f)
      }
      StateMachineError::NoGlobal { machine, name } => {
        f.write_str("Unknown global variable “")?;
        Display::fmt(name, f)?;
        f.write_str("” in machine #")?;
        Display::fmt(machine, f)
      }
      StateMachineError::NoLocal { machine, name } => {
        f.write_str("Unknown local variable “")?;
        Display::fmt(name, f)?;
        f.write_str("” in machine #")?;
        Display::fmt(machine, f)
      }
      StateMachineError::NoStates(machine) => {
        f.write_str("No states in machine #")?;
        Display::fmt(machine, f)
      }
      StateMachineError::VariableInGlobal => f.write_str("Global variable has external reference."),
    }
  }
}

impl std::error::Error for StateMachineError {}
impl<E: std::error::Error> From<StateMachineError> for LoadError<E, StateMachineError> {
  fn from(value: StateMachineError) -> Self {
    LoadError::State(value)
  }
}
