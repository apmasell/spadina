use crate::convert::IntoBevy;

pub struct BoolUpdateState(pub std::sync::Arc<std::sync::atomic::AtomicBool>, pub bool);
pub struct NumUpdateState(pub std::sync::Arc<std::sync::atomic::AtomicU32>, pub u32);

pub struct Gradiator<T: crate::convert::IntoBevy> {
  sources: Vec<puzzleverse_core::asset::gradiator::Source<T, NumUpdateState, BoolUpdateState>>,
  points: std::collections::BTreeMap<(u32, u32, u32), Vec<T::GradiatorUpdate>>,
}
pub trait Change<T> {
  fn change(&self, value: &T, world: &mut bevy::ecs::world::World);
}

impl<T: crate::convert::IntoBevy> Gradiator<T> {
  pub fn get(&self, x: u32, y: u32, z: u32) -> T {
    T::mix(self.sources.iter().map(|s| (s.function.distance(x, y, z), Self::value(&s.source, z)))).expect("Failed to mix gradiator value")
  }
  pub fn update(&mut self, world: &mut bevy::ecs::world::World) {
    let mut update = false;
    for source in self.sources.iter_mut() {
      if Self::check_dirty(&mut source.source) {
        update = true;
      }
    }
    if update {
      for (&(x, y, z), targets) in self.points.iter() {
        let value =
          T::mix(self.sources.iter().map(|s| (s.function.distance(x, y, z), Self::value(&s.source, z)))).expect("Failed to mix gradiator value");
        for target in targets {
          target.change(&value, world);
        }
      }
    }
  }
  pub fn register(&mut self, x: u32, y: u32, z: u32, update: T::GradiatorUpdate) {
    self.points.entry((x, y, z)).or_default().push(update);
  }
  fn check_dirty(current: &mut puzzleverse_core::asset::gradiator::Current<T, NumUpdateState, BoolUpdateState>) -> bool {
    match current {
      puzzleverse_core::asset::gradiator::Current::Altitude { .. } => false,
      puzzleverse_core::asset::gradiator::Current::BoolControlled { value, .. } => {
        let current = value.0.load(std::sync::atomic::Ordering::Relaxed);
        if current == value.1 {
          false
        } else {
          value.1 = current;
          true
        }
      }
      puzzleverse_core::asset::gradiator::Current::Fixed(_) => false,
      puzzleverse_core::asset::gradiator::Current::NumControlled { value, .. } => {
        let current = value.0.load(std::sync::atomic::Ordering::Relaxed);
        if current == value.1 {
          false
        } else {
          value.1 = current;
          true
        }
      }
      puzzleverse_core::asset::gradiator::Current::Setting(_) => todo!(),
    }
  }
  fn value(current: &puzzleverse_core::asset::gradiator::Current<T, NumUpdateState, BoolUpdateState>, z: u32) -> T {
    match current {
      puzzleverse_core::asset::gradiator::Current::Altitude { top_value, top_altitude, bottom_value, bottom_altitude } => {
        if z <= *bottom_altitude {
          bottom_value.clone()
        } else if z >= *top_altitude {
          top_value.clone()
        } else {
          let ratio = (z - *bottom_altitude) as f64 / (*top_altitude - *bottom_altitude) as f64;
          T::mix(vec![(1.0 - ratio, top_value.clone()), (ratio, bottom_value.clone())]).expect("Failed to mix gradiator value")
        }
      }
      puzzleverse_core::asset::gradiator::Current::BoolControlled { value, when_true, when_false, .. } => {
        if value.1 {
          when_true.clone()
        } else {
          when_false.clone()
        }
      }
      puzzleverse_core::asset::gradiator::Current::Fixed(v) => v.clone(),
      puzzleverse_core::asset::gradiator::Current::NumControlled { value, values, default_value, .. } => {
        values.get(value.1 as usize).unwrap_or(default_value).clone()
      }
      puzzleverse_core::asset::gradiator::Current::Setting(_) => todo!(),
    }
  }
}
pub enum FloatUpdater {
  ChangeLightIntensity(bevy::ecs::entity::Entity),
}

impl Change<f64> for FloatUpdater {
  fn change(&self, value: &f64, world: &mut bevy::ecs::world::World) {
    match self {
      FloatUpdater::ChangeLightIntensity(entity) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.intensity = *value as f32 * crate::convert::MAX_ILLUMINATION;
        }
      }
    }
  }
}
pub enum ColorUpdater {
  ChangeLightColor(bevy::ecs::entity::Entity),
}

impl Change<puzzleverse_core::asset::Color> for ColorUpdater {
  fn change(&self, value: &puzzleverse_core::asset::Color, world: &mut bevy::ecs::world::World) {
    match self {
      ColorUpdater::ChangeLightColor(entity) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.color = crate::convert::convert_color(value.clone());
        }
      }
    }
  }
}
pub struct Unchanging;
impl<T: IntoBevy> Change<T> for Unchanging {
  fn change(&self, value: &T, world: &mut bevy::ecs::world::World) {
    eprintln!("Trying to change non-gradiated type.");
  }
}
