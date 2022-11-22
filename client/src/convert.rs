pub(crate) const MAX_ILLUMINATION: f32 = 4000.0;

pub trait UpdateSource<T: IntoBevy> {
  fn as_bool(
    self,
    when_true: T,
    when_false: T,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (T::Bevy, crate::update_handler::Update);
  fn as_num(
    self,
    default: T,
    states: Vec<T>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (T::Bevy, crate::update_handler::Update);
  fn as_gradiator(self, store: &mut WorldBuildingState) -> Option<(T::Bevy, T::GradiatorUpdate)>;
  fn as_setting(self, store: &mut WorldBuildingState) -> Option<(T::Bevy, crate::update_handler::SettingUpdate)>;
  fn as_setting_bool(
    self,
    when_true: T,
    when_false: T,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (T::Bevy, Option<crate::update_handler::SettingUpdate>);
  fn as_setting_num(
    self,
    default: T,
    values: Vec<T>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (T::Bevy, Option<crate::update_handler::SettingUpdate>);
}
pub(crate) trait ExtractArgument {
  type LocalName: ?Sized;
  fn extract_color(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::render::color::Color>;
  fn extract_color_local(
    &self,
    name: &Self::LocalName,
    seed: i32,
    state: &mut WorldBuildingState,
  ) -> Option<LocalBevyValue<puzzleverse_core::asset::Color>>;
  fn extract_light_intensity(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<f32>;
  fn extract_light_intensity_local(&self, name: &Self::LocalName, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<f64>>;
  fn extract_material(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::asset::Handle<bevy::pbr::StandardMaterial>>;
  fn extract_material_local(&self, name: &Self::LocalName, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<u32>>;
}

pub trait IntoBevy: Sized + Clone + 'static {
  type Bevy: Clone + Send + Sync + 'static;
  type GradiatorUpdate: crate::gradiator::Change<Self>;
  fn convert(self) -> Self::Bevy;
  fn empty() -> Self::Bevy;
  fn gradiator<'a>(state: &'a mut WorldBuildingState) -> Option<&'a mut std::collections::BTreeMap<String, crate::gradiator::Gradiator<Self>>>;
  fn locals<'a>(state: &'a mut WorldBuildingState) -> &'a mut Vec<Locals<Self>>;
  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: impl UpdateSource<Self>,
    state: &mut WorldBuildingState,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)>;
  fn mix<'a>(values: impl IntoIterator<Item = (f64, Self)>) -> Option<Self>;
}
pub trait IntoLocalBevy: Sized + Clone + 'static {
  type Bevy: Clone + Send + Sync + 'static;
  type UpdateSource;
  fn altitude(bottom_limit: u32, bottom_value: Self, top_limit: u32, top_value: Self, state: &mut WorldBuildingState) -> Option<usize>;
  fn empty(state: &mut WorldBuildingState) -> Self::Bevy;
  fn fixed(self, state: &mut WorldBuildingState) -> Self::Bevy;
  fn gradiator(state: &mut WorldBuildingState, name: String) -> Option<usize>;
  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: Self::UpdateSource,
    state: &mut WorldBuildingState,
    x: u32,
    y: u32,
    z: u32,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)>;
  fn prepare_bool(
    id: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_mask(mask: String, state: &mut WorldBuildingState) -> Option<usize>;
  fn prepare_num(
    id: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_setting(id: String, state: &mut WorldBuildingState) -> Option<usize>;
  fn prepare_setting_bool(
    id: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn prepare_setting_num(
    id: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize;
  fn register(id: usize, source: Self::UpdateSource, state: &mut WorldBuildingState, x: u32, y: u32, z: u32) -> Self::Bevy;
}
pub enum ArgumentValue {
  Material(bevy::asset::Handle<bevy::pbr::StandardMaterial>),
  BoolMaterial(bool, bevy::asset::Handle<bevy::pbr::StandardMaterial>, bevy::asset::Handle<bevy::pbr::StandardMaterial>),
  Color(puzzleverse_core::asset::LocalValue<puzzleverse_core::asset::Color>),
  Intensity(puzzleverse_core::asset::LocalValue<f64>),
}

pub struct AmbientLight;
pub struct LightEntity(pub bevy::ecs::entity::Entity);
#[derive(Clone)]
pub enum LocalBevyValue<T: IntoLocalBevy> {
  Positional(usize),
  Fixed(T::Bevy),
  RandomLocal(Vec<T::Bevy>),
}

pub enum LocalUpdate<T: IntoLocalBevy> {
  Bool(String, T::Bevy, T::Bevy),
}
pub struct WorldBuildingState<'a> {
  pub default_material: &'a mut bevy::asset::Handle<bevy::pbr::StandardMaterial>,
  pub locals_color: Vec<Locals<puzzleverse_core::asset::Color>>,
  pub locals_intensity: Vec<Locals<f64>>,
  pub gradiators_intensity: &'a mut std::collections::BTreeMap<String, crate::gradiator::Gradiator<f64>>,
  pub gradiators_color: &'a mut std::collections::BTreeMap<String, crate::gradiator::Gradiator<puzzleverse_core::asset::Color>>,
  pub masks: &'a std::collections::BTreeMap<String, puzzleverse_core::asset::MaskConfiguration>,
  pub materials: &'a Vec<LocalBevyValue<u32>>,
  pub material_builders: &'a mut Vec<crate::materials::MaterialBuilder>,
  pub meshes: &'a mut bevy::prelude::Assets<bevy::prelude::Mesh>,
  pub occupied: std::collections::BTreeSet<(u32, u32)>,
  pub seed: i32,
  pub settings: &'a mut std::collections::HashMap<String, Vec<crate::update_handler::SettingUpdate>>,
  pub updates: &'a mut std::collections::HashMap<puzzleverse_core::PropertyKey, Vec<crate::update_handler::Update>>,
}
enum Locals<T: IntoBevy> {
  AltitudeMixer(crate::altitude_mixer::AltitudeMixer<T>),
  Gradiator(String),
  Masked(String),
  PuzzleBool(String, T, T, puzzleverse_core::asset::Transition),
  PuzzleNum(String, T, Vec<T>, puzzleverse_core::asset::Transition),
  Setting(String),
  SettingBool(String, T, T, puzzleverse_core::asset::Transition),
  SettingNum(String, T, Vec<T>, puzzleverse_core::asset::Transition),
}

pub fn convert_mesh(
  meshes: &mut bevy::prelude::Assets<bevy::prelude::Mesh>,
  mesh: puzzleverse_core::asset::SprayModelElement,
) -> (bevy::asset::Handle<bevy::render::mesh::Mesh>, u32) {
  use bevy::render::mesh::*;
  (
    meshes.add(match mesh.mesh {
      puzzleverse_core::asset::Mesh::Triangle { elements } => {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(elements.into_iter().map(|(x, y, z)| [x, y, z]).collect()));
        mesh.set_indices(Some(Indices::U32(vec![0, 1, 2])));
        mesh
      }
    }),
    mesh.material,
  )
}

pub(crate) fn transform(transformation: puzzleverse_core::asset::Transformation) -> bevy::transform::components::Transform {
  use bevy::transform::components::Transform;
  match transformation {
    puzzleverse_core::asset::Transformation::N => Transform::identity(),
    puzzleverse_core::asset::Transformation::H => Transform::from_scale(bevy::math::vec3(-1.0, 1.0, 1.0)),
    puzzleverse_core::asset::Transformation::V => Transform::from_scale(bevy::math::vec3(1.0, -1.0, 1.0)),
    puzzleverse_core::asset::Transformation::C => Transform::from_rotation(bevy::math::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    puzzleverse_core::asset::Transformation::A => Transform::from_rotation(bevy::math::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    puzzleverse_core::asset::Transformation::AV => {
      Transform::from_rotation(bevy::math::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(bevy::math::vec3(1.0, -1.0, 1.0))
    }
    puzzleverse_core::asset::Transformation::CV => {
      Transform::from_rotation(bevy::math::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)).with_scale(bevy::math::vec3(1.0, -1.0, 1.0))
    }
    puzzleverse_core::asset::Transformation::VH => Transform::from_scale(bevy::math::vec3(-1.0, -1.0, 1.0)),
  }
}

pub(crate) fn add_mesh<A: ExtractArgument>(
  commands: &mut bevy::ecs::system::Commands,
  state: &mut WorldBuildingState,
  interaction_key: Option<puzzleverse_core::InteractionKey>,
  property_key: Option<puzzleverse_core::PropertyKey>,
  model: puzzleverse_core::asset::SimpleSprayModel,
  arguments: Vec<A>,
  platform_id: u32,
  x: u32,
  y: u32,
  z: u32,
  transformation: puzzleverse_core::asset::Transformation,
) {
  use bevy::prelude::BuildChildren;
  use bevy::transform::components::Transform;
  let mut meshes_iter = model.meshes.into_iter();
  if let Some(mut root_mesh) = meshes_iter.next() {
    let (root_mesh, material) = convert_mesh(&mut state.meshes, root_mesh);
    let mut updates = Vec::new();
    let spawn = commands.spawn();
    let root_mesh = bevy::pbr::PbrBundle {
      mesh: root_mesh,
      material: arguments
        .get(material as usize)
        .map(|a| a.extract_material(x, y, z, spawn.id(), &mut updates, state))
        .flatten()
        .unwrap_or(state.default_material.clone()),
      global_transform: (Transform::from_xyz(x as f32, y as f32, z as f32) * transform(transformation)).into(),
      ..Default::default()
    };
    spawn.insert_bundle(root_mesh).insert_bundle(bevy_mod_picking::PickableBundle::default()).insert(bevy_mod_picking::NoDeselect);
    if let Some(interaction_key) = interaction_key {
      spawn.insert(crate::InteractionTarget { click: true, key: interaction_key, platform: platform_id, x, y });
    }
    spawn.with_children(|builder| {
      for mesh in meshes_iter {
        let (mesh, material) = convert_mesh(&mut state.meshes, mesh);
        let spawn = builder.spawn();
        spawn.insert_bundle(bevy::pbr::PbrBundle {
          mesh,
          material: arguments
            .get(material as usize)
            .map(|a| a.extract_material(x, y, z, spawn.id(), &mut updates, state))
            .flatten()
            .unwrap_or(state.default_material.clone()),
          ..Default::default()
        });
      }
      for light in model.lights {
        match light {
          puzzleverse_core::asset::Light::Point { position, color, intensity } => {
            let spawn = builder.spawn();
            let color = arguments
              .get(color as usize)
              .map(|arg| arg.extract_color(x, y, z, spawn.id(), &mut updates, state))
              .flatten()
              .unwrap_or(bevy::render::color::Color::WHITE);
            let intensity = arguments
              .get(intensity as usize)
              .map(|arg| arg.extract_light_intensity(x, y, z, spawn.id(), &mut updates, state))
              .flatten()
              .unwrap_or(1.0)
              * MAX_ILLUMINATION;
            spawn.insert_bundle(bevy::pbr::PointLightBundle {
              point_light: bevy::pbr::PointLight {
                color,
                intensity,
                range: 20.0,
                radius: 0.0,
                shadows_enabled: false,
                shadow_depth_bias: bevy::pbr::PointLight::DEFAULT_SHADOW_DEPTH_BIAS,
                shadow_normal_bias: bevy::pbr::PointLight::DEFAULT_SHADOW_NORMAL_BIAS,
              },
              computed_visibility: Default::default(),
              cubemap_visible_entities: Default::default(),
              cubemap_frusta: Default::default(),
              transform: Transform::from_translation(bevy::math::Vec3::new(position.0, position.1, position.2)),
              global_transform: Default::default(),
              visibility: Default::default(),
            });
          }
        }
      }
    });
    if let Some(property_key) = property_key {
      state.updates.entry(property_key).or_default().extend(updates);
    }
    state.occupied.insert((x, y));
  }
}
pub fn convert_color(value: puzzleverse_core::asset::Color) -> bevy::render::color::Color {
  match value {
    puzzleverse_core::asset::Color::Rgb(r, g, b) => {
      bevy::render::color::Color::Rgba { red: r as f32 / 255.0, green: (g as f32 / 255.0), blue: (b as f32 / 255.0), alpha: 1.0 }
    }
    puzzleverse_core::asset::Color::Hsl(h, s, l) => {
      bevy::render::color::Color::Hsla { hue: h as f32 / 255.0, saturation: s as f32 / 255.0, lightness: l as f32 / 255.0, alpha: 1.0 }
    }
  }
}
pub fn convert_global<T: IntoBevy>(
  value: puzzleverse_core::asset::GlobalValue<T>,
  source: impl UpdateSource<T>,
  state: &mut WorldBuildingState,
) -> T::Bevy {
  match value {
    puzzleverse_core::asset::GlobalValue::Fixed(value) => T::convert(value),
    puzzleverse_core::asset::GlobalValue::PuzzleBool { id, when_true, when_false, transition } => {
      let (handle, update) = source.as_bool(when_true, when_false, transition, state);
      state.updates.entry(puzzleverse_core::PropertyKey::BoolSink(id)).or_default().push(update);
      handle
    }
    puzzleverse_core::asset::GlobalValue::PuzzleNum { id, default, values, transition } => {
      let (handle, update) = source.as_num(default, values, transition, state);
      state.updates.entry(puzzleverse_core::PropertyKey::NumSink(id)).or_default().push(update);
      handle
    }
    puzzleverse_core::asset::GlobalValue::Random(values) => T::convert(values[state.seed.abs() as usize % values.len()]),
    puzzleverse_core::asset::GlobalValue::Setting(setting) => {
      if let Some((handle, update)) = source.as_setting(state) {
        state.settings.entry(setting).or_default().push(update);
        handle
      } else {
        T::empty()
      }
    }
    puzzleverse_core::asset::GlobalValue::SettingBool { id, when_true, when_false, transition } => {
      let (handle, update) = source.as_setting_bool(when_true, when_false, transition, state);
      if let Some(update) = update {
        state.settings.entry(id).or_default().push(update);
      }
      handle
    }
    puzzleverse_core::asset::GlobalValue::SettingNum { id, default, values, transition } => {
      let (handle, update) = source.as_setting_num(default, values, transition, state);
      if let Some(update) = update {
        state.settings.entry(id).or_default().push(update);
      }
      handle
    }
    puzzleverse_core::asset::GlobalValue::Masked(name) => match state.masks.get(&name) {
      Some(mask) => match T::mask(mask, source, state) {
        Some((handle, update)) => {
          state
            .updates
            .entry(match mask {
              puzzleverse_core::asset::MaskConfiguration::Bool { .. } => puzzleverse_core::PropertyKey::BoolSink(name),
              puzzleverse_core::asset::MaskConfiguration::Num { .. } => puzzleverse_core::PropertyKey::NumSink(name),
            })
            .or_default()
            .push(update);
          handle
        }
        None => T::empty(),
      },
      None => T::empty(),
    },
  }
}
fn convert_local<T: IntoBevy>(
  value: puzzleverse_core::asset::LocalValue<T>,
  x: u32,
  y: u32,
  z: u32,
  source: impl UpdateSource<T>,
  state: &mut WorldBuildingState,
) -> T::Bevy {
  match value {
    puzzleverse_core::asset::LocalValue::Altitude { top_value, bottom_value, bottom_limit, top_limit } => {
      if z >= top_limit {
        top_value.convert()
      } else if z <= bottom_limit || bottom_limit <= top_limit {
        bottom_value.convert()
      } else {
        let fraction = (top_limit - z) as f64 / (top_limit - bottom_limit) as f64;
        match T::mix(vec![(fraction, top_value.clone()), (1.0 - fraction, bottom_value.clone())]) {
          Some(value) => T::convert(value),
          None => T::empty(),
        }
      }
    }
    puzzleverse_core::asset::LocalValue::Global(value) => convert_global(value, source, state),
    puzzleverse_core::asset::LocalValue::Gradiator(name) => {
      match (T::gradiator(state).map(|g| g.get_mut(&name)).flatten(), source.as_gradiator(state)) {
        (Some(gradiator), Some((handle, update))) => {
          gradiator.register(x, y, z, update);
          handle
        }
        _ => T::empty(),
      }
    }
    puzzleverse_core::asset::LocalValue::RandomLocal(values) => {
      T::convert(values.remove((x as usize).wrapping_mul(y as usize).wrapping_mul(z as usize) % values.len()))
    }
  }
}
fn convert_local_delayed<T: IntoLocalBevy>(
  value: puzzleverse_core::asset::LocalValue<T>,
  seed: i32,
  state: &mut WorldBuildingState,
) -> LocalBevyValue<T> {
  match value {
    puzzleverse_core::asset::LocalValue::Altitude { top_value, bottom_value, bottom_limit, top_limit } => {
      match T::altitude(bottom_limit, bottom_value, top_limit, top_value, state) {
        Some(id) => LocalBevyValue::Positional(id),
        None => LocalBevyValue::Fixed(T::empty(state)),
      }
    }
    puzzleverse_core::asset::LocalValue::Global(value) => match value {
      puzzleverse_core::asset::GlobalValue::Fixed(value) => LocalBevyValue::Fixed(value.fixed(state)),
      puzzleverse_core::asset::GlobalValue::PuzzleBool { id, when_true, when_false, transition } => {
        LocalBevyValue::Positional(T::prepare_bool(id, when_true, when_false, transition, state))
      }
      puzzleverse_core::asset::GlobalValue::PuzzleNum { id, default, values, transition } => {
        LocalBevyValue::Positional(T::prepare_num(id, default, values, transition, state))
      }
      puzzleverse_core::asset::GlobalValue::Masked(mask) => match T::prepare_mask(mask, state) {
        Some(id) => LocalBevyValue::Positional(id),
        None => LocalBevyValue::Fixed(T::empty(state)),
      },
      puzzleverse_core::asset::GlobalValue::Random(values) => LocalBevyValue::Fixed(values[state.seed.abs() as usize % values.len()].fixed(state)),
      puzzleverse_core::asset::GlobalValue::Setting(id) => match T::prepare_setting(id, state) {
        Some(id) => LocalBevyValue::Positional(id),
        None => LocalBevyValue::Fixed(T::empty(state)),
      },

      puzzleverse_core::asset::GlobalValue::SettingBool { id, when_true, when_false, transition } => {
        LocalBevyValue::Positional(T::prepare_setting_bool(id, when_true, when_false, transition, state))
      }
      puzzleverse_core::asset::GlobalValue::SettingNum { id, default, values, transition } => {
        LocalBevyValue::Positional(T::prepare_setting_num(id, default, values, transition, state))
      }
    },
    puzzleverse_core::asset::LocalValue::Gradiator(name) => match T::gradiator(state, name) {
      Some(id) => LocalBevyValue::Positional(id),
      _ => LocalBevyValue::Fixed(T::empty(state)),
    },
    puzzleverse_core::asset::LocalValue::RandomLocal(values) => {
      LocalBevyValue::RandomLocal(values.into_iter().map(|value| value.fixed(state)).collect())
    }
  }
}

impl ExtractArgument for puzzleverse_core::asset::Argument {
  type LocalName = ();
  fn extract_color(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::render::color::Color> {
    match self {
      &puzzleverse_core::asset::Argument::Color(color) => Some(convert_local(color, x, y, z, LightEntity(entity), state)),
      _ => None,
    }
  }

  fn extract_light_intensity(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<f32> {
    match self {
      &puzzleverse_core::asset::Argument::Intensity(intensity) => Some(convert_local(intensity, x, y, z, LightEntity(entity), state) as f32),
      _ => None,
    }
  }
  fn extract_material(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::asset::Handle<bevy::pbr::StandardMaterial>> {
    match self {
      &puzzleverse_core::asset::Argument::Material(id) => state.materials.get(id as usize).map(|m| m.at(x, y, z, entity, state)),
      _ => None,
    }
  }

  fn extract_color_local(&self, name: &(), seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<puzzleverse_core::asset::Color>> {
    match self {
      &puzzleverse_core::asset::Argument::Color(value) => Some(convert_local_delayed(value, seed, state)),
      _ => None,
    }
  }

  fn extract_light_intensity_local(&self, name: &(), seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<f64>> {
    match self {
      &puzzleverse_core::asset::Argument::Intensity(intensity) => Some(convert_local_delayed(intensity, seed, state)),
      _ => None,
    }
  }

  fn extract_material_local(&self, name: &(), seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<u32>> {
    match self {
      &puzzleverse_core::asset::Argument::Material(id) => state.materials.get(id as usize).cloned(),
      _ => None,
    }
  }
}

impl ExtractArgument for puzzleverse_core::asset::CycleArgument {
  type LocalName = str;
  fn extract_material(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::asset::Handle<bevy::pbr::StandardMaterial>> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Material(id) => state.materials.get(id as usize).map(|m| m.at(x, y, z, entity, state)),
      &puzzleverse_core::asset::CycleArgument::CycleMaterial(default, values, transition) => {
        let default = state.materials.get(default as usize).map(|m| m.at(x, y, z, entity, state)).unwrap_or(state.default_material.clone());
        let values: Vec<_> = values
          .into_iter()
          .map(|material| state.materials.get(material as usize).map(|m| m.at(x, y, z, entity, state)).unwrap_or(state.default_material.clone()))
          .collect();
        let current = values.get(0).cloned();
        updates.push(crate::update_handler::Update::NumChangeMeshMaterial(entity, default, values, transition));
        current
      }
      _ => None,
    }
  }

  fn extract_color(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::render::color::Color> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Color(c) => Some(convert_local(c, x, y, z, LightEntity(entity), state)),
      &puzzleverse_core::asset::CycleArgument::CycleColor(default, values, transition) => {
        let default = convert_color(default);
        let values: Vec<_> = values.into_iter().map(|color| convert_color(color)).collect();
        updates.push(crate::update_handler::Update::NumChangeLightColor(entity, default.clone(), values, transition));
        Some(default)
      }
      _ => None,
    }
  }

  fn extract_light_intensity(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<f32> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Intensity(i) => Some(convert_local(i, x, y, z, LightEntity(entity), state) as f32),
      &puzzleverse_core::asset::CycleArgument::CycleIntensity(default, values, transition) => {
        let current = values.get(0).map(|&v| v as f32 * MAX_ILLUMINATION);
        updates.push(crate::update_handler::Update::NumChangeLightIntensity(entity, default, values, transition));
        current
      }
      _ => None,
    }
  }

  fn extract_color_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<puzzleverse_core::asset::Color>> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Color(color) => Some(convert_local_delayed(color, seed, state)),
      &puzzleverse_core::asset::CycleArgument::CycleColor(default, values, transition) => {
        let id = state.locals_color.len();
        state.locals_color.push(Locals::PuzzleNum(name.to_owned(), default, values, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }

  fn extract_light_intensity_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<f64>> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Intensity(intensity) => Some(convert_local_delayed(intensity, seed, state)),
      &puzzleverse_core::asset::CycleArgument::CycleIntensity(default, values, transition) => {
        let id = state.locals_intensity.len();
        state.locals_intensity.push(Locals::PuzzleNum(name.to_owned(), default, values, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }

  fn extract_material_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<u32>> {
    match self {
      &puzzleverse_core::asset::CycleArgument::Material(id) => state.materials.get(id as usize).cloned(),
      &puzzleverse_core::asset::CycleArgument::CycleMaterial(default, values, transition) => {
        let id = state.material_builders.len();
        state.material_builders.push(crate::materials::MaterialBuilder::PuzzleNum(name.to_owned(), default, values, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }
}
impl ExtractArgument for puzzleverse_core::asset::SwitchArgument {
  type LocalName = str;
  fn extract_material(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::asset::Handle<bevy::pbr::StandardMaterial>> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Material(id) => state.materials.get(id as usize).map(|m| m.at(x, y, z, entity, state)),
      &puzzleverse_core::asset::SwitchArgument::SwitchMaterial(on, off, transition) => {
        let on = state.materials.get(on as usize).map(|m| m.at(x, y, z, entity, state))?;
        let off = state.materials.get(off as usize).map(|m| m.at(x, y, z, entity, state))?;
        updates.push(crate::update_handler::Update::BoolChangeMeshMaterial(entity, on, off, transition));
        Some(on.clone())
      }
      _ => None,
    }
  }

  fn extract_color(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<bevy::render::color::Color> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Color(color) => Some(convert_local(color, x, y, z, LightEntity(entity), state)),
      &puzzleverse_core::asset::SwitchArgument::SwitchColor(on, off, transition) => {
        let on = puzzleverse_core::asset::Color::convert(on);
        let off = puzzleverse_core::asset::Color::convert(off);
        updates.push(crate::update_handler::Update::BoolChangeLightColor(entity, on, off.clone(), transition));
        Some(off)
      }
      _ => None,
    }
  }

  fn extract_light_intensity(
    &self,
    x: u32,
    y: u32,
    z: u32,
    entity: bevy::ecs::entity::Entity,
    updates: &mut Vec<crate::update_handler::Update>,
    state: &mut WorldBuildingState,
  ) -> Option<f32> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Intensity(color) => Some(convert_local(color, x, y, z, LightEntity(entity), state) as f32),
      &puzzleverse_core::asset::SwitchArgument::SwitchIntensity(on, off, transition) => {
        updates.push(crate::update_handler::Update::BoolChangeLightIntensity(entity, on, off, transition));
        Some((on as f32) * MAX_ILLUMINATION)
      }
      _ => None,
    }
  }

  fn extract_color_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<puzzleverse_core::asset::Color>> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Color(color) => Some(convert_local_delayed(color, seed, state)),
      &puzzleverse_core::asset::SwitchArgument::SwitchColor(on, off, transition) => {
        let id = state.locals_color.len();
        state.locals_color.push(Locals::PuzzleBool(name.to_owned(), on, off, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }

  fn extract_light_intensity_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<f64>> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Intensity(intensity) => Some(convert_local_delayed(intensity, seed, state)),
      &puzzleverse_core::asset::SwitchArgument::SwitchIntensity(on, off, transition) => {
        let id = state.locals_intensity.len();
        state.locals_intensity.push(Locals::PuzzleBool(name.to_owned(), on, off, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }

  fn extract_material_local(&self, name: &str, seed: i32, state: &mut WorldBuildingState) -> Option<LocalBevyValue<u32>> {
    match self {
      &puzzleverse_core::asset::SwitchArgument::Material(id) => state.materials.get(id as usize).cloned(),
      &puzzleverse_core::asset::SwitchArgument::SwitchMaterial(on, off, transition) => {
        let id = state.material_builders.len();
        state.material_builders.push(crate::materials::MaterialBuilder::PuzzleBool(name.to_owned(), on, off, transition));
        Some(LocalBevyValue::Positional(id))
      }
      _ => None,
    }
  }
}
impl UpdateSource<f64> for LightEntity {
  fn as_bool(
    self,
    when_true: f64,
    when_false: f64,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, crate::update_handler::Update) {
    (when_true, crate::update_handler::Update::BoolChangeLightIntensity(self.0, when_true, when_false, transition))
  }

  fn as_num(
    self,
    default: f64,
    states: Vec<f64>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, crate::update_handler::Update) {
    (default, crate::update_handler::Update::NumChangeLightIntensity(self.0, default, states, transition))
  }

  fn as_gradiator(self, store: &mut WorldBuildingState) -> Option<(<f64 as IntoBevy>::Bevy, <f64 as IntoBevy>::GradiatorUpdate)> {
    Some((0.0, crate::gradiator::FloatUpdater::ChangeLightIntensity(self.0)))
  }

  fn as_setting(self, store: &mut WorldBuildingState) -> Option<(<f64 as IntoBevy>::Bevy, crate::update_handler::SettingUpdate)> {
    Some((0.0, crate::update_handler::SettingUpdate::ChangeLightIntensity(self.0)))
  }

  fn as_setting_bool(
    self,
    when_true: f64,
    when_false: f64,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (when_false, Some(crate::update_handler::SettingUpdate::BoolChangeLightIntensity(self.0, when_true, when_false)))
  }

  fn as_setting_num(
    self,
    default: f64,
    values: Vec<f64>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (default, Some(crate::update_handler::SettingUpdate::NumChangeLightIntensity(self.0, default, values)))
  }
}
impl UpdateSource<puzzleverse_core::asset::Color> for LightEntity {
  fn as_bool(
    self,
    when_true: puzzleverse_core::asset::Color,
    when_false: puzzleverse_core::asset::Color,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::Update) {
    let when_true = convert_color(when_true);
    let when_false = convert_color(when_false);
    (when_false.clone(), crate::update_handler::Update::BoolChangeLightColor(self.0, when_true, when_false, transition))
  }

  fn as_num(
    self,
    default: puzzleverse_core::asset::Color,
    states: Vec<puzzleverse_core::asset::Color>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::Update) {
    let default = convert_color(default);
    let states: Vec<_> = states.into_iter().map(|c| convert_color(c)).collect();
    (default.clone(), crate::update_handler::Update::NumChangeLightColor(self.0, default, states, transition))
  }

  fn as_gradiator(
    self,
    store: &mut WorldBuildingState,
  ) -> Option<(<puzzleverse_core::asset::Color as IntoBevy>::Bevy, <puzzleverse_core::asset::Color as IntoBevy>::GradiatorUpdate)> {
    Some((bevy::render::color::Color::BLACK, crate::gradiator::ColorUpdater::ChangeLightColor(self.0)))
  }

  fn as_setting(
    self,
    store: &mut WorldBuildingState,
  ) -> Option<(<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::SettingUpdate)> {
    Some((bevy::render::color::Color::BLACK, crate::update_handler::SettingUpdate::ChangeLightColor(self.0)))
  }

  fn as_setting_bool(
    self,
    when_true: puzzleverse_core::asset::Color,
    when_false: puzzleverse_core::asset::Color,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (
      bevy::render::color::Color::BLACK,
      Some(crate::update_handler::SettingUpdate::BoolChangeLightColor(self.0, convert_color(when_true), convert_color(when_false))),
    )
  }

  fn as_setting_num(
    self,
    default: puzzleverse_core::asset::Color,
    values: Vec<puzzleverse_core::asset::Color>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    let default = convert_color(default);
    (
      default.clone(),
      Some(crate::update_handler::SettingUpdate::NumChangeLightColor(self.0, default, values.into_iter().map(|c| convert_color(c)).collect())),
    )
  }
}
impl UpdateSource<f64> for AmbientLight {
  fn as_bool(
    self,
    when_true: f64,
    when_false: f64,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, crate::update_handler::Update) {
    (when_true, crate::update_handler::Update::BoolAmbientLightIntensity(when_true, when_false, transition))
  }

  fn as_num(
    self,
    default: f64,
    states: Vec<f64>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, crate::update_handler::Update) {
    (default, crate::update_handler::Update::NumAmbientLightIntensity(default, states, transition))
  }

  fn as_gradiator(self, store: &mut WorldBuildingState) -> Option<(<f64 as IntoBevy>::Bevy, <f64 as IntoBevy>::GradiatorUpdate)> {
    None
  }

  fn as_setting(self, store: &mut WorldBuildingState) -> Option<(<f64 as IntoBevy>::Bevy, crate::update_handler::SettingUpdate)> {
    Some((0.0, crate::update_handler::SettingUpdate::AmbientLightIntensity))
  }

  fn as_setting_bool(
    self,
    when_true: f64,
    when_false: f64,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (when_false, Some(crate::update_handler::SettingUpdate::BoolAmbientLightIntensity(when_true, when_false)))
  }

  fn as_setting_num(
    self,
    default: f64,
    values: Vec<f64>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<f64 as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (default, Some(crate::update_handler::SettingUpdate::NumAmbientLightIntensity(default, values)))
  }
}
impl UpdateSource<puzzleverse_core::asset::Color> for AmbientLight {
  fn as_bool(
    self,
    when_true: puzzleverse_core::asset::Color,
    when_false: puzzleverse_core::asset::Color,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::Update) {
    let when_true = convert_color(when_true);
    let when_false = convert_color(when_false);
    (when_false.clone(), crate::update_handler::Update::BoolAmbientLightColor(when_true, when_false, transition))
  }

  fn as_num(
    self,
    default: puzzleverse_core::asset::Color,
    states: Vec<puzzleverse_core::asset::Color>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::Update) {
    let default = convert_color(default);
    let states: Vec<_> = states.into_iter().map(|c| convert_color(c)).collect();
    (default.clone(), crate::update_handler::Update::NumAmbientLightColor(default, states, transition))
  }

  fn as_gradiator(
    self,
    store: &mut WorldBuildingState,
  ) -> Option<(<puzzleverse_core::asset::Color as IntoBevy>::Bevy, <puzzleverse_core::asset::Color as IntoBevy>::GradiatorUpdate)> {
    None
  }

  fn as_setting(
    self,
    store: &mut WorldBuildingState,
  ) -> Option<(<puzzleverse_core::asset::Color as IntoBevy>::Bevy, crate::update_handler::SettingUpdate)> {
    Some((bevy::render::color::Color::BLACK, crate::update_handler::SettingUpdate::AmbientLightColor))
  }

  fn as_setting_bool(
    self,
    when_true: puzzleverse_core::asset::Color,
    when_false: puzzleverse_core::asset::Color,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (
      bevy::render::color::Color::BLACK,
      Some(crate::update_handler::SettingUpdate::BoolAmbientLightColor(convert_color(when_true), convert_color(when_false))),
    )
  }

  fn as_setting_num(
    self,
    default: puzzleverse_core::asset::Color,
    values: Vec<puzzleverse_core::asset::Color>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<puzzleverse_core::asset::Color as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    let default = convert_color(default);
    (
      default.clone(),
      Some(crate::update_handler::SettingUpdate::NumAmbientLightColor(default, values.into_iter().map(|c| convert_color(c)).collect())),
    )
  }
}

impl IntoBevy for puzzleverse_core::asset::Color {
  type Bevy = bevy::render::color::Color;

  type GradiatorUpdate = crate::gradiator::ColorUpdater;

  fn convert(self) -> Self::Bevy {
    convert_color(self)
  }

  fn empty() -> Self::Bevy {
    bevy::render::color::Color::BLACK
  }

  fn gradiator<'a>(state: &'a mut WorldBuildingState) -> Option<&'a mut std::collections::BTreeMap<String, crate::gradiator::Gradiator<Self>>> {
    Some(state.gradiators_color)
  }

  fn mix<'a>(values: impl IntoIterator<Item = (f64, Self)>) -> Option<Self> {
    // Blending from http://jcgt.org/published/0002/02/09/ and clarified in https://computergraphics.stackexchange.com/questions/4651/additive-blending-with-weighted-blended-order-independent-transparency/5937
    let mut r_value = 0.0;
    let mut g_value = 0.0;
    let mut b_value = 0.0;
    let mut a_value = 0.0;
    for (weight, item) in values {
      let weight = weight as f32;
      let (r, g, b) = match item {
        puzzleverse_core::asset::Color::Rgb(r, g, b) => (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        puzzleverse_core::asset::Color::Hsl(h, s, l) => {
          let [r, g, b, a] = bevy::render::color::Color::hsl(h as f32 / 255.0, s as f32 / 255.0, l as f32 / 255.0).as_rgba_f32();
          (r, g, b)
        }
      };
      r_value += weight * r;
      g_value += weight * g;
      b_value += weight * b;
      a_value += weight;
    }
    a_value = a_value.max(1e-5);
    Some(puzzleverse_core::asset::Color::Rgb((r_value / a_value * 255.0) as u8, (g_value / a_value * 255.0) as u8, (b_value / a_value * 255.0) as u8))
  }

  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: impl UpdateSource<Self>,
    state: &mut WorldBuildingState,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)> {
    match mask {
      puzzleverse_core::asset::MaskConfiguration::Bool { color, .. } => match color {
        Some((when_true, when_false, transition)) => Some(source.as_bool(*when_true, *when_false, *transition, state)),
        None => None,
      },
      puzzleverse_core::asset::MaskConfiguration::Num { color, .. } => match color.clone() {
        Some((default, values, transition)) => Some(source.as_num(default, values, transition, state)),
        None => None,
      },
    }
  }

  fn locals<'a>(state: &'a mut WorldBuildingState) -> &'a mut Vec<Locals<Self>> {
    &mut state.locals_color
  }
}
impl IntoBevy for f64 {
  type Bevy = f64;

  type GradiatorUpdate = crate::gradiator::FloatUpdater;

  fn convert(self) -> Self::Bevy {
    self
  }

  fn empty() -> Self::Bevy {
    0.0
  }

  fn gradiator<'a>(state: &'a mut WorldBuildingState) -> Option<&'a mut std::collections::BTreeMap<String, crate::gradiator::Gradiator<Self>>> {
    Some(state.gradiators_intensity)
  }

  fn mix<'a>(values: impl IntoIterator<Item = (f64, Self)>) -> Option<Self> {
    let mut value = 0.0;
    let mut total = 0.0;
    for (weight, item) in values {
      value += weight * item;
      total += weight;
    }
    Some(if total > 0.0 { (value / total).max(0.0).min(1.0) } else { 0.0 })
  }

  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: impl UpdateSource<Self>,
    state: &mut WorldBuildingState,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)> {
    match mask {
      puzzleverse_core::asset::MaskConfiguration::Bool { intensity, .. } => match intensity {
        Some((when_true, when_false, transition)) => Some(source.as_bool(*when_true, *when_false, *transition, state)),
        None => None,
      },
      puzzleverse_core::asset::MaskConfiguration::Num { intensity, .. } => match intensity.clone() {
        Some((default, values, transition)) => Some(source.as_num(default, values, transition, state)),
        None => None,
      },
    }
  }

  fn locals<'a>(state: &'a mut WorldBuildingState) -> &'a mut Vec<Locals<Self>> {
    &mut state.locals_intensity
  }
}

impl<T: IntoBevy> IntoLocalBevy for T {
  type Bevy = T::Bevy;
  type UpdateSource = Box<dyn UpdateSource<T>>;

  fn altitude(bottom_limit: u32, bottom_value: Self, top_limit: u32, top_value: Self, state: &mut WorldBuildingState) -> Option<usize> {
    let id = state.locals_intensity.len();
    T::locals(state).push(Locals::AltitudeMixer(crate::altitude_mixer::AltitudeMixer::new(bottom_limit, bottom_value, top_limit, top_value)));
    Some(id)
  }

  fn empty(state: &mut WorldBuildingState) -> Self::Bevy {
    T::empty()
  }

  fn fixed(self, state: &mut WorldBuildingState) -> Self::Bevy {
    self.convert()
  }

  fn gradiator(state: &mut WorldBuildingState, name: String) -> Option<usize> {
    if T::gradiator(state).map(|g| g.contains_key(&name)).unwrap_or(false) {
      let locals = T::locals(state);
      let id = locals.len();
      locals.push(Locals::Gradiator(name));
      Some(id)
    } else {
      None
    }
  }

  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: Self::UpdateSource,
    state: &mut WorldBuildingState,
    x: u32,
    y: u32,
    z: u32,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)> {
    T::mask(mask, source, state)
  }

  fn prepare_bool(
    name: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::PuzzleBool(name, when_true, when_false, transition));
    id
  }

  fn prepare_mask(mask: String, state: &mut WorldBuildingState) -> Option<usize> {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::Masked(mask));
    Some(id)
  }
  fn prepare_num(
    name: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::PuzzleNum(name, default, values, transition));
    id
  }

  fn register(id: usize, source: Self::UpdateSource, state: &mut WorldBuildingState, x: u32, y: u32, z: u32) -> Self::Bevy {
    match T::locals(state)[id] {
      Locals::Masked(id) => match state.masks.get(&id) {
        Some(mask) => match T::mask(mask, source, state) {
          Some((value, update)) => {
            state
              .updates
              .entry(match mask {
                puzzleverse_core::asset::MaskConfiguration::Bool { .. } => puzzleverse_core::PropertyKey::BoolSink(id),
                puzzleverse_core::asset::MaskConfiguration::Num { .. } => puzzleverse_core::PropertyKey::NumSink(id),
              })
              .or_default()
              .push(update);
            value
          }
          None => T::empty(),
        },
        None => T::empty(),
      },
      Locals::AltitudeMixer(altitude_mixer) => altitude_mixer.register(z),
      Locals::Gradiator(name) => match (T::gradiator(state).map(|g| g.get_mut(&name)).flatten(), source.as_gradiator(state)) {
        (Some(gradiator), Some((handle, update))) => {
          gradiator.register(x, y, z, update);
          handle
        }
        _ => T::empty(),
      },
      Locals::PuzzleBool(name, when_true, when_false, transition) => {
        let (value, update) = source.as_bool(when_true, when_false, transition, state);
        state.updates.entry(puzzleverse_core::PropertyKey::BoolSink(name)).or_default().push(update);
        value
      }
      Locals::PuzzleNum(name, default, values, transition) => {
        let (value, update) = source.as_num(default, values, transition, state);
        state.updates.entry(puzzleverse_core::PropertyKey::NumSink(name)).or_default().push(update);
        value
      }
      Locals::Setting(name) => match source.as_setting(state) {
        Some((value, update)) => {
          state.settings.entry(name).or_default().push(update);
          value
        }
        None => T::empty(),
      },
      Locals::SettingBool(name, when_true, when_false, transition) => {
        let (value, update) = source.as_setting_bool(when_true, when_false, transition, state);
        if let Some(update) = update {
          state.settings.entry(name).or_default().push(update);
        }
        value
      }
      Locals::SettingNum(name, default, values, transition) => {
        let (value, update) = source.as_setting_num(default, values, transition, state);
        if let Some(update) = update {
          state.settings.entry(name).or_default().push(update);
        }
        value
      }
    }
  }

  fn prepare_setting(name: String, state: &mut WorldBuildingState) -> Option<usize> {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::Setting(name));
    Some(id)
  }

  fn prepare_setting_bool(
    name: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::SettingBool(name, when_true, when_false, transition));
    id
  }

  fn prepare_setting_num(
    name: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let locals = T::locals(state);
    let id = locals.len();
    locals.push(Locals::SettingNum(name, default, values, transition));
    id
  }
}
impl IntoLocalBevy for u32 {
  type Bevy = bevy::asset::Handle<bevy::pbr::StandardMaterial>;
  type UpdateSource = bevy::ecs::entity::Entity;

