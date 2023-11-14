pub struct RealmMechanics {
  pub rules: Vec<spadina_core::asset::rules::PropagationRule<usize, crate::shstr::ShStr>>,
  pub manifold: crate::realm::navigation::RealmManifold,
  pub effects: std::collections::BTreeMap<u8, spadina_core::avatar::Effect>,
  pub settings: std::collections::BTreeMap<crate::shstr::ShStr, spadina_core::realm::RealmSetting<crate::shstr::ShStr>>,
}
pub(crate) trait IdSource {
  fn extract(&self, ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>);
}

pub(crate) fn extract_global_value_ids<T>(
  value: &spadina_core::asset::GlobalValue<T, std::sync::Arc<str>>,
  ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
) {
  match value {
    spadina_core::asset::GlobalValue::Fixed(_) => (),
    spadina_core::asset::GlobalValue::PuzzleBool { id, .. } => {
      ids.insert(spadina_core::realm::PropertyKey::BoolSink(id.clone()));
    }
    spadina_core::asset::GlobalValue::PuzzleNum { id, .. } => {
      ids.insert(spadina_core::realm::PropertyKey::NumSink(id.clone()));
    }
    spadina_core::asset::GlobalValue::Random(_) => (),
    spadina_core::asset::GlobalValue::Setting(_) => (),
    spadina_core::asset::GlobalValue::SettingBool { .. } => (),
    spadina_core::asset::GlobalValue::SettingNum { .. } => (),
    spadina_core::asset::GlobalValue::Masked(_) => (),
  }
}
pub(crate) fn extract_local_blendable_value_ids<T>(
  value: &spadina_core::asset::LocalBlendableValue<T, std::sync::Arc<str>>,
  ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
) {
  match value {
    spadina_core::asset::LocalBlendableValue::Altitude { .. } => (),
    spadina_core::asset::LocalBlendableValue::Global(v) => extract_global_value_ids(v, ids),
    spadina_core::asset::LocalBlendableValue::Gradiator(_) => (),
    spadina_core::asset::LocalBlendableValue::RandomLocal(_) => (),
  }
}
pub(crate) fn extract_local_discrete_value_ids<T>(
  value: &spadina_core::asset::LocalDiscreteValue<T, std::sync::Arc<str>>,
  ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
) {
  match value {
    spadina_core::asset::LocalDiscreteValue::Global(v) => extract_global_value_ids(v, ids),
    spadina_core::asset::LocalDiscreteValue::RandomLocal(_) => (),
  }
}
pub(crate) fn convert_realm(
  realm: spadina_core::asset::SimpleRealmDescription<
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyAudio, std::sync::Arc<str>>,
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyModel, std::sync::Arc<str>>,
    spadina_core::asset::Loaded<spadina_core::asset::AssetAnyCustom<std::sync::Arc<str>>, std::sync::Arc<str>>,
    std::sync::Arc<str>,
  >,
  seed: Option<i32>,
  server_name: &std::sync::Arc<str>,
) -> Result<(Vec<std::boxed::Box<(dyn crate::realm::puzzle::PuzzleAsset + 'static)>>, RealmMechanics), spadina_core::net::server::AssetError> {
  fn create_asset_for_logic(logic: &spadina_core::asset::LogicElement, piece_assets: &mut Vec<Box<dyn crate::realm::puzzle::PuzzleAsset>>) {
    piece_assets.push(match logic {
      &spadina_core::asset::LogicElement::Arithemetic(operation) => {
        Box::new(crate::realm::puzzle::arithmetic::ArithmeticAsset(operation)) as Box<dyn crate::realm::puzzle::PuzzleAsset>
      }
      &spadina_core::asset::LogicElement::Buffer(length, buffer_type) => {
        Box::new(crate::realm::puzzle::buffer::BufferAsset { length: length as u32, buffer_type })
      }
      &spadina_core::asset::LogicElement::Clock { period, max, shift } => Box::new(crate::realm::puzzle::clock::ClockAsset { period, max, shift }),
      &spadina_core::asset::LogicElement::Compare(operation, value_type) => {
        Box::new(crate::realm::puzzle::comparator::ComparatorAsset { operation, value_type })
      }
      &spadina_core::asset::LogicElement::Counter(max) => Box::new(crate::realm::puzzle::counter::CounterAsset { max }),
      &spadina_core::asset::LogicElement::HolidayBrazil => {
        Box::new(crate::realm::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::brazil::BRSettlement) })
      }
      &spadina_core::asset::LogicElement::HolidayEaster => {
        Box::new(crate::realm::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::us::USSettlement) })
      }
      &spadina_core::asset::LogicElement::HolidayUnitedStates => {
        Box::new(crate::realm::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::us::USSettlement) })
      }
      &spadina_core::asset::LogicElement::HolidayWeekends => {
        Box::new(crate::realm::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::WeekendsOnly) })
      }
      &spadina_core::asset::LogicElement::IndexList(list_type) => Box::new(crate::realm::puzzle::index_list::IndexListAsset(list_type)),
      &spadina_core::asset::LogicElement::Logic(operation) => Box::new(crate::realm::puzzle::logic::LogicAsset(operation)),
      &spadina_core::asset::LogicElement::Metronome(frequency) => Box::new(crate::realm::puzzle::metronome::MetronomeAsset { frequency }),
      &spadina_core::asset::LogicElement::Permutation(length) => Box::new(crate::realm::puzzle::permutation::PermutationAsset { length }),
      &spadina_core::asset::LogicElement::Timer { frequency, initial_counter } => {
        Box::new(crate::realm::puzzle::timer::TimerAsset { frequency, initial_counter })
      }
    });
  }
  fn extract_gradiator_sinks<T>(
    gradiators: &std::collections::BTreeMap<std::sync::Arc<str>, spadina_core::asset::gradiator::Gradiator<T, std::sync::Arc<str>>>,
    ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
  ) {
    for gradiator in gradiators.values() {
      for source in &gradiator.sources {
        match &source.source {
          spadina_core::asset::gradiator::Current::Altitude { .. } => (),
          spadina_core::asset::gradiator::Current::Fixed(..) => (),
          spadina_core::asset::gradiator::Current::Setting(..) => (),
          spadina_core::asset::gradiator::Current::BoolControlled { value, .. } => {
            ids.insert(spadina_core::realm::PropertyKey::BoolSink(value.clone()));
          }
          spadina_core::asset::gradiator::Current::NumControlled { value, .. } => {
            ids.insert(spadina_core::realm::PropertyKey::NumSink(value.clone()));
          }
        }
      }
    }
  }
  fn extract_arguments<A: IdSource>(arguments: &[A], ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>) {
    for argument in arguments {
      argument.extract(ids);
    }
  }
  fn extract_light_ids(
    value: &spadina_core::asset::Light<
      spadina_core::asset::GlobalValue<f64, std::sync::Arc<str>>,
      spadina_core::asset::GlobalValue<spadina_core::asset::Color, std::sync::Arc<str>>,
    >,
    ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
  ) {
    match value {
      spadina_core::asset::Light::Point { color, intensity, .. } => {
        extract_global_value_ids(color, ids);
        extract_global_value_ids(intensity, ids);
      }
    }
  }
  fn extract_material_ids(
    material: &spadina_core::asset::Material<
      spadina_core::asset::LocalBlendableValue<spadina_core::asset::Color, std::sync::Arc<str>>,
      spadina_core::asset::LocalBlendableValue<f64, std::sync::Arc<str>>,
      spadina_core::asset::LocalDiscreteValue<bool, std::sync::Arc<str>>,
    >,
    ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
  ) {
    match material {
      spadina_core::asset::Material::BrushedMetal { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::Crystal { color, opacity } => {
        extract_local_blendable_value_ids(color, ids);
        extract_local_blendable_value_ids(opacity, ids);
      }
      spadina_core::asset::Material::Gem { color, accent, glow } => {
        extract_local_blendable_value_ids(color, ids);
        extract_local_discrete_value_ids(glow, ids);
        if let Some(accent) = accent {
          extract_local_blendable_value_ids(accent, ids);
        }
      }
      spadina_core::asset::Material::Metal { color, corrosion } => {
        extract_local_blendable_value_ids(color, ids);
        if let Some((corrosion_color, corrosion_intensity)) = corrosion {
          extract_local_blendable_value_ids(corrosion_color, ids);
          extract_local_blendable_value_ids(corrosion_intensity, ids);
        }
      }
      spadina_core::asset::Material::Rock { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::Sand { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::ShinyMetal { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::Soil { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::Textile { color } => {
        extract_local_blendable_value_ids(color, ids);
      }
      spadina_core::asset::Material::TreadPlate { color, corrosion } => {
        extract_local_blendable_value_ids(color, ids);
        if let Some(corrosion) = corrosion {
          extract_local_blendable_value_ids(corrosion, ids);
        }
      }
      spadina_core::asset::Material::Wood { background, grain } => {
        extract_local_blendable_value_ids(background, ids);
        extract_local_blendable_value_ids(grain, ids);
      }
    }
  }
  fn extract_spray_element_ids(
    element: &spadina_core::asset::SprayElement<
      spadina_core::asset::Loaded<spadina_core::asset::AssetAnyModel, std::sync::Arc<str>>,
      std::sync::Arc<str>,
    >,
    ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>,
  ) {
    extract_arguments(&element.arguments, ids);
  }
  let mut ids = std::collections::HashSet::new();
  let mut gates = std::collections::HashMap::new();
  extract_gradiator_sinks(&realm.gradiators_audio, &mut ids);
  extract_gradiator_sinks(&realm.gradiators_color, &mut ids);
  extract_gradiator_sinks(&realm.gradiators_intensity, &mut ids);
  extract_global_value_ids(&realm.ambient_color, &mut ids);
  extract_global_value_ids(&realm.ambient_intensity, &mut ids);
  for audio in &realm.ambient_audio {
    extract_global_value_ids(&audio.volume, &mut ids);
  }
  for audio in &realm.event_audio {
    extract_global_value_ids(&audio.volume, &mut ids);
    ids.insert(spadina_core::realm::PropertyKey::EventSink(audio.name.clone()));
  }
  for material in &realm.materials {
    extract_material_ids(material, &mut ids);
  }
  for spray in &realm.sprays {
    if let Some(name) = &spray.visible {
      ids.insert(spadina_core::realm::PropertyKey::BoolSink(name.clone()));
    }
    for element in &spray.elements {
      extract_spray_element_ids(element, &mut ids);
    }
  }
  for wall in &realm.walls {
    match wall {
      spadina_core::asset::Wall::Fence { posts, .. } => {
        for post in posts {
          extract_spray_element_ids(post, &mut ids)
        }
      }
      spadina_core::asset::Wall::Gate { arguments, identifier, .. } => {
        extract_arguments(arguments, &mut ids);
        gates
          .entry(spadina_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
          .or_insert_with(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
      }
      spadina_core::asset::Wall::Block { arguments, identifier, .. } => {
        extract_arguments(arguments, &mut ids);
        gates
          .entry(spadina_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
          .or_insert_with(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
      }
      spadina_core::asset::Wall::Solid { .. } => (),
    }
  }
  let mut ids_for_piece = std::collections::HashMap::new();
  let mut piece_assets = Vec::new();
  for (index, logic) in realm.logic.iter().enumerate() {
    ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Logic(index as u32), piece_assets.len());
    create_asset_for_logic(logic, &mut piece_assets);
  }
  let mut navigation_platforms = Vec::new();
  let mut custom_propagations = Vec::new();
  let mut spawn_points = std::collections::HashMap::new();
  for (platform_id, platform) in realm.platforms.into_iter().enumerate() {
    let mut navigation_platform = crate::realm::navigation::Platform {
      width: platform.width,
      length: platform.length,
      terrain: Default::default(),
      animation: spadina_core::realm::CharacterAnimation::Walk,
    };
    for (wall_id, wall_path) in platform.walls {
      let wall_body = realm
        .walls
        .get(wall_id as usize)
        .map(|w| match w {
          spadina_core::asset::Wall::Fence { .. } | spadina_core::asset::Wall::Solid { .. } => crate::realm::navigation::Ground::Obstacle,
          spadina_core::asset::Wall::Gate { identifier, .. } | spadina_core::asset::Wall::Block { identifier, .. } => gates
            .get(&spadina_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
            .cloned()
            .map(crate::realm::navigation::Ground::GatedObstacle)
            .unwrap_or(crate::realm::navigation::Ground::Obstacle),
        })
        .unwrap_or(crate::realm::navigation::Ground::Obstacle);
      for segment in wall_path {
        segment.plot_points(|x, y| {
          navigation_platform.terrain.insert((x, y), wall_body.clone());
        });
      }
    }
    for (item_id, item) in platform.contents.into_iter().enumerate() {
      fn add_piece(
        navigation_platform: &mut crate::realm::navigation::Platform,
        left: u32,
        top: u32,
        width: u32,
        length: u32,
        transformation: spadina_core::asset::Transformation,
        key: spadina_core::realm::InteractionKey<std::sync::Arc<str>>,
        info: crate::realm::navigation::InteractionInformation,
      ) {
        let (x_range, y_range) = transformation.map_range(left, width, top, length);
        for x in x_range {
          for y in y_range.clone() {
            match navigation_platform.terrain.entry((x, y)) {
              std::collections::btree_map::Entry::Occupied(mut o) => {
                if let crate::realm::navigation::Ground::Pieces { interaction, .. } = o.get_mut() {
                  interaction.insert(key.clone(), info.clone());
                }
              }
              std::collections::btree_map::Entry::Vacant(v) => {
                v.insert(crate::realm::navigation::Ground::Pieces {
                  interaction: std::iter::once((key.clone(), info.clone())).collect(),
                  proximity: Vec::new(),
                });
              }
            }
          }
        }
      }
      match item.item {
        spadina_core::asset::PuzzleItem::Button { arguments, enabled, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::realm::puzzle::button::ButtonAsset { enabled, matcher }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            spadina_core::realm::InteractionKey::Button(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: spadina_core::realm::CharacterAnimation::Touch,
              duration: crate::realm::navigation::TOUCH_TIME,
            },
          );
          ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Interact(spadina_core::realm::InteractionKey::Button(name)), piece_id);
        }
        spadina_core::asset::PuzzleItem::CycleButton { arguments, enabled, matcher, name, states, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let max = states;
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::realm::puzzle::cycle_button::CycleButtonAsset { matcher, max, enabled }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            spadina_core::realm::InteractionKey::Button(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: spadina_core::realm::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Interact(spadina_core::realm::InteractionKey::Button(name)), piece_id);
        }
        spadina_core::asset::PuzzleItem::CycleDisplay { arguments, name, .. } => {
          extract_arguments(&arguments, &mut ids);
          ids.insert(spadina_core::realm::PropertyKey::NumSink(name));
        }
        spadina_core::asset::PuzzleItem::Display { arguments, .. } => {
          extract_arguments(&arguments, &mut ids);
        }
        spadina_core::asset::PuzzleItem::Proximity { name, width, length, matcher } => {
          let piece_id = piece_assets.len();
          for x in item.x..=(item.x + width) {
            for y in item.y..=(item.y + length) {
              match navigation_platform.terrain.entry((x, y)) {
                std::collections::btree_map::Entry::Occupied(mut o) => {
                  if let crate::realm::navigation::Ground::Pieces { proximity, .. } = o.get_mut() {
                    proximity.push(piece_id);
                  }
                }
                std::collections::btree_map::Entry::Vacant(v) => {
                  v.insert(crate::realm::navigation::Ground::Pieces { interaction: Default::default(), proximity: vec![piece_id] });
                }
              }
            }
          }
          piece_assets.push(Box::new(crate::realm::puzzle::proximity::ProximityAsset(matcher)));
          spawn_points.insert(
            name.clone(),
            crate::realm::navigation::SpawnArea { platform: platform_id, x1: item.x, y1: item.y, x2: item.x + width, y2: item.y + length },
          );
          ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Proximity(name), piece_id);
        }
        spadina_core::asset::PuzzleItem::RealmSelector { arguments, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::realm::puzzle::realm_selector::RealmSelectorAsset(matcher)));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            spadina_core::realm::InteractionKey::RealmSelector(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: spadina_core::realm::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece
            .insert(spadina_core::asset::SimpleRealmPuzzleId::Interact(spadina_core::realm::InteractionKey::RealmSelector(name)), piece_id);
        }
        spadina_core::asset::PuzzleItem::Switch { arguments, enabled, initial, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::realm::puzzle::switch::SwitchAsset { enabled, initial, matcher }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            spadina_core::realm::InteractionKey::Switch(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: spadina_core::realm::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Interact(spadina_core::realm::InteractionKey::Switch(name)), piece_id);
        }
        spadina_core::asset::PuzzleItem::Custom { item: custom_item, settings, transformation, .. } => match &*custom_item {
          spadina_core::asset::AssetAnyCustom::Simple(c) => {
            let mut custom_ids = std::collections::HashSet::new();
            let mut custom_ids_for_piece = std::collections::HashMap::new();
            for audio in &c.ambient_audio {
              extract_global_value_ids(&audio.volume, &mut custom_ids);
            }
            for audio in &c.event_audio {
              extract_global_value_ids(&audio.volume, &mut custom_ids);
            }
            for light in &c.lights {
              match light {
                spadina_core::asset::PuzzleCustomLight::Output { light, id } => {
                  extract_light_ids(light, &mut custom_ids);
                  custom_ids.insert(spadina_core::realm::PropertyKey::BoolSink(id.clone()));
                }
                spadina_core::asset::PuzzleCustomLight::Select { lights, id } => {
                  for l in lights {
                    extract_light_ids(l, &mut custom_ids);
                  }
                  custom_ids.insert(spadina_core::realm::PropertyKey::NumSink(id.clone()));
                }
                spadina_core::asset::PuzzleCustomLight::Static(color) => {
                  extract_light_ids(color, &mut custom_ids);
                }
              }
            }
            for material in c.materials.values() {
              match material {
                spadina_core::asset::PuzzleCustomMaterial::Fixed(m) => extract_material_ids(m, &mut custom_ids),
                spadina_core::asset::PuzzleCustomMaterial::Replaceable { default, .. } => extract_material_ids(default, &mut custom_ids),
              }
            }
            for mesh in &c.meshes {
              fn add_piece_info(
                outer_x: u32,
                outer_y: u32,
                c: &spadina_core::asset::PuzzleCustom<
                  spadina_core::asset::Loaded<spadina_core::asset::AssetAnyAudio, std::sync::Arc<str>>,
                  spadina_core::asset::Loaded<spadina_core::asset::AssetAnyModel, std::sync::Arc<str>>,
                  std::sync::Arc<str>,
                >,
                transformation: &spadina_core::asset::Transformation,
                x: u32,
                y: u32,
                width: u32,
                length: u32,
                key: &spadina_core::realm::InteractionKey<std::sync::Arc<str>>,
                piece_id: usize,
                navigation_platform: &mut crate::realm::navigation::Platform,
              ) {
                if let Some((x_range, y_range)) = transformation.map_child_ranges(
                  outer_x,
                  outer_y,
                  c.ground.len() as u32,
                  c.ground.get(0).map(Vec::len).unwrap_or(0) as u32,
                  x,
                  y,
                  width,
                  length,
                ) {
                  let info = crate::realm::navigation::InteractionInformation {
                    piece: piece_id,
                    animation: spadina_core::realm::CharacterAnimation::Touch,
                    duration: crate::realm::navigation::TOUCH_TIME,
                  };
                  for x in x_range {
                    for y in y_range.clone() {
                      match navigation_platform.terrain.entry((x, y)) {
                        std::collections::btree_map::Entry::Vacant(v) => {
                          v.insert(crate::realm::navigation::Ground::Pieces {
                            interaction: std::iter::once((key.clone(), info.clone())).collect(),
                            proximity: Vec::new(),
                          });
                        }
                        std::collections::btree_map::Entry::Occupied(mut u) => {
                          if let crate::realm::navigation::Ground::Pieces { interaction, .. } = u.get_mut() {
                            interaction.insert(key.clone(), info.clone());
                          }
                        }
                      }
                    }
                  }
                }
              }
              match mesh {
                spadina_core::asset::PuzzleCustomModel::Button { enabled, length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = spadina_core::realm::InteractionKey::Button(name.clone());
                  piece_assets.push(Box::new(crate::realm::puzzle::button::ButtonAsset {
                    enabled: *enabled,
                    matcher: spadina_core::asset::rules::PlayerMarkMatcher::Any,
                  }));
                  custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                spadina_core::asset::PuzzleCustomModel::Output { name, .. } => {
                  custom_ids.insert(spadina_core::realm::PropertyKey::NumSink(name.clone()));
                }
                spadina_core::asset::PuzzleCustomModel::RadioButton { enabled, initial, length, name, value, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = spadina_core::realm::InteractionKey::RadioButton(name.clone());
                  piece_assets.push(Box::new(crate::realm::puzzle::radio_button::RadioButtonAsset {
                    enabled: *enabled,
                    initial: *initial,
                    matcher: spadina_core::asset::rules::PlayerMarkMatcher::Any,
                    name: crate::shstr::ShStr::Shared(name.clone()),
                    value: *value,
                  }));
                  custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                spadina_core::asset::PuzzleCustomModel::RealmSelector { length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = spadina_core::realm::InteractionKey::RealmSelector(name.clone());
                  piece_assets
                    .push(Box::new(crate::realm::puzzle::realm_selector::RealmSelectorAsset(spadina_core::asset::rules::PlayerMarkMatcher::Any)));
                  custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                spadina_core::asset::PuzzleCustomModel::Static { .. } => (),
                spadina_core::asset::PuzzleCustomModel::Switch { enabled, initial, length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = spadina_core::realm::InteractionKey::Switch(name.clone());
                  piece_assets.push(Box::new(crate::realm::puzzle::switch::SwitchAsset {
                    initial: *initial,
                    enabled: *enabled,
                    matcher: spadina_core::asset::rules::PlayerMarkMatcher::Any,
                  }));
                  custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
              }
            }
            for (logic_id, logic) in c.logic.iter().enumerate() {
              custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Logic(logic_id as u32), piece_assets.len());
              create_asset_for_logic(logic, &mut piece_assets);
            }
            for (x, row) in c.ground.iter().enumerate() {
              for (y, v) in row.iter().enumerate() {
                if let Some((x, y)) = transformation.translate(
                  item.x,
                  item.y,
                  c.ground.len() as u32,
                  c.ground.get(0).map(Vec::len).unwrap_or(0) as u32,
                  x as u32,
                  y as u32,
                ) {
                  match v {
                    Some(spadina_core::asset::PuzzleCustomGround::Proximity(name)) => {
                      let piece_id = match custom_ids_for_piece.entry(spadina_core::asset::PuzzleCustomInternalId::Proximity(*name)) {
                        std::collections::hash_map::Entry::Vacant(v) => {
                          let piece_id = piece_assets.len();
                          piece_assets
                            .push(Box::new(crate::realm::puzzle::proximity::ProximityAsset(spadina_core::asset::rules::PlayerMarkMatcher::Any)));
                          v.insert(piece_id);
                          piece_id
                        }
                        std::collections::hash_map::Entry::Occupied(o) => *o.get(),
                      };
                      match navigation_platform.terrain.entry((x, y)) {
                        std::collections::btree_map::Entry::Vacant(v) => {
                          v.insert(crate::realm::navigation::Ground::Pieces { proximity: vec![piece_id], interaction: Default::default() });
                        }
                        std::collections::btree_map::Entry::Occupied(mut o) => {
                          if let crate::realm::navigation::Ground::Pieces { proximity, .. } = o.get_mut() {
                            proximity.push(piece_id);
                          }
                        }
                      }
                    }
                    Some(spadina_core::asset::PuzzleCustomGround::Solid) => {
                      navigation_platform.terrain.insert((x, y), crate::realm::navigation::Ground::Obstacle);
                    }
                    Some(spadina_core::asset::PuzzleCustomGround::Suppress) | None => {}
                  }
                }
              }
            }

            for id in custom_ids {
              let piece_id = piece_assets.len();
              piece_assets.push(match &id {
                spadina_core::realm::PropertyKey::BoolSink(name) => {
                  Box::new(crate::realm::puzzle::sink::SinkAsset::Bool(crate::shstr::ShStr::Shared(name.clone())))
                }
                spadina_core::realm::PropertyKey::EventSink(name) => {
                  Box::new(crate::realm::puzzle::event_sink::EventSinkAsset(crate::shstr::ShStr::Shared(name.clone())))
                }
                spadina_core::realm::PropertyKey::NumSink(name) => {
                  Box::new(crate::realm::puzzle::sink::SinkAsset::Int(crate::shstr::ShStr::Shared(name.clone())))
                }
              });
              custom_ids_for_piece.insert(spadina_core::asset::PuzzleCustomInternalId::Property(id), piece_id);
            }
            custom_propagations.extend(c.propagation_rules.iter().flat_map(|p| {
              match (
                custom_ids_for_piece.get(&p.recipient).copied(),
                custom_ids_for_piece.get(&p.sender).copied(),
                match &p.propagation_match {
                  spadina_core::asset::rules::PropagationValueMatcher::AnyToEmpty => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::AnyToEmpty)
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::BoolEmpty { input } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::BoolEmpty { input: *input })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::BoolInvert => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::BoolInvert)
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::BoolToBoolList { input, output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::BoolToBoolList { input: *input, output: output.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::BoolToNum { input, output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::BoolToNum { input: *input, output: *output })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::BoolToNumList { input, output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::BoolToNumList { input: *input, output: output.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToBool { output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToBool { output: *output })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToBoolList { output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToBoolList { output: output.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm { owner, asset, server } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm {
                      asset: asset.clone().into(),
                      owner: owner.clone().into(),
                      server: server.clone().into(),
                    })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToHome => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToHome)
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToNum { output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToNum { output: *output })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToNumList { output } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToNumList { output: output.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset: asset.clone().into() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToSettingRealm { setting } => {
                    fn link_to_rule(
                      link: &spadina_core::realm::RealmTarget<std::sync::Arc<str>>,
                      server_name: &std::sync::Arc<str>,
                    ) -> spadina_core::asset::rules::PropagationValueMatcher<crate::shstr::ShStr> {
                      match link {
                        spadina_core::realm::RealmTarget::RemoteRealm { asset, owner, server } => {
                          spadina_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm {
                            asset: asset.clone().into(),
                            owner: owner.clone().into(),
                            server: server.clone().into(),
                          }
                        }
                        spadina_core::realm::RealmTarget::Home => spadina_core::asset::rules::PropagationValueMatcher::EmptyToHome,
                        spadina_core::realm::RealmTarget::PersonalRealm { asset } => {
                          spadina_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset: asset.clone().into() }
                        }
                        spadina_core::realm::RealmTarget::LocalRealm { asset, owner } => {
                          spadina_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm {
                            asset: asset.clone().into(),
                            owner: owner.clone().into(),
                            server: server_name.clone().into(),
                          }
                        }
                      }
                    }
                    settings
                      .get(setting)
                      .map(|s| match s {
                        spadina_core::asset::PuzzleCustomSettingValue::Realm(link) => match link {
                          spadina_core::asset::GlobalValue::Fixed(link) => Some(link_to_rule(link, server_name)),
                          spadina_core::asset::GlobalValue::PuzzleBool { .. } => None,
                          spadina_core::asset::GlobalValue::PuzzleNum { .. } => None,
                          spadina_core::asset::GlobalValue::Random(choices) => match seed {
                            Some(seed) => choices.get(seed.abs() as usize % choices.len()).map(|link| link_to_rule(link, server_name)),
                            None => None,
                          },
                          spadina_core::asset::GlobalValue::Setting(setting) => {
                            Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToSettingRealm { setting: setting.clone().into() })
                          }
                          spadina_core::asset::GlobalValue::SettingBool { .. } => None,
                          spadina_core::asset::GlobalValue::SettingNum { .. } => None,
                          spadina_core::asset::GlobalValue::Masked(_) => None,
                        },
                        _ => None,
                      })
                      .flatten()
                      .or_else(|| {
                        c.settings
                          .get(setting)
                          .map(|s| match s {
                            spadina_core::asset::PuzzleCustomSetting::Realm(link) => Some(link_to_rule(link, server_name)),
                            _ => None,
                          })
                          .flatten()
                      })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToSpawnPoint { .. } => None,
                  spadina_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext)
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::NumToBool { input, comparison } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::NumToBool { input: *input, comparison: comparison.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::NumToBoolList { bits, low_to_high } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::NumToBoolList { bits: *bits, low_to_high: *low_to_high })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::NumToEmpty { input, comparison } => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::NumToEmpty { input: *input, comparison: comparison.clone() })
                  }
                  spadina_core::asset::rules::PropagationValueMatcher::Unchanged => {
                    Some(spadina_core::asset::rules::PropagationValueMatcher::Unchanged)
                  }
                },
              ) {
                (Some(recipient), Some(sender), Some(propagation_match)) => {
                  Some(spadina_core::asset::rules::PropagationRule { causes: p.causes, recipient, sender, trigger: p.trigger, propagation_match })
                }
                _ => None,
              }
              .into_iter()
            }));

            ids_for_piece.extend(custom_ids_for_piece.into_iter().map(|(id, piece)| {
              (spadina_core::asset::SimpleRealmPuzzleId::Custom { platform: platform_id as u32, item: item_id as u32, name: id }, piece)
            }));
          }
        },
      }
    }
    navigation_platforms.push(navigation_platform);
  }
  for (id, mask) in realm.masks {
    let piece_id = piece_assets.len();
    let (key, piece) = match mask {
      spadina_core::asset::MaskConfiguration::Bool { masks, .. } => (
        spadina_core::realm::PropertyKey::BoolSink(id.clone()),
        Box::new(crate::realm::puzzle::sink::MultiSinkAsset(crate::shstr::ShStr::Shared(id), masks)) as Box<dyn crate::realm::puzzle::PuzzleAsset>,
      ),
      spadina_core::asset::MaskConfiguration::Num { masks, .. } => (
        spadina_core::realm::PropertyKey::NumSink(id.clone()),
        Box::new(crate::realm::puzzle::sink::MultiSinkAsset(crate::shstr::ShStr::Shared(id), masks)) as Box<dyn crate::realm::puzzle::PuzzleAsset>,
      ),
    };
    ids.remove(&key);
    piece_assets.push(piece);
    ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Property(key), piece_id);
  }
  for id in ids {
    let piece_id = piece_assets.len();
    piece_assets.push(match &id {
      spadina_core::realm::PropertyKey::BoolSink(name) => {
        Box::new(crate::realm::puzzle::sink::SinkAsset::Bool(crate::shstr::ShStr::Shared(name.clone())))
      }
      spadina_core::realm::PropertyKey::EventSink(name) => {
        Box::new(crate::realm::puzzle::event_sink::EventSinkAsset(crate::shstr::ShStr::Shared(name.clone())))
      }
      spadina_core::realm::PropertyKey::NumSink(name) => {
        Box::new(crate::realm::puzzle::sink::SinkAsset::Int(crate::shstr::ShStr::Shared(name.clone())))
      }
    });
    ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Property(id), piece_id);
  }
  for (id, gate) in gates {
    let piece_id = piece_assets.len();
    piece_assets.push(Box::new(crate::realm::puzzle::map_sink::MapSinkAsset(gate)));
    ids_for_piece.insert(spadina_core::asset::SimpleRealmPuzzleId::Map(id), piece_id);
  }
  if let Some(entry_point) = spawn_points.get(&realm.entry) {
    return Ok((
      piece_assets,
      RealmMechanics {
        rules: realm
          .propagation_rules
          .into_iter()
          .flat_map(|r| {
            match (ids_for_piece.get(&r.sender), ids_for_piece.get(&r.recipient)) {
              (Some(&sender), Some(&recipient)) => Some(spadina_core::asset::rules::PropagationRule {
                sender,
                trigger: r.trigger,
                recipient,
                causes: r.causes,
                propagation_match: r.propagation_match.convert_str(),
              }),
              _ => None,
            }
            .into_iter()
          })
          .chain(custom_propagations)
          .collect(),
        manifold: crate::realm::navigation::RealmManifold { platforms: navigation_platforms, default_spawn: entry_point.clone(), spawn_points },
        effects: realm.player_effects,
        settings: realm.settings.into_iter().map(|(k, v)| (k.into(), v.convert_str())).collect(),
      },
    ));
  }
  Err(spadina_core::net::server::AssetError::Invalid)
}
impl IdSource for spadina_core::asset::Argument<std::sync::Arc<str>> {
  fn extract(&self, ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>) {
    match self {
      spadina_core::asset::Argument::Material(_) => (),
      spadina_core::asset::Argument::Color(c) => extract_local_blendable_value_ids(c, ids),
      spadina_core::asset::Argument::Intensity(i) => extract_local_blendable_value_ids(i, ids),
    }
  }
}
impl IdSource for spadina_core::asset::CycleArgument<std::sync::Arc<str>> {
  fn extract(&self, ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>) {
    match self {
      spadina_core::asset::CycleArgument::Material(_) => (),
      spadina_core::asset::CycleArgument::CycleMaterial(_, _, _) => (),
      spadina_core::asset::CycleArgument::Color(c) => extract_local_blendable_value_ids(c, ids),
      spadina_core::asset::CycleArgument::CycleColor(_, _, _) => (),
      spadina_core::asset::CycleArgument::Intensity(i) => extract_local_blendable_value_ids(i, ids),
      spadina_core::asset::CycleArgument::CycleIntensity(_, _, _) => (),
    }
  }
}
impl IdSource for spadina_core::asset::SwitchArgument<std::sync::Arc<str>> {
  fn extract(&self, ids: &mut std::collections::HashSet<spadina_core::realm::PropertyKey<std::sync::Arc<str>>>) {
    match self {
      spadina_core::asset::SwitchArgument::Material(_) => (),
      spadina_core::asset::SwitchArgument::SwitchMaterial(_, _, _) => (),
      spadina_core::asset::SwitchArgument::Color(c) => extract_local_blendable_value_ids(c, ids),
      spadina_core::asset::SwitchArgument::SwitchColor(_, _, _) => (),
      spadina_core::asset::SwitchArgument::Intensity(i) => extract_local_blendable_value_ids(i, ids),
      spadina_core::asset::SwitchArgument::SwitchIntensity(_, _, _) => (),
    }
  }
}
