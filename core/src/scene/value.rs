use crate::asset::ExtractChildren;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CustomValue<T, OutputIdentifier> {
  Fixed(T),
  PuzzleBool { id: OutputIdentifier, when_true: T, when_false: T, transition: Transition },
  PuzzleNum { id: OutputIdentifier, default: T, values: Vec<T>, transition: Transition },
  Random(Vec<T>),
  SettingBool { id: OutputIdentifier, when_true: T, when_false: T },
  SettingNum { id: OutputIdentifier, default: T, values: Vec<T> },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GlobalValue<T, OutputIdentifier, Setting> {
  Fixed(T),
  PuzzleBool { id: OutputIdentifier, when_true: T, when_false: T, transition: Transition },
  PuzzleNum { id: OutputIdentifier, default: T, values: Vec<T>, transition: Transition },
  Random(Vec<T>),
  Setting(Setting),
  SettingBool { id: OutputIdentifier, when_true: T, when_false: T, transition: Transition },
  SettingNum { id: OutputIdentifier, default: T, values: Vec<T>, transition: Transition },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocalBlendableValue<T, Gradiator, OutputIdentifier, Setting> {
  Altitude { top_value: T, bottom_value: T, top_limit: u32, bottom_limit: u32 },
  Global(GlobalValue<T, OutputIdentifier, Setting>),
  Gradiator(Gradiator),
  RandomLocal(Vec<T>),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocalDiscreteValue<T, OutputIdentifier, Setting> {
  Global(GlobalValue<T, OutputIdentifier, Setting>),
  RandomLocal(Vec<T>),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Transition {
  Instant,
  Fade(std::time::Duration),
}
impl<T, S: AsRef<str>> CustomValue<T, S> {
  pub fn map<R, E, M: FnMut(T) -> Result<R, E>>(self, mut mapper: M) -> Result<CustomValue<R, S>, E> {
    Ok(match self {
      CustomValue::Fixed(v) => CustomValue::Fixed(mapper(v)?),
      CustomValue::PuzzleBool { id, when_true, when_false, transition } => {
        CustomValue::PuzzleBool { id, when_true: mapper(when_true)?, when_false: mapper(when_false)?, transition }
      }
      CustomValue::PuzzleNum { id, default, values, transition } => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::PuzzleNum { id, default: mapper(default)?, values: new_values, transition }
      }
      CustomValue::Random(values) => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::Random(new_values)
      }
      CustomValue::SettingBool { id, when_true, when_false } => {
        CustomValue::SettingBool { id, when_true: mapper(when_true)?, when_false: mapper(when_false)? }
      }
      CustomValue::SettingNum { id, default, values } => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::SettingNum { id, default: mapper(default)?, values: new_values }
      }
    })
  }
}
impl<T: ExtractChildren<OutputIdentifier>, OutputIdentifier: AsRef<str>> ExtractChildren<OutputIdentifier> for CustomValue<T, OutputIdentifier> {
  fn extract_children(&self, assets: &mut std::collections::BTreeSet<OutputIdentifier>) {
    match self {
      CustomValue::Fixed(a) => {
        a.extract_children(assets);
      }
      CustomValue::PuzzleBool { when_true, when_false, .. } => {
        when_true.extract_children(assets);
        when_false.extract_children(assets);
      }
      CustomValue::PuzzleNum { default, values, .. } => {
        default.extract_children(assets);
        for value in values {
          value.extract_children(assets);
        }
      }
      CustomValue::Random(values) => {
        for value in values {
          value.extract_children(assets);
        }
      }
      CustomValue::SettingBool { when_true, when_false, .. } => {
        when_true.extract_children(assets);
        when_false.extract_children(assets);
      }
      CustomValue::SettingNum { default, values, .. } => {
        default.extract_children(assets);
        for value in values {
          value.extract_children(assets);
        }
      }
    }
  }
}