  fn empty(state: &mut WorldBuildingState) -> Self::Bevy {
    state.default_material.clone()
  }

  fn mask(
    mask: &puzzleverse_core::asset::MaskConfiguration,
    source: Self::UpdateSource,
    state: &mut WorldBuildingState,
    x: u32,
    y: u32,
    z: u32,
  ) -> Option<(Self::Bevy, crate::update_handler::Update)> {
    match mask {
      puzzleverse_core::asset::MaskConfiguration::Bool { material, .. } => match material {
        Some((when_true, when_false, transition)) => Some((
          state.default_material.clone(),
          crate::update_handler::Update::BoolChangeMeshMaterial(
            source,
            state.materials.get(*when_true as usize).map(|m| m.at(x, y, z, source, state)).unwrap_or(state.default_material.clone()),
            state.materials.get(*when_false as usize).map(|m| m.at(x, y, z, source, state)).unwrap_or(state.default_material.clone()),
            *transition,
          ),
        )),
        None => None,
      },
      puzzleverse_core::asset::MaskConfiguration::Num { material, .. } => match material {
        Some((default, values, transition)) => Some((
          state.default_material.clone(),
          crate::update_handler::Update::NumChangeMeshMaterial(
            source,
            state.materials.get(*default as usize).map(|m| m.at(x, y, z, source, state)).unwrap_or(state.default_material.clone()),
            values
              .iter()
              .map(|v| state.materials.get(*v as usize).map(|m| m.at(x, y, z, source, state)).unwrap_or(state.default_material.clone()))
              .collect(),
            *transition,
          ),
        )),
        None => None,
      },
    }
  }
  fn register(id: usize, entity: bevy::ecs::entity::Entity, state: &mut WorldBuildingState, x: u32, y: u32, z: u32) -> <Self as IntoLocalBevy>::Bevy {
    {
      state.material_builders.get_mut(id).map(|builder| builder.define(x, y, z)).unwrap_or(state.default_material.clone())
    }
  }

