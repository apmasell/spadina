pub trait GlobalValueCreator<Source: Sized> {
  type Converted: Clone + Send + Sync + 'static;
  fn as_bool(self, when_true: Source, when_false: Source, transition: spadina_core::asset::Transition) -> Self::Converted;
  fn as_mask(&mut self, mask: spadina_core::asset::ChoiceValue<'_, Source>) -> Self::Converted;
  fn as_num(self, default: Source, states: Vec<Source>, transition: spadina_core::asset::Transition) -> Self::Converted;
  fn as_setting(self) -> Option<Self::Converted>;
  fn as_setting_bool(self, when_true: Source, when_false: Source, transition: spadina_core::asset::Transition) -> Self::Converted;
  fn as_setting_num(self, default: Source, values: Vec<Source>, transition: spadina_core::asset::Transition) -> Self::Converted;
  fn convert(source: Source) -> Self::Converted;
  fn empty() -> Self::Converted;
}

pub trait BlendedValueCreator<Source: Clone>: GlobalValueCreator<Source> {
  fn as_gradiator(self) -> <Self as GlobalValueCreator<Source>>::Converted;
}
pub trait LocalWorldType<Source: Sized + Clone + 'static, WorldBuildingState, Update>: Clone + Send + Sync + 'static {
  type UpdateSource;
  fn altitude(bottom_limit: u32, bottom_value: Source, top_limit: u32, top_value: Source, state: &mut WorldBuildingState) -> Option<usize>;
  fn empty(state: &mut WorldBuildingState) -> Self;
  fn fixed(self, state: &mut WorldBuildingState) -> Self;
  fn gradiator(state: &mut WorldBuildingState, name: String) -> Option<usize>;
  fn mask(mask: spadina_core::asset::ChoiceValue<'_, Self>, state: &mut WorldBuildingState, x: u32, y: u32, z: u32) -> Option<(Self, Update)>;
  fn prepare_bool(
    id: String,
    when_true: Source,
    when_false: Source,
    transition: spadina_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_mask(mask: String, state: &mut WorldBuildingState) -> Option<usize>;
  fn prepare_num(
    id: String,
    default: Source,
    values: Vec<Source>,
    transition: spadina_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_setting(id: String, state: &mut WorldBuildingState) -> Option<usize>;
  fn prepare_setting_bool(
    id: String,
    when_true: Source,
    when_false: Source,
    transition: spadina_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_setting_num(
    id: String,
    default: Source,
    values: Vec<Source>,
    transition: spadina_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn register(id: usize, source: Self::UpdateSource, state: &mut WorldBuildingState, x: u32, y: u32, z: u32) -> Self;
}

#[derive(Clone)]
pub enum LocalBuiltValue<T> {
  Positional(usize),
  Fixed(T),
  RandomLocal(Vec<T>),
}

pub fn convert_global<Source: Masked, GVC: GlobalValueCreator<Source>>(
  value: spadina_core::asset::GlobalValue<Source>,
  seed: i32,
  masks: &std::collections::BTreeMap<String, spadina_core::asset::MaskConfiguration>,
  target: GVC,
) -> GVC::Converted {
  match value {
    spadina_core::asset::GlobalValue::Fixed(value) => GVC::convert(value),
    spadina_core::asset::GlobalValue::PuzzleBool { id, when_true, when_false, transition } => target.as_bool(when_true, when_false, transition),
    spadina_core::asset::GlobalValue::PuzzleNum { id, default, values, transition } => target.as_num(default, values, transition),
    spadina_core::asset::GlobalValue::Random(values) => GVC::convert(values[seed.abs() as usize % values.len()]),
    spadina_core::asset::GlobalValue::Setting(setting) => target.as_setting().unwrap_or_else(GVC::empty),
    spadina_core::asset::GlobalValue::SettingBool { id, when_true, when_false, transition } => {
      target.as_setting_bool(when_true, when_false, transition)
    }
    spadina_core::asset::GlobalValue::SettingNum { id, default, values, transition } => target.as_setting_num(default, values, transition),
    spadina_core::asset::GlobalValue::Masked(name) => match masks.get(&name) {
      Some(mask) => match Source::get_mask(mask) {
        Some(mask) => target.as_mask(mask),
        None => GVC::empty(),
      },
      None => GVC::empty(),
    },
  }
}
pub fn convert_local_discrete<Source: Masked + LocalValueBuilder>(
  value: spadina_core::asset::LocalDiscreteValue<Source>,
  seed: i32,
  masks: &std::collections::BTreeMap<String, spadina_core::asset::MaskConfiguration>,
  target: DVB,
) -> LocalBuiltValue<DVB::Converted> {
  match value {
    spadina_core::asset::LocalDiscreteValue::Global(value) => LocalBuiltValue::Fixed(convert_global(value, seed, masks, target)),
    spadina_core::asset::LocalDiscreteValue::RandomLocal(values) => LocalBuiltValue::RandomLocal(values.into_iter().map()),
  }
}

trait Masked: Sized {
  fn get_mask(mask: &spadina_core::asset::MaskConfiguration) -> Option<spadina_core::asset::ChoiceValue<'_, Self>>;
}
impl Masked for f64 {
  fn get_mask(mask: &spadina_core::asset::MaskConfiguration) -> Option<spadina_core::asset::ChoiceValue<'_, Self>> {
    match mask {
      spadina_core::asset::MaskConfiguration::Bool { intensity, .. } => match intensity {
        None => None,
        Some((when_true, when_false, transition)) => Some(spadina_core::asset::ChoiceValue::Bool(when_true, when_false, *transition)),
      },
      spadina_core::asset::MaskConfiguration::Num { intensity, .. } => match intensity {
        None => None,
        Some((default, values, transition)) => Some(spadina_core::asset::ChoiceValue::Num(default, values, *transition)),
      },
    }
  }
}
impl Masked for u32 {
  fn get_mask(mask: &spadina_core::asset::MaskConfiguration) -> Option<spadina_core::asset::ChoiceValue<'_, Self>> {
    match mask {
      spadina_core::asset::MaskConfiguration::Bool { material, .. } => match material {
        None => None,
        Some((when_true, when_false, transition)) => Some(spadina_core::asset::ChoiceValue::Bool(when_true, when_false, *transition)),
      },
      spadina_core::asset::MaskConfiguration::Num { material, .. } => match material {
        None => None,
        Some((default, values, transition)) => Some(spadina_core::asset::ChoiceValue::Num(default, values, *transition)),
      },
    }
  }
}
impl Masked for spadina_core::asset::Color {
  fn get_mask(mask: &spadina_core::asset::MaskConfiguration) -> Option<spadina_core::asset::ChoiceValue<'_, Self>> {
    match mask {
      spadina_core::asset::MaskConfiguration::Bool { color, .. } => match color {
        None => None,
        Some((when_true, when_false, transition)) => Some(spadina_core::asset::ChoiceValue::Bool(when_true, when_false, *transition)),
      },
      spadina_core::asset::MaskConfiguration::Num { color, .. } => match color {
        None => None,
        Some((default, values, transition)) => Some(spadina_core::asset::ChoiceValue::Num(default, values, *transition)),
      },
    }
  }
}
