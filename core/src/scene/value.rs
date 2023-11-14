use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GlobalValue<T, OutputIdentifier, Setting> {
  Fixed(T),
  PuzzleBool { id: OutputIdentifier, when_true: T, when_false: T, transition: Transition },
  PuzzleNum { id: OutputIdentifier, default: T, values: Vec<T>, transition: Transition },
  Random(Vec<T>),
  Setting(Setting),
  SettingBool { id: Setting, when_true: T, when_false: T, transition: Transition },
  SettingNum { id: Setting, default: T, values: Vec<T>, transition: Transition },
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
impl<T, OutputIdentifier: Ord, Setting: Ord> GlobalValue<T, OutputIdentifier, Setting> {
  pub fn validate<'a>(
    &'a self,
    output_identifiers: &mut BTreeSet<&'a OutputIdentifier>,
    settings: &mut BTreeSet<&'a Setting>,
  ) -> Result<(), Cow<'static, str>> {
    match self {
      GlobalValue::Fixed(_) => Ok(()),
      GlobalValue::PuzzleBool { id, .. } => {
        output_identifiers.insert(id);
        Ok(())
      }
      GlobalValue::PuzzleNum { id, .. } => {
        output_identifiers.insert(id);
        Ok(())
      }
      GlobalValue::Random(items) => {
        if items.is_empty() {
          Err(Cow::Borrowed("Random contains no values"))
        } else {
          Ok(())
        }
      }
      GlobalValue::Setting(id) => {
        settings.insert(id);
        Ok(())
      }
      GlobalValue::SettingBool { id, .. } => {
        settings.insert(id);
        Ok(())
      }
      GlobalValue::SettingNum { id, .. } => {
        settings.insert(id);
        Ok(())
      }
    }
  }
}
impl<T, OutputIdentifier: Ord, Setting: Ord> LocalDiscreteValue<T, OutputIdentifier, Setting> {
  pub fn validate<'a>(
    &'a self,
    output_identifiers: &mut BTreeSet<&'a OutputIdentifier>,
    settings: &mut BTreeSet<&'a Setting>,
  ) -> Result<(), Cow<'static, str>> {
    match self {
      LocalDiscreteValue::Global(g) => g.validate(output_identifiers, settings),
      LocalDiscreteValue::RandomLocal(items) => {
        if items.is_empty() {
          Err(Cow::Borrowed("Local random contains no values"))
        } else {
          Ok(())
        }
      }
    }
  }
}
impl<T, Gradiator: Ord, OutputIdentifier: Ord, Setting: Ord> LocalBlendableValue<T, Gradiator, OutputIdentifier, Setting> {
  pub fn validate<'a>(
    &'a self,
    output_identifiers: &mut BTreeSet<&'a OutputIdentifier>,
    gradiators: &mut BTreeSet<&'a Gradiator>,
    settings: &mut BTreeSet<&'a Setting>,
  ) -> Result<(), Cow<'static, str>> {
    match self {
      LocalBlendableValue::Altitude { bottom_limit, top_limit, .. } => {
        if top_limit <= bottom_limit {
          Err(Cow::Owned(format!("Bottom limit of {} is at or above top limit of {} in altitude blending", bottom_limit, top_limit)))
        } else {
          Ok(())
        }
      }
      LocalBlendableValue::Global(g) => g.validate(output_identifiers, settings),
      LocalBlendableValue::RandomLocal(items) => {
        if items.is_empty() {
          Err(Cow::Borrowed("Local random contains no values"))
        } else {
          Ok(())
        }
      }
      LocalBlendableValue::Gradiator(g) => {
        gradiators.insert(g);
        Ok(())
      }
    }
  }
}