  fn altitude(bottom_limit: u32, bottom_value: Self, top_limit: u32, top_value: Self, state: &mut WorldBuildingState) -> Option<usize> {
    None
  }

  fn fixed(self, state: &mut WorldBuildingState) -> Self::Bevy {
    state.default_material.clone()
  }

  fn gradiator(state: &mut WorldBuildingState, name: String) -> Option<usize> {
    None
  }

  fn prepare_bool(
    name: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let id = state.material_builders.len();
    state.material_builders.push(crate::materials::MaterialBuilder::PuzzleBool(name, when_true, when_false, transition));
    id
  }

  fn prepare_mask(mask: String, state: &mut WorldBuildingState) -> Option<usize> {
    match state.masks.get(&mask)? {
      puzzleverse_core::asset::MaskConfiguration::Bool { material: Some((when_true, when_false, transition)), .. } => {
        let id = state.material_builders.len();
        state.material_builders.push(crate::materials::MaterialBuilder::PuzzleBool(mask, *when_true, *when_false, *transition));
        Some(id)
      }
      puzzleverse_core::asset::MaskConfiguration::Num { material: Some((default, values, transition)), .. } => {
        let id = state.material_builders.len();
        state.material_builders.push(crate::materials::MaterialBuilder::PuzzleNum(mask, *default, values.clone(), *transition));
        Some(id)
      }
      _ => None,
    }
  }

