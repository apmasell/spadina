pub(super) enum RealmLoad<S> {
  Fetch(Vec<String>),
  Corrupt(std::borrow::Cow<'static, str>),
  Loaded((std::sync::Arc<S::World>, super::Paths)),
}
pub trait PlatformBuilder<Material>: Send + Sync + 'static {
  fn new(base: spadina_core::asset::PlatformBase, material: &Material, x: u32, y: u32, z: u32, length: u32, width: u32) -> Self;
}

pub trait WorldBuilder: Default + Send + Sync + 'static {
  type Platform: PlatformBuilder<Self::Material>;
  type Material;
  type World: location::WorldRenderer;
  type IntensityGradiator: crate::gradiator::IntoGradiator<f64>;
  type ColorGradiator: crate::gradiator::IntoGradiator<spadina_core::asset::Color>;
  type Mesh: Clone;
  fn create_mesh(&mut self, mesh: &spadina_core::asset::Mesh) -> Self::Mesh;
  fn add(&mut self, platform: Self::Platform);
  fn finish(self) -> Self::World;
}
struct MeshCache<S: super::WorldBuilder>(
  std::collections::BTreeMap<String, std::sync::Arc<spadina_core::asset::SimpleSprayModel<S::Mesh, u32, u32, u32>>>,
);
impl<S: super::WorldBuilder> MeshCache<S> {
  fn upsert(
    &mut self,
    world_builder: &mut S,
    mesh: spadina_core::asset::Loaded<spadina_core::asset::AssetAnyModel, String>,
  ) -> std::sync::Arc<spadina_core::asset::SimpleSprayModel<S::Mesh, u32, u32, u32>> {
    match self.0.entry(mesh.asset().to_string()) {
      std::collections::btree_map::Entry::Occupied(o) => o.get().clone(),
      std::collections::btree_map::Entry::Vacant(mut v) => match &*mesh {
        spadina_core::asset::AssetAnyModel::Simple(spadina_core::asset::SimpleSprayModel { meshes, lights }) => v
          .insert(std::sync::Arc::new(spadina_core::asset::SimpleSprayModel {
            meshes: meshes
              .iter()
              .map(|element| spadina_core::asset::SprayModelElement { mesh: world_builder.create_mesh(&element.mesh), material: element.material })
              .collect(),
            lights: lights.clone(),
          }))
          .clone(),
      },
    }
  }
}
impl<S: super::WorldBuilder> Default for MeshCache<S> {
  fn default() -> Self {
    Self(Default::default())
  }
}
pub(super) async fn load<S: super::WorldBuilder>(
  asset_store: &impl spadina_core::asset_store::AsyncAssetStore,
  info: &super::RealmInfo<super::RealmLoading<S::World>>,
) -> RealmLoad<S> {
  let realm_asset = match asset_store.pull(&info.asset).await {
    Ok(asset) => asset,
    Err(spadina_core::asset_store::LoadError::Corrupt) => {
      return RealmLoad::Corrupt(std::borrow::Cow::Owned(format!("Asset {} is corrupt on disk.", &info.asset)));
    }
    Err(spadina_core::asset_store::LoadError::InternalError) => {
      return RealmLoad::Corrupt(std::borrow::Cow::Owned(format!("Internal error accessing asset {}", &info.asset)));
    }
    Err(spadina_core::asset_store::LoadError::Unknown) => {
      return RealmLoad::Fetch(vec![info.asset.clone()]);
    }
  };
  match spadina_core::asset::AssetAnyRealm::load(realm_asset, &asset_store).await {
    Ok((spadina_core::asset::AssetAnyRealm::Simple(realm), _)) => simple_realm(realm, info.seed).await,
    Err(spadina_core::AssetError::Missing(missing)) => RealmLoad::Fetch(missing),
    Err(e) => RealmLoad::Corrupt(e.description()),
  }
}
async fn simple_realm<S: super::WorldBuilder>(
  realm: spadina_core::asset::SimpleRealmDescription<
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyAudio, String>,
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyModel, String>,
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyCustom<String>, String>,
    String,
  >,
  seed: i32,
) -> RealmLoad<S> {
  let mut builder = S::default();
  let mut bool_updates = std::collections::BTreeMap::new();
  let mut num_updates = std::collections::BTreeMap::new();
  //let gradiators_audio = crate::gradiator::load(realm.gradiators_audio, &mut bool_updates, &mut num_updates);
  let gradiators_color = match crate::gradiator::load::<_, S::ColorGradiator>(realm.gradiators_color, &mut bool_updates, &mut num_updates) {
    Ok(gradiators) => gradiators,
    Err(e) => return RealmLoad::Corrupt(e.into()),
  };
  let gradiators_intensity = match crate::gradiator::load::<_, S::IntensityGradiator>(realm.gradiators_intensity, &mut bool_updates, &mut num_updates)
  {
    Ok(gradiators) => gradiators,
    Err(e) => return RealmLoad::Corrupt(e.into()),
  };
  let mut materials = Vec::new();
  let default_material = materials_assets.add(bevy::render::color::Color::rgb(0.5, 0.5, 0.5).into());
  //pub aesthetic: Aesthetic,
  for material in realm.materials {
    match material {
      spadina_core::asset::Material::BrushedMetal { color } => todo!(),
    }
  }

  //pub ambient_audio: Vec<AmbientAudio<A>>,
  //pub event_audio: Vec<EventAudio<A>>,

  let mut updates = std::collections::HashMap::new();

  updates.extend(
    bool_updates
      .into_iter()
      .map(|(name, target)| (spadina_core::realm::PropertyKey::BoolSink(name), vec![crate::update_handler::Update::BoolShared(target)])),
  );

  updates.extend(
    num_updates
      .into_iter()
      .map(|(name, target)| (spadina_core::realm::PropertyKey::NumSink(name), vec![crate::update_handler::Update::NumShared(target)])),
  );
  let mut settings = Default::default();
  ambient_light.color = convert::convert_global(realm.ambient_color, convert::AmbientLight, &mut world_building_state);
  ambient_light.brightness =
    convert::convert_global(realm.ambient_intensity, convert::AmbientLight, &mut world_building_state) as f32 * convert::MAX_ILLUMINATION;

  let mut paths: super::Paths = Default::default();
  let sprays = realm
    .sprays
    .into_iter()
    .enumerate()
    .map(|(index, spray)| {
      spray::ConvertedSpray::new(spray, &mut meshes, seed, &mut world_building_state).ok_or(format!("Cannot convert spray {}", index))
    })
    .collect::<Result<Vec<_>, _>>()?;

  let walls: Vec<_> = realm.walls.into_iter().map(|wall| spray::ConvertedWall::new(wall, &mut meshes, seed, &mut world_building_state)).collect();

  for (platform_id, platform) in realm.platforms.into_iter().enumerate() {
    let mut platform_builder = S::Platform::new(platform.base, platform.x, platform.y, platform.z, platform.length, platform.width);
    let mut occupied = std::collections::BTreeSet::new();
    for spadina_core::asset::PlatformItem { x, y, item } in platform.contents {
      match item {
        spadina_core::asset::PuzzleItem::Button { arguments, enabled, model, name, transformation, .. } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              Some(spadina_core::InteractionKey::Button(name)),
              None,
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::Switch { arguments, enabled, initial, model, name, transformation, .. } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              Some(spadina_core::InteractionKey::Switch(name.clone())),
              Some(spadina_core::PropertyKey::BoolSink(name)),
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::CycleButton { arguments, enabled, model, name, states, transformation, .. } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              Some(spadina_core::InteractionKey::Button(name.clone())),
              Some(spadina_core::PropertyKey::NumSink(name)),
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::CycleDisplay { arguments, model, name, states, transformation } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              None,
              Some(spadina_core::PropertyKey::NumSink(name)),
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::Display { arguments, model, transformation } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              None,
              None,
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::RealmSelector { arguments, model, name, transformation, .. } => match *model {
          spadina_core::asset::AssetAnyModel::Simple(model) => {
            convert::add_mesh(
              &mut commands,
              &mut world_building_state,
              Some(spadina_core::InteractionKey::RealmSelector(name)),
              None,
              model,
              arguments,
              platform_id as u32,
              x + platform.x,
              y + platform.y,
              platform.z,
              transformation,
            );
          }
        },
        spadina_core::asset::PuzzleItem::Proximity { .. } => (),
        spadina_core::asset::PuzzleItem::Custom { item, transformation, gradiators_color, gradiators_intensity, materials, settings } => {
          match item {}
        }
      }
    }
    for (wall_id, wall_path) in platform.walls {
      if let Some(Some(wall)) = walls.get(wall_id as usize) {
        for segment in wall_path {
          segment.plot_points(|x, y| {
            let x = platform.x + x;
            let y = platform.y + y;
            let random = ((seed as i64).abs() as u64).wrapping_mul(x as u64).wrapping_mul(y as u64);

            let is_solid = match wall {
              spray::ConvertedWall::Solid { width, width_perturbation, material } => {
                let width = *width + width_perturbation.compute(seed, x, y).1;

                let mut spawn = commands.spawn();
                let source = spawn.id();
                spawn.insert_bundle(bevy::pbr::PbrBundle {
                  mesh: meshes.add(shape::Box::new(width, width, 1.0).into()),
                  material: material.at(x, y, platform.z, source, &mut world_building_state),
                  global_transform: Transform::from_xyz(x as f32 + 0.5, y as f32 + 0.5, platform.z as f32 + 0.5).into(),
                  ..Default::default()
                });
                true
              }
              spray::ConvertedWall::Fence { angle, posts, vertical, vertical_perturbation } => {
                let index = random % posts.iter().map(|(weight, _)| (*weight).max(1) as u64).sum();
                let mut accumulator = 0u32;
                let (_, model) = posts
                  .iter()
                  .skip_while(|(weight, _)| {
                    accumulator += (*weight).max(1) as u32;
                    index < accumulator
                  })
                  .next()
                  .unwrap();
                model.instantiate(
                  &mut commands,
                  x,
                  y,
                  platform.z,
                  seed,
                  angle,
                  if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                  vertical_perturbation,
                  &mut world_building_state,
                );
                true
              }
              spray::ConvertedWall::Gate { angle, model, vertical, vertical_perturbation } => {
                model.instantiate(
                  &mut commands,
                  x,
                  y,
                  platform.z,
                  seed,
                  angle,
                  if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                  vertical_perturbation,
                  &mut world_building_state,
                );
                false
              }
              spray::ConvertedWall::Block { angle, identifier, model, vertical, vertical_perturbation } => {
                let update = crate::update_handler::Update::BoolVisibility(
                  model
                    .instantiate(
                      &mut commands,
                      x,
                      y,
                      platform.z,
                      seed,
                      angle,
                      if *vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                      vertical_perturbation,
                      &mut world_building_state,
                    )
                    .insert(bevy::render::view::visibility::Visibility { is_visible: true })
                    .id(),
                );
                world_building_state.updates.entry(spadina_core::PropertyKey::BoolSink(identifier.clone())).or_default().push(update);
                false
              }
            };
            if is_solid {
              world_building_state.occupied.insert((x, y));
            }
          })
        }
      }
    }

    for x in 0..=platform.width {
      for y in 0..=platform.length {
        if !world_building_state.occupied.contains(&(x, y)) {
          if x > 0 {
            if y > 0 && !world_building_state.occupied.contains(&(x - 1, y - 1)) {
              paths.entry(spadina_core::Point { platform: platform_id as u32, x: x - 1, y: y - 1 }).or_default().push(spadina_core::Point {
                platform: platform_id as u32,
                x,
                y,
              });
            }
            if !world_building_state.occupied.contains(&(x - 1, y)) {
              paths.entry(spadina_core::Point { platform: platform_id as u32, x: x - 1, y }).or_default().push(spadina_core::Point {
                platform: platform_id as u32,
                x,
                y,
              });
            }
            if y < platform.length && !world_building_state.occupied.contains(&(x - 1, y + 1)) {
              paths.entry(spadina_core::Point { platform: platform_id as u32, x: x - 1, y: y + 1 }).or_default().push(spadina_core::Point {
                platform: platform_id as u32,
                x,
                y,
              });
            }
          }
          if y > 0 {
            if !world_building_state.occupied.contains(&(x, y - 1)) {
              paths.entry(spadina_core::Point { platform: platform_id as u32, x, y: y - 1 }).or_default().push(spadina_core::Point {
                platform: platform_id as u32,
                x,
                y,
              });
            }
            if x < platform.width && !world_building_state.occupied.contains(&(x + 1, y - 1)) {
              paths.entry(spadina_core::Point { platform: platform_id as u32, x, y }).or_default().push(spadina_core::Point {
                platform: platform_id as u32,
                x: x + 1,
                y: y - 1,
              });
            }
          }
          let position = spadina_core::Point { platform: platform_id as u32, x, y };
          let x = platform.x + x;
          let y = platform.y + y;
          let random = ((seed as i64).abs() as u64).wrapping_mul(x as u64).wrapping_mul(y as u64);
          let index = random
            % platform
              .sprays
              .iter()
              .copied()
              .flat_map(|id| sprays.get(id as usize).into_iter())
              .flat_map(|spray| spray.elements.iter())
              .map(|(weight, _)| (*weight).max(1) as u64)
              .sum();
          let mut accumulator = 0u64;
          match platform
            .sprays
            .iter()
            .copied()
            .flat_map(|id| sprays.get(id as usize).into_iter())
            .flat_map(|spray| spray.elements.iter().map(|(weight, model)| (*weight, model, spray)))
            .skip_while(|(weight, _, _)| {
              accumulator += (*weight).max(1) as u64;
              index < accumulator
            })
            .next()
          {
            Some((_, model, spray)) => {
              let child = model
                .instantiate(
                  &mut commands,
                  x,
                  y,
                  platform.z,
                  seed,
                  &spray.angle,
                  if spray.vertical { &bevy::math::Quat::IDENTITY } else { &platform_normal },
                  &spray.vertical_perturbation,
                  &mut world_building_state,
                )
                .id();
              let mut commands = commands.spawn();
              commands.add_child(child);
              commands
            }
            None => commands.spawn(),
          }
          .with_children(|builder| {
            let mut commands = builder.spawn();
            commands.insert_bundle(bevy::pbr::PbrBundle {
              mesh: ground_square,
              material: match materials.get(platform.material as usize) {
                Some(material) => material.at(x, y, platform.z, commands.id(), &mut world_building_state),
                None => default_material.clone(),
              },
              transform: Transform::from_translation(bevy::math::Vec3::new(x as f32, y as f32, platform.z as f32)),
              ..Default::default()
            });
          })
          .insert(Target(position));
        }
      }
    }
  }
  RealmLoad::Loaded(builder.finish())
}
