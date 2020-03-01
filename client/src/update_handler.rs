#[derive(Debug)]
pub enum SettingUpdate {
  AmbientLightColor,
  AmbientLightIntensity,
  ChangeLightColor(bevy::ecs::entity::Entity),
  ChangeLightIntensity(bevy::ecs::entity::Entity),
  BoolChangeMeshMaterial(
    bevy::ecs::entity::Entity,
    bevy::asset::Handle<bevy::pbr::StandardMaterial>,
    bevy::asset::Handle<bevy::pbr::StandardMaterial>,
  ),
  BoolAmbientLightColor(bevy::render::color::Color, bevy::render::color::Color),
  BoolAmbientLightIntensity(f64, f64),
  BoolChangeLightColor(bevy::ecs::entity::Entity, bevy::render::color::Color, bevy::render::color::Color),
  BoolChangeLightIntensity(bevy::ecs::entity::Entity, f64, f64),
  NumAmbientLightColor(bevy::render::color::Color, Vec<bevy::render::color::Color>),
  NumAmbientLightIntensity(f64, Vec<f64>),
  NumChangeMeshMaterial(bevy::ecs::entity::Entity, Vec<bevy::asset::Handle<bevy::pbr::StandardMaterial>>),
  NumChangeLightColor(bevy::ecs::entity::Entity, bevy::render::color::Color, Vec<bevy::render::color::Color>),
  NumChangeLightIntensity(bevy::ecs::entity::Entity, f64, Vec<f64>),
}
#[derive(Debug)]
pub enum Update {
  BoolShared(std::sync::Arc<std::sync::atomic::AtomicBool>),
  BoolAmbientLightColor(bevy::render::color::Color, bevy::render::color::Color, spadina_core::asset::Transition),
  BoolAmbientLightIntensity(f64, f64, spadina_core::asset::Transition),
  BoolChangeMeshMaterial(
    bevy::ecs::entity::Entity,
    bevy::asset::Handle<bevy::pbr::StandardMaterial>,
    bevy::asset::Handle<bevy::pbr::StandardMaterial>,
    spadina_core::asset::Transition,
  ),
  BoolChangeLightColor(bevy::ecs::entity::Entity, bevy::render::color::Color, bevy::render::color::Color, spadina_core::asset::Transition),
  BoolChangeLightIntensity(bevy::ecs::entity::Entity, f64, f64, spadina_core::asset::Transition),
  BoolVisibility(bevy::ecs::entity::Entity),
  NumShared(std::sync::Arc<std::sync::atomic::AtomicU32>),
  NumAmbientLightColor(bevy::render::color::Color, Vec<bevy::render::color::Color>, spadina_core::asset::Transition),
  NumAmbientLightIntensity(f64, Vec<f64>, spadina_core::asset::Transition),
  NumChangeMeshMaterial(
    bevy::ecs::entity::Entity,
    bevy::asset::Handle<bevy::pbr::StandardMaterial>,
    Vec<bevy::asset::Handle<bevy::pbr::StandardMaterial>>,
    spadina_core::asset::Transition,
  ),
  NumChangeLightColor(bevy::ecs::entity::Entity, bevy::render::color::Color, Vec<bevy::render::color::Color>, spadina_core::asset::Transition),
  NumChangeLightIntensity(bevy::ecs::entity::Entity, f64, Vec<f64>, spadina_core::asset::Transition),
}

impl Update {
  pub fn process(&self, value: &spadina_core::PropertyValue, world: &mut bevy::ecs::world::World) {
    match (value, self) {
      (spadina_core::PropertyValue::Bool(value), Update::BoolShared(atomic)) => {
        atomic.store(*value, std::sync::atomic::Ordering::Relaxed);
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolAmbientLightColor(when_true, when_false, _)) => {
        if let Some(light) = world.get_resource_mut::<bevy::pbr::AmbientLight>() {
          light.color = (if *value { when_true } else { when_false }).clone();
        }
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolAmbientLightIntensity(when_true, when_false, _)) => {
        if let Some(light) = world.get_resource_mut::<bevy::pbr::AmbientLight>() {
          light.brightness = *(if *value { when_true } else { when_false }) as f32 * crate::convert::MAX_ILLUMINATION;
        }
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolChangeMeshMaterial(entity, when_true, when_false, _)) => {
        if let Some(material) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::asset::Handle<bevy::pbr::StandardMaterial>>()).flatten() {
          *material = (if *value { when_true } else { when_false }).clone();
        }
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolChangeLightColor(entity, when_true, when_false, _)) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.color = (if *value { when_true } else { when_false }).clone();
        }
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolChangeLightIntensity(entity, when_true, when_false, _)) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.intensity = *(if *value { when_true } else { when_false }) as f32 * crate::convert::MAX_ILLUMINATION;
        }
      }
      (spadina_core::PropertyValue::Bool(value), Update::BoolVisibility(entity)) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::render::view::visibility::Visibility>()).flatten() {
          light.is_visible = *value;
        }
      }
      (spadina_core::PropertyValue::Num(value), Update::NumShared(atomic)) => {
        atomic.store(*value, std::sync::atomic::Ordering::Relaxed);
      }
      (spadina_core::PropertyValue::Num(value), Update::NumAmbientLightColor(default, colors, _)) => {
        if let Some(light) = world.get_resource_mut::<bevy::pbr::AmbientLight>() {
          light.color = colors.get(*value as usize).unwrap_or(default).clone();
        }
      }
      (spadina_core::PropertyValue::Num(value), Update::NumAmbientLightIntensity(default, intensities, _)) => {
        if let Some(light) = world.get_resource_mut::<bevy::pbr::AmbientLight>() {
          light.brightness = *intensities.get(*value as usize).unwrap_or(default) as f32 * crate::convert::MAX_ILLUMINATION;
        }
      }
      (spadina_core::PropertyValue::Num(value), Update::NumChangeMeshMaterial(entity, default, materials, _)) => {
        if let Some(mesh_material) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::asset::Handle<bevy::pbr::StandardMaterial>>()).flatten()
        {
          *mesh_material = materials.get(*value as usize).unwrap_or(default).clone();
        }
      }
      (spadina_core::PropertyValue::Num(value), Update::NumChangeLightColor(entity, default, colors, _)) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.color = colors.get(*value as usize).unwrap_or(default).clone();
        }
      }
      (spadina_core::PropertyValue::Num(value), Update::NumChangeLightIntensity(entity, default, intensities, _)) => {
        if let Some(light) = world.get_entity_mut(*entity).map(|e| e.get_mut::<bevy::pbr::PointLight>()).flatten() {
          light.intensity = *intensities.get(*value as usize).unwrap_or(default) as f32 * crate::convert::MAX_ILLUMINATION;
        }
      }
      _ => eprintln!("Sever sent an update {:?} that can't be processed for {:?}.", value, self),
    }
  }
}
