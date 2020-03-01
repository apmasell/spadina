pub(crate) trait IdSource {
  fn extract(&self, ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>);
}

pub(crate) fn extract_global_value_ids<T>(
  value: &puzzleverse_core::asset::GlobalValue<T>,
  ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
) {
  match value {
    puzzleverse_core::asset::GlobalValue::Fixed(_) => (),
    puzzleverse_core::asset::GlobalValue::PuzzleBool { id, .. } => {
      ids.insert(puzzleverse_core::PropertyKey::BoolSink(id.clone()));
    }
    puzzleverse_core::asset::GlobalValue::PuzzleNum { id, .. } => {
      ids.insert(puzzleverse_core::PropertyKey::NumSink(id.clone()));
    }
    puzzleverse_core::asset::GlobalValue::Random(_) => (),
    puzzleverse_core::asset::GlobalValue::Setting(_) => (),
    puzzleverse_core::asset::GlobalValue::SettingBool { .. } => (),
    puzzleverse_core::asset::GlobalValue::SettingNum { .. } => (),
    puzzleverse_core::asset::GlobalValue::Masked(_) => (),
  }
}
pub(crate) fn extract_local_value_ids<T>(
  value: &puzzleverse_core::asset::LocalValue<T>,
  ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
) {
  match value {
    puzzleverse_core::asset::LocalValue::Altitude { .. } => (),
    puzzleverse_core::asset::LocalValue::Global(v) => extract_global_value_ids(v, ids),
    puzzleverse_core::asset::LocalValue::Gradiator(_) => (),
    puzzleverse_core::asset::LocalValue::RandomLocal(_) => (),
  }
}
pub(crate) fn convert_realm(
  realm: puzzleverse_core::asset::SimpleRealmDescription<
    puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyAudio>,
    puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>,
    puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyCustom>,
  >,
  seed: Option<i32>,
) -> Result<
  (
    Vec<std::boxed::Box<(dyn crate::puzzle::PuzzleAsset + 'static)>>,
    Vec<puzzleverse_core::asset::rules::PropagationRule<usize>>,
    crate::realm::navigation::RealmManifold,
    std::collections::BTreeMap<u8, puzzleverse_core::avatar::Effect>,
    std::collections::BTreeMap<std::string::String, puzzleverse_core::RealmSetting>,
  ),
  puzzleverse_core::AssetError,
> {
  fn create_asset_for_logic(logic: &puzzleverse_core::asset::LogicElement, piece_assets: &mut Vec<Box<dyn crate::puzzle::PuzzleAsset>>) {
    piece_assets.push(match logic {
      &puzzleverse_core::asset::LogicElement::Arithemetic(operation) => {
        Box::new(crate::puzzle::arithmetic::ArithmeticAsset(operation)) as Box<dyn crate::puzzle::PuzzleAsset>
      }
      &puzzleverse_core::asset::LogicElement::Buffer(length, buffer_type) => {
        Box::new(crate::puzzle::buffer::BufferAsset { length: length as u32, buffer_type })
      }
      &puzzleverse_core::asset::LogicElement::Clock { period, max, shift } => Box::new(crate::puzzle::clock::ClockAsset { period, max, shift }),
      &puzzleverse_core::asset::LogicElement::Compare(operation, value_type) => {
        Box::new(crate::puzzle::comparator::ComparatorAsset { operation, value_type })
      }
      &puzzleverse_core::asset::LogicElement::Counter(max) => Box::new(crate::puzzle::counter::CounterAsset { max }),
      &puzzleverse_core::asset::LogicElement::HolidayBrazil => {
        Box::new(crate::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::brazil::BRSettlement) })
      }
      &puzzleverse_core::asset::LogicElement::HolidayEaster => {
        Box::new(crate::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::us::USSettlement) })
      }
      &puzzleverse_core::asset::LogicElement::HolidayUnitedStates => {
        Box::new(crate::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::us::USSettlement) })
      }
      &puzzleverse_core::asset::LogicElement::HolidayWeekends => {
        Box::new(crate::puzzle::holiday::HolidayAsset { calendar: std::sync::Arc::new(bdays::calendars::WeekendsOnly) })
      }
      &puzzleverse_core::asset::LogicElement::IndexList(list_type) => Box::new(crate::puzzle::index_list::IndexListAsset(list_type)),
      &puzzleverse_core::asset::LogicElement::Logic(operation) => Box::new(crate::puzzle::logic::LogicAsset(operation)),
      &puzzleverse_core::asset::LogicElement::Metronome(frequency) => Box::new(crate::puzzle::metronome::MetronomeAsset { frequency }),
      &puzzleverse_core::asset::LogicElement::Permutation(length) => Box::new(crate::puzzle::permutation::PermutationAsset { length }),
      &puzzleverse_core::asset::LogicElement::Timer { frequency, initial_counter } => {
        Box::new(crate::puzzle::timer::TimerAsset { frequency, initial_counter })
      }
    });
  }
  fn extract_gradiator_sinks<T>(
    gradiators: &std::collections::BTreeMap<String, puzzleverse_core::asset::gradiator::Gradiator<T>>,
    ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
  ) {
    for gradiator in gradiators.values() {
      for source in &gradiator.sources {
        match &source.source {
          puzzleverse_core::asset::gradiator::Current::Altitude { .. } => (),
          puzzleverse_core::asset::gradiator::Current::Fixed(..) => (),
          puzzleverse_core::asset::gradiator::Current::Setting(..) => (),
          puzzleverse_core::asset::gradiator::Current::BoolControlled { value, .. } => {
            ids.insert(puzzleverse_core::PropertyKey::BoolSink(value.clone()));
          }
          puzzleverse_core::asset::gradiator::Current::NumControlled { value, .. } => {
            ids.insert(puzzleverse_core::PropertyKey::NumSink(value.clone()));
          }
        }
      }
    }
  }
  fn extract_arguments<A: IdSource>(arguments: &[A], ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>) {
    for argument in arguments {
      argument.extract(ids);
    }
  }
  fn extract_light_ids(
    value: &puzzleverse_core::asset::Light<
      puzzleverse_core::asset::GlobalValue<f64>,
      puzzleverse_core::asset::GlobalValue<puzzleverse_core::asset::Color>,
    >,
    ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
  ) {
    match value {
      puzzleverse_core::asset::Light::Point { color, intensity, .. } => {
        extract_global_value_ids(color, ids);
        extract_global_value_ids(intensity, ids);
      }
    }
  }
  fn extract_material_ids(
    material: &puzzleverse_core::asset::Material<
      puzzleverse_core::asset::LocalValue<puzzleverse_core::asset::Color>,
      puzzleverse_core::asset::LocalValue<f64>,
      puzzleverse_core::asset::LocalValue<bool>,
    >,
    ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
  ) {
    match material {
      puzzleverse_core::asset::Material::BrushedMetal { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::Crystal { color, opacity } => {
        extract_local_value_ids(color, ids);
        extract_local_value_ids(opacity, ids);
      }
      puzzleverse_core::asset::Material::Gem { color, accent, glow } => {
        extract_local_value_ids(color, ids);
        extract_local_value_ids(glow, ids);
        if let Some(accent) = accent {
          extract_local_value_ids(accent, ids);
        }
      }
      puzzleverse_core::asset::Material::Metal { color, corrosion } => {
        extract_local_value_ids(color, ids);
        if let Some((corrosion_color, corrosion_intensity)) = corrosion {
          extract_local_value_ids(corrosion_color, ids);
          extract_local_value_ids(corrosion_intensity, ids);
        }
      }
      puzzleverse_core::asset::Material::Rock { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::Sand { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::ShinyMetal { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::Soil { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::Textile { color } => {
        extract_local_value_ids(color, ids);
      }
      puzzleverse_core::asset::Material::TreadPlate { color, corrosion } => {
        extract_local_value_ids(color, ids);
        if let Some(corrosion) = corrosion {
          extract_local_value_ids(corrosion, ids);
        }
      }
      puzzleverse_core::asset::Material::Wood { background, grain } => {
        extract_local_value_ids(background, ids);
        extract_local_value_ids(grain, ids);
      }
    }
  }
  fn extract_spray_element_ids(
    element: &puzzleverse_core::asset::SprayElement<puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>>,
    ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>,
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
    ids.insert(puzzleverse_core::PropertyKey::EventSink(audio.name.clone()));
  }
  for material in &realm.materials {
    extract_material_ids(material, &mut ids);
  }
  for spray in &realm.sprays {
    if let Some(name) = &spray.visible {
      ids.insert(puzzleverse_core::PropertyKey::BoolSink(name.clone()));
    }
    for element in &spray.elements {
      extract_spray_element_ids(element, &mut ids);
    }
  }
  for wall in &realm.walls {
    match wall {
      puzzleverse_core::asset::Wall::Fence { posts, .. } => {
        for post in posts {
          extract_spray_element_ids(post, &mut ids)
        }
      }
      puzzleverse_core::asset::Wall::Gate { arguments, identifier, .. } => {
        extract_arguments(arguments, &mut ids);
        gates
          .entry(puzzleverse_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
          .or_insert_with(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
      }
      puzzleverse_core::asset::Wall::Block { arguments, identifier, .. } => {
        extract_arguments(arguments, &mut ids);
        gates
          .entry(puzzleverse_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
          .or_insert_with(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
      }
      puzzleverse_core::asset::Wall::Solid { .. } => (),
    }
  }
  let mut ids_for_piece = std::collections::HashMap::new();
  let mut piece_assets = Vec::new();
  for (index, logic) in realm.logic.iter().enumerate() {
    ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Logic(index as u32), piece_assets.len());
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
      animation: puzzleverse_core::CharacterAnimation::Walk,
    };
    for (wall_id, wall_path) in platform.walls {
      let wall_body = realm
        .walls
        .get(wall_id as usize)
        .map(|w| match w {
          puzzleverse_core::asset::Wall::Fence { .. } | puzzleverse_core::asset::Wall::Solid { .. } => crate::realm::navigation::Ground::Obstacle,
          puzzleverse_core::asset::Wall::Gate { identifier, .. } | puzzleverse_core::asset::Wall::Block { identifier, .. } => gates
            .get(&puzzleverse_core::asset::SimpleRealmMapId::Wall(identifier.clone()))
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
        transformation: puzzleverse_core::asset::Transformation,
        key: puzzleverse_core::InteractionKey,
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
        puzzleverse_core::asset::PuzzleItem::Button { arguments, enabled, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::puzzle::button::ButtonAsset { enabled, matcher }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            puzzleverse_core::InteractionKey::Button(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: puzzleverse_core::CharacterAnimation::Touch,
              duration: crate::realm::navigation::TOUCH_TIME,
            },
          );
          ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Interact(puzzleverse_core::InteractionKey::Button(name)), piece_id);
        }
        puzzleverse_core::asset::PuzzleItem::CycleButton { arguments, enabled, matcher, name, states, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let max = states;
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::puzzle::cycle_button::CycleButtonAsset { matcher, max, enabled }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            puzzleverse_core::InteractionKey::Button(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: puzzleverse_core::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Interact(puzzleverse_core::InteractionKey::Button(name)), piece_id);
        }
        puzzleverse_core::asset::PuzzleItem::CycleDisplay { arguments, name, .. } => {
          extract_arguments(&arguments, &mut ids);
          ids.insert(puzzleverse_core::PropertyKey::NumSink(name));
        }
        puzzleverse_core::asset::PuzzleItem::Display { arguments, .. } => {
          extract_arguments(&arguments, &mut ids);
        }
        puzzleverse_core::asset::PuzzleItem::Proximity { name, width, length, matcher } => {
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
          piece_assets.push(Box::new(crate::puzzle::proximity::ProximityAsset(matcher)));
          spawn_points.insert(
            name.clone(),
            crate::realm::navigation::SpawnArea { platform: platform_id, x1: item.x, y1: item.y, x2: item.x + width, y2: item.y + length },
          );
          ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Proximity(name), piece_id);
        }
        puzzleverse_core::asset::PuzzleItem::RealmSelector { arguments, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::puzzle::realm_selector::RealmSelectorAsset(matcher)));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            puzzleverse_core::InteractionKey::RealmSelector(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: puzzleverse_core::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece
            .insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Interact(puzzleverse_core::InteractionKey::RealmSelector(name)), piece_id);
        }
        puzzleverse_core::asset::PuzzleItem::Switch { arguments, enabled, initial, matcher, name, transformation, .. } => {
          extract_arguments(&arguments, &mut ids);
          let piece_id = piece_assets.len();
          piece_assets.push(Box::new(crate::puzzle::switch::SwitchAsset { enabled, initial, matcher }));
          add_piece(
            &mut navigation_platform,
            item.x,
            item.y,
            1,
            1,
            transformation,
            puzzleverse_core::InteractionKey::Switch(name.clone()),
            crate::realm::navigation::InteractionInformation {
              piece: piece_id,
              animation: puzzleverse_core::CharacterAnimation::Touch,
              duration: 10,
            },
          );
          ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Interact(puzzleverse_core::InteractionKey::Switch(name)), piece_id);
        }
        puzzleverse_core::asset::PuzzleItem::Custom { item: custom_item, settings, transformation, .. } => match &*custom_item {
          puzzleverse_core::asset::AssetAnyCustom::Simple(c) => {
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
                puzzleverse_core::asset::PuzzleCustomLight::Output { light, id } => {
                  extract_light_ids(light, &mut custom_ids);
                  custom_ids.insert(puzzleverse_core::PropertyKey::BoolSink(id.clone()));
                }
                puzzleverse_core::asset::PuzzleCustomLight::Select { lights, id } => {
                  for l in lights {
                    extract_light_ids(l, &mut custom_ids);
                  }
                  custom_ids.insert(puzzleverse_core::PropertyKey::NumSink(id.clone()));
                }
                puzzleverse_core::asset::PuzzleCustomLight::Static(color) => {
                  extract_light_ids(color, &mut custom_ids);
                }
              }
            }
            for material in c.materials.values() {
              match material {
                puzzleverse_core::asset::PuzzleCustomMaterial::Fixed(m) => extract_material_ids(m, &mut custom_ids),
                puzzleverse_core::asset::PuzzleCustomMaterial::Replaceable { default, .. } => extract_material_ids(default, &mut custom_ids),
              }
            }
            for mesh in &c.meshes {
              fn add_piece_info(
                outer_x: u32,
                outer_y: u32,
                c: &puzzleverse_core::asset::PuzzleCustom<
                  puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyAudio>,
                  puzzleverse_core::asset::Loaded<puzzleverse_core::asset::AssetAnyModel>,
                >,
                transformation: &puzzleverse_core::asset::Transformation,
                x: u32,
                y: u32,
                width: u32,
                length: u32,
                key: &puzzleverse_core::InteractionKey,
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
                    animation: puzzleverse_core::CharacterAnimation::Touch,
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
                puzzleverse_core::asset::PuzzleCustomModel::Button { enabled, length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = puzzleverse_core::InteractionKey::Button(name.clone());
                  piece_assets.push(Box::new(crate::puzzle::button::ButtonAsset {
                    enabled: *enabled,
                    matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher::Any,
                  }));
                  custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                puzzleverse_core::asset::PuzzleCustomModel::Output { name, .. } => {
                  custom_ids.insert(puzzleverse_core::PropertyKey::NumSink(name.clone()));
                }
                puzzleverse_core::asset::PuzzleCustomModel::RadioButton { enabled, initial, length, name, value, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = puzzleverse_core::InteractionKey::RadioButton(name.clone());
                  piece_assets.push(Box::new(crate::puzzle::radio_button::RadioButtonAsset {
                    enabled: *enabled,
                    initial: *initial,
                    matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher::Any,
                    name: name.clone(),
                    value: *value,
                  }));
                  custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                puzzleverse_core::asset::PuzzleCustomModel::RealmSelector { length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = puzzleverse_core::InteractionKey::RealmSelector(name.clone());
                  piece_assets
                    .push(Box::new(crate::puzzle::realm_selector::RealmSelectorAsset(puzzleverse_core::asset::rules::PlayerMarkMatcher::Any)));
                  custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
                puzzleverse_core::asset::PuzzleCustomModel::Static { .. } => (),
                puzzleverse_core::asset::PuzzleCustomModel::Switch { enabled, initial, length, name, width, x, y, .. } => {
                  let piece_id = piece_assets.len();
                  let key = puzzleverse_core::InteractionKey::Switch(name.clone());
                  piece_assets.push(Box::new(crate::puzzle::switch::SwitchAsset {
                    initial: *initial,
                    enabled: *enabled,
                    matcher: puzzleverse_core::asset::rules::PlayerMarkMatcher::Any,
                  }));
                  custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Interact(key.clone()), piece_id);
                  add_piece_info(item.x, item.y, c, &transformation, *x, *y, *width, *length, &key, piece_id, &mut navigation_platform);
                }
              }
            }
            for (logic_id, logic) in c.logic.iter().enumerate() {
              custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Logic(logic_id as u32), piece_assets.len());
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
                    Some(puzzleverse_core::asset::PuzzleCustomGround::Proximity(name)) => {
                      let piece_id = match custom_ids_for_piece.entry(puzzleverse_core::asset::PuzzleCustomInternalId::Proximity(*name)) {
                        std::collections::hash_map::Entry::Vacant(v) => {
                          let piece_id = piece_assets.len();
                          piece_assets
                            .push(Box::new(crate::puzzle::proximity::ProximityAsset(puzzleverse_core::asset::rules::PlayerMarkMatcher::Any)));
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
                    Some(puzzleverse_core::asset::PuzzleCustomGround::Solid) => {
                      navigation_platform.terrain.insert((x, y), crate::realm::navigation::Ground::Obstacle);
                    }
                    Some(puzzleverse_core::asset::PuzzleCustomGround::Suppress) | None => {}
                  }
                }
              }
            }

            for id in custom_ids {
              let piece_id = piece_assets.len();
              piece_assets.push(match &id {
                puzzleverse_core::PropertyKey::BoolSink(name) => Box::new(crate::puzzle::sink::SinkAsset::Bool(name.clone())),
                puzzleverse_core::PropertyKey::EventSink(name) => Box::new(crate::puzzle::event_sink::EventSinkAsset(name.clone())),
                puzzleverse_core::PropertyKey::NumSink(name) => Box::new(crate::puzzle::sink::SinkAsset::Int(name.clone())),
              });
              custom_ids_for_piece.insert(puzzleverse_core::asset::PuzzleCustomInternalId::Property(id), piece_id);
            }
            custom_propagations.extend(c.propagation_rules.iter().flat_map(|p| {
              match (
                custom_ids_for_piece.get(&p.recipient).copied(),
                custom_ids_for_piece.get(&p.sender).copied(),
                match &p.propagation_match {
                  puzzleverse_core::asset::rules::PropagationValueMatcher::AnyToEmpty => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::AnyToEmpty)
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::BoolEmpty { input } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::BoolEmpty { input: *input })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::BoolInvert => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::BoolInvert)
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToBoolList { input, output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToBoolList { input: *input, output: output.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToNum { input, output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToNum { input: *input, output: *output })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToNumList { input, output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::BoolToNumList { input: *input, output: output.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToBool { output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToBool { output: *output })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToBoolList { output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToBoolList { output: output.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm { realm, server } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm { realm: realm.clone(), server: server.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToHome => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToHome)
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToNum { output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToNum { output: *output })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToNumList { output } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToNumList { output: output.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset: asset.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToSettingRealm { setting } => {
                    fn link_to_rule(link: &puzzleverse_core::RealmSettingLink) -> puzzleverse_core::asset::rules::PropagationValueMatcher {
                      match link {
                        puzzleverse_core::RealmSettingLink::Global(realm, server) => {
                          puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToGlobalRealm { realm: realm.clone(), server: server.clone() }
                        }
                        puzzleverse_core::RealmSettingLink::Home => puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToHome,
                        puzzleverse_core::RealmSettingLink::Owner(asset) => {
                          puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToOwnerRealm { asset: asset.clone() }
                        }
                      }
                    }
                    settings
                      .get(setting)
                      .map(|s| match s {
                        puzzleverse_core::asset::PuzzleCustomSettingValue::Realm(link) => match link {
                          puzzleverse_core::asset::GlobalValue::Fixed(link) => Some(link_to_rule(link)),
                          puzzleverse_core::asset::GlobalValue::PuzzleBool { .. } => None,
                          puzzleverse_core::asset::GlobalValue::PuzzleNum { .. } => None,
                          puzzleverse_core::asset::GlobalValue::Random(choices) => match seed {
                            Some(seed) => choices.get(seed.abs() as usize % choices.len()).map(|link| link_to_rule(link)),
                            None => None,
                          },
                          puzzleverse_core::asset::GlobalValue::Setting(setting) => {
                            Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToSettingRealm { setting: setting.clone() })
                          }
                          puzzleverse_core::asset::GlobalValue::SettingBool { .. } => None,
                          puzzleverse_core::asset::GlobalValue::SettingNum { .. } => None,
                          puzzleverse_core::asset::GlobalValue::Masked(_) => None,
                        },
                        _ => None,
                      })
                      .flatten()
                      .or_else(|| {
                        c.settings
                          .get(setting)
                          .map(|s| match s {
                            puzzleverse_core::asset::PuzzleCustomSetting::Realm(link) => Some(link_to_rule(link)),
                            _ => None,
                          })
                          .flatten()
                      })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToSpawnPoint { .. } => None,
                  puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::EmptyToTrainNext)
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::NumToBool { input, comparison } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::NumToBool { input: *input, comparison: comparison.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::NumToBoolList { bits, low_to_high } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::NumToBoolList { bits: *bits, low_to_high: *low_to_high })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::NumToEmpty { input, comparison } => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::NumToEmpty { input: *input, comparison: comparison.clone() })
                  }
                  puzzleverse_core::asset::rules::PropagationValueMatcher::Unchanged => {
                    Some(puzzleverse_core::asset::rules::PropagationValueMatcher::Unchanged)
                  }
                },
              ) {
                (Some(recipient), Some(sender), Some(propagation_match)) => {
                  Some(puzzleverse_core::asset::rules::PropagationRule { causes: p.causes, recipient, sender, trigger: p.trigger, propagation_match })
                }
                _ => None,
              }
              .into_iter()
            }));

            ids_for_piece.extend(custom_ids_for_piece.into_iter().map(|(id, piece)| {
              (puzzleverse_core::asset::SimpleRealmPuzzleId::Custom { platform: platform_id as u32, item: item_id as u32, name: id }, piece)
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
      puzzleverse_core::asset::MaskConfiguration::Bool { masks, .. } => (
        puzzleverse_core::PropertyKey::BoolSink(id.clone()),
        Box::new(crate::puzzle::sink::MultiSinkAsset(id, masks)) as Box<dyn crate::puzzle::PuzzleAsset>,
      ),
      puzzleverse_core::asset::MaskConfiguration::Num { masks, .. } => (
        puzzleverse_core::PropertyKey::NumSink(id.clone()),
        Box::new(crate::puzzle::sink::MultiSinkAsset(id, masks)) as Box<dyn crate::puzzle::PuzzleAsset>,
      ),
    };
    ids.remove(&key);
    piece_assets.push(piece);
    ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Property(key), piece_id);
  }
  for id in ids {
    let piece_id = piece_assets.len();
    piece_assets.push(match &id {
      puzzleverse_core::PropertyKey::BoolSink(name) => Box::new(crate::puzzle::sink::SinkAsset::Bool(name.clone())),
      puzzleverse_core::PropertyKey::EventSink(name) => Box::new(crate::puzzle::event_sink::EventSinkAsset(name.clone())),
      puzzleverse_core::PropertyKey::NumSink(name) => Box::new(crate::puzzle::sink::SinkAsset::Int(name.clone())),
    });
    ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Property(id), piece_id);
  }
  for (id, gate) in gates {
    let piece_id = piece_assets.len();
    piece_assets.push(Box::new(crate::puzzle::map_sink::MapSinkAsset(gate)));
    ids_for_piece.insert(puzzleverse_core::asset::SimpleRealmPuzzleId::Map(id), piece_id);
  }
  if let Some(entry_point) = spawn_points.get(&realm.entry) {
    return Ok((
      piece_assets,
      realm
        .propagation_rules
        .into_iter()
        .flat_map(|r| {
          match (ids_for_piece.get(&r.sender), ids_for_piece.get(&r.recipient)) {
            (Some(&sender), Some(&recipient)) => Some(puzzleverse_core::asset::rules::PropagationRule {
              sender,
              trigger: r.trigger,
              recipient,
              causes: r.causes,
              propagation_match: r.propagation_match,
            }),
            _ => None,
          }
          .into_iter()
        })
        .chain(custom_propagations)
        .collect(),
      crate::realm::navigation::RealmManifold { platforms: navigation_platforms, default_spawn: entry_point.clone(), spawn_points },
      realm.player_effects,
      realm.settings,
    ));
  }
  Err(puzzleverse_core::AssetError::Invalid)
}
impl IdSource for puzzleverse_core::asset::Argument {
  fn extract(&self, ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>) {
    match self {
      puzzleverse_core::asset::Argument::Material(_) => (),
      puzzleverse_core::asset::Argument::Color(c) => extract_local_value_ids(c, ids),
      puzzleverse_core::asset::Argument::Intensity(i) => extract_local_value_ids(i, ids),
    }
  }
}
impl IdSource for puzzleverse_core::asset::CycleArgument {
  fn extract(&self, ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>) {
    match self {
      puzzleverse_core::asset::CycleArgument::Material(_) => (),
      puzzleverse_core::asset::CycleArgument::CycleMaterial(_, _, _) => (),
      puzzleverse_core::asset::CycleArgument::Color(c) => extract_local_value_ids(c, ids),
      puzzleverse_core::asset::CycleArgument::CycleColor(_, _, _) => (),
      puzzleverse_core::asset::CycleArgument::Intensity(i) => extract_local_value_ids(i, ids),
      puzzleverse_core::asset::CycleArgument::CycleIntensity(_, _, _) => (),
    }
  }
}
impl IdSource for puzzleverse_core::asset::SwitchArgument {
  fn extract(&self, ids: &mut std::collections::HashSet<puzzleverse_core::PropertyKey>) {
    match self {
      puzzleverse_core::asset::SwitchArgument::Material(_) => (),
      puzzleverse_core::asset::SwitchArgument::SwitchMaterial(_, _, _) => (),
      puzzleverse_core::asset::SwitchArgument::Color(c) => extract_local_value_ids(c, ids),
      puzzleverse_core::asset::SwitchArgument::SwitchColor(_, _, _) => (),
      puzzleverse_core::asset::SwitchArgument::Intensity(i) => extract_local_value_ids(i, ids),
      puzzleverse_core::asset::SwitchArgument::SwitchIntensity(_, _, _) => (),
    }
  }
}
