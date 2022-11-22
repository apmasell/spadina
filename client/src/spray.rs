use bevy::prelude::BuildChildren;

pub struct ConvertedModel<M, C, I> {
  pub meshes: Vec<(bevy::asset::Handle<bevy::render::mesh::Mesh>, M)>,
  pub lights: Vec<puzzleverse_core::asset::Light<I, C>>,
}
impl ConvertedModel<u32, u32, u32> {
  fn new(
    source: puzzleverse_core::asset::AssetAnyModel,
    meshes: &mut bevy::prelude::Assets<bevy::render::mesh::Mesh>,
  ) -> ConvertedModel<u32, u32, u32> {
    match source {
      puzzleverse_core::asset::AssetAnyModel::Simple(model) => {
        ConvertedModel { meshes: model.meshes.into_iter().map(|mesh| crate::convert::convert_mesh(meshes, mesh)).collect(), lights: model.lights }
      }
    }
  }
  fn bind<A: crate::convert::ExtractArgument>(
    &self,
    args: &[A],
    name: &A::LocalName,
    seed: i32,
    state: &mut crate::convert::WorldBuildingState,
  ) -> Option<
    ConvertedModel<
      crate::convert::LocalBevyValue<u32>,
      crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
      crate::convert::LocalBevyValue<f64>,
    >,
  > {
    Some(ConvertedModel {
      meshes: self
        .meshes
        .iter()
        .map(|(mesh, material)| match args.get(*material as usize).map(|arg| arg.extract_material_local(name, seed, state)).flatten() {
          Some(m) => Some((mesh.clone(), m)),
          None => None,
        })
        .collect::<Option<_>>()?,
      lights: self
        .lights
        .iter()
        .map(|light| match light {
          puzzleverse_core::asset::Light::Point { position, color, intensity } => match (
            args.get(*color as usize).map(|arg| arg.extract_color_local(name, seed, state)).flatten(),
            args.get(*intensity as usize).map(|arg| arg.extract_light_intensity_local(name, seed, state)).flatten(),
          ) {
            (Some(color), Some(intensity)) => Some(puzzleverse_core::asset::Light::Point { position: position.clone(), color, intensity }),
            _ => None,
          },
        })
        .collect::<Option<_>>()?,
    })
  }
}

impl
  ConvertedModel<
    crate::convert::LocalBevyValue<u32>,
    crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
    crate::convert::LocalBevyValue<f64>,
  >
{
  pub fn instantiate<'w, 's, 'a>(
    &self,
    commands: &'a mut bevy::ecs::system::Commands<'w, 's>,
    x: u32,
    y: u32,
    z: u32,
    seed: i32,
    angle: &puzzleverse_core::asset::Angle,
    vertical: &bevy::math::Quat,
    vertical_perturbation: &puzzleverse_core::asset::Perturbation,
    state: &mut crate::convert::WorldBuildingState,
  ) -> bevy::ecs::system::EntityCommands<'w, 's, 'a> {
    let mut spawn = commands.spawn();
    spawn.with_children(|builder| {
      for (mesh, material) in &self.meshes {
        let mut spawn = builder.spawn();
        let material = material.at(x, y, z, spawn.id(), state);
        spawn.insert_bundle(bevy::pbr::PbrBundle {
          mesh: mesh.clone(),
          material,
          transform: Default::default(),
          global_transform: Default::default(),
          visibility: Default::default(),
          computed_visibility: Default::default(),
        });
      }
      for light in &self.lights {
        let mut spawn = builder.spawn();
        match light {
          puzzleverse_core::asset::Light::Point { position, color, intensity } => {
            let color = color.at(x, y, z, Box::new(crate::convert::LightEntity(spawn.id())), state);
            let intensity = intensity.at(x, y, z, Box::new(crate::convert::LightEntity(spawn.id())), state) as f32 * crate::convert::MAX_ILLUMINATION;
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
              transform: bevy::transform::components::Transform::from_xyz(position.0, position.1, position.2),
              global_transform: Default::default(),
              visibility: Default::default(),
            });
          }
        }
      }
    });
    let (perturbation_angle, perturbation_from_vertical) = vertical_perturbation.compute(seed, x, y);
    let mut transform = bevy::transform::components::Transform::from_xyz(x as f32, y as f32, z as f32);
    transform.rotate(
      bevy::math::Quat::from_rotation_z(angle.compute(seed, x, y))
        * (*vertical)
        * bevy::math::Quat::from_euler(bevy::math::EulerRot::XZY, perturbation_angle, perturbation_from_vertical, 0.0),
    );
    spawn.insert(bevy::transform::components::GlobalTransform::from(transform));

    spawn
  }
}