  fn prepare_num(
    name: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let id = state.material_builders.len();
    state.material_builders.push(crate::materials::MaterialBuilder::PuzzleNum(name, default, values, transition));
    id
  }

  fn prepare_setting(id: String, state: &mut WorldBuildingState) -> Option<usize> {
    None
  }

  fn prepare_setting_bool(
    name: String,
    when_true: Self,
    when_false: Self,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let id = state.material_builders.len();
    state.material_builders.push(crate::materials::MaterialBuilder::SettingBool(name, when_true, when_false, transition));
    id
  }

  fn prepare_setting_num(
    name: String,
    default: Self,
    values: Vec<Self>,
    transition: puzzleverse_core::asset::Transition,
    state: &mut WorldBuildingState,
  ) -> usize {
    let id = state.material_builders.len();
    state.material_builders.push(crate::materials::MaterialBuilder::SettingNum(name, default, values, transition));
    id
  }
}
impl<T: IntoLocalBevy> LocalBevyValue<T> {
  pub fn at(&self, x: u32, y: u32, z: u32, source: T::UpdateSource, state: &mut WorldBuildingState) -> T::Bevy {
    match self {
      LocalBevyValue::Positional(id) => T::register(*id, source, state, x, y, z),
      LocalBevyValue::Fixed(v) => v.clone(),
      LocalBevyValue::RandomLocal(values) => {
        use std::hash::Hasher;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        hash.write_u32(x);
        hash.write_u32(y);
        hash.write_u32(z);
        values[(hash.finish() % values.len() as u64) as usize].clone()
      }
    }
  }
}
impl<T: IntoBevy> UpdateSource<T> for Box<dyn UpdateSource<T>> {
  fn as_bool(
    self,
    when_true: T,
    when_false: T,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<T as IntoBevy>::Bevy, crate::update_handler::Update) {
    (*self).as_bool(when_true, when_false, transition, store)
  }

  fn as_num(
    self,
    default: T,
    states: Vec<T>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<T as IntoBevy>::Bevy, crate::update_handler::Update) {
    (*self).as_num(default, states, transition, store)
  }

  fn as_gradiator(self, store: &mut WorldBuildingState) -> Option<(<T as IntoBevy>::Bevy, <T as IntoBevy>::GradiatorUpdate)> {
    (*self).as_gradiator(store)
  }

  fn as_setting(self, store: &mut WorldBuildingState) -> Option<(<T as IntoBevy>::Bevy, crate::update_handler::SettingUpdate)> {
    (*self).as_setting(store)
  }

  fn as_setting_bool(
    self,
    when_true: T,
    when_false: T,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<T as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (*self).as_setting_bool(when_true, when_false, transition, store)
  }

  fn as_setting_num(
    self,
    default: T,
    values: Vec<T>,
    transition: puzzleverse_core::asset::Transition,
    store: &mut WorldBuildingState,
  ) -> (<T as IntoBevy>::Bevy, Option<crate::update_handler::SettingUpdate>) {
    (*self).as_setting_num(default, values, transition, store)
  }
}