pub struct ConvertedSpray {
  pub angle: puzzleverse_core::asset::Angle,
  pub elements: Vec<(
    u8,
    ConvertedModel<
      crate::convert::LocalBevyValue<u32>,
      crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
      crate::convert::LocalBevyValue<f64>,
    >,
  )>,
  pub vertical: bool,
  pub vertical_perturbation: puzzleverse_core::asset::Perturbation,
}
impl ConvertedSpray {
  pub fn new(
    source: puzzleverse_core::asset::Spray<puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>>,
    meshes: &mut bevy::prelude::Assets<bevy::render::mesh::Mesh>,
    seed: i32,
    state: &mut crate::convert::WorldBuildingState,
  ) -> Option<ConvertedSpray> {
    Some(ConvertedSpray {
      angle: source.angle,
      elements: source
        .elements
        .into_iter()
        .map(|element| match ConvertedModel::new(element.model.into_inner(), meshes).bind(&element.arguments, &(), seed, state) {
          None => None,
          Some(spray) => Some((element.weight, spray)),
        })
        .collect::<Option<_>>()?,
      vertical: source.vertical,
      vertical_perturbation: source.vertical_perturbation,
    })
  }
}
pub(crate) enum ConvertedWall {
  Solid {
    width: f32,
    width_perturbation: puzzleverse_core::asset::Perturbation,
    material: crate::convert::LocalBevyValue<u32>,
  },
  Fence {
    /// The rotation that should be applied to each model
    angle: puzzleverse_core::asset::Angle,
    posts: Vec<(
      u8,
      ConvertedModel<
        crate::convert::LocalBevyValue<u32>,
        crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
        crate::convert::LocalBevyValue<f64>,
      >,
    )>,
    vertical: bool,
    vertical_perturbation: puzzleverse_core::asset::Perturbation,
  },
  Gate {
    angle: puzzleverse_core::asset::Angle,
    model: ConvertedModel<
      crate::convert::LocalBevyValue<u32>,
      crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
      crate::convert::LocalBevyValue<f64>,
    >,
    vertical: bool,
    vertical_perturbation: puzzleverse_core::asset::Perturbation,
  },
  Block {
    angle: puzzleverse_core::asset::Angle,
    identifier: String,
    model: ConvertedModel<
      crate::convert::LocalBevyValue<u32>,
      crate::convert::LocalBevyValue<puzzleverse_core::asset::Color>,
      crate::convert::LocalBevyValue<f64>,
    >,
    vertical: bool,
    vertical_perturbation: puzzleverse_core::asset::Perturbation,
  },
}

impl ConvertedWall {
  pub fn new(
    source: puzzleverse_core::asset::Wall<puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>>,
    meshes: &mut bevy::prelude::Assets<bevy::render::mesh::Mesh>,
    seed: i32,
    state: &mut crate::convert::WorldBuildingState,
  ) -> Option<ConvertedWall> {
    Some(match source {
      puzzleverse_core::asset::Wall::Fence { angle, posts, vertical, vertical_perturbation } => ConvertedWall::Fence {
        angle,
        posts: posts
          .into_iter()
          .map(|element| match ConvertedModel::new(element.model.into_inner(), meshes).bind(&element.arguments, &(), seed, state) {
            None => None,
            Some(spray) => Some((element.weight, spray)),
          })
          .collect::<Option<_>>()?,
        vertical,
        vertical_perturbation,
      },
      puzzleverse_core::asset::Wall::Solid { width, width_perturbation, material } => {
        ConvertedWall::Solid { width, width_perturbation, material: state.materials.get(material as usize)?.clone() }
      }
      puzzleverse_core::asset::Wall::Gate { angle, arguments, identifier, model, vertical, vertical_perturbation } => ConvertedWall::Gate {
        angle,
        model: ConvertedModel::new(model.into_inner(), meshes).bind(&arguments, &identifier, seed, state)?,
        vertical,
        vertical_perturbation,
      },
      puzzleverse_core::asset::Wall::Block { angle, arguments, identifier, model, vertical, vertical_perturbation } => ConvertedWall::Block {
        angle,
        model: ConvertedModel::new(model.into_inner(), meshes).bind(&arguments, &(), seed, state)?,
        identifier,
        vertical,
        vertical_perturbation,
      },
    })
  }
}
