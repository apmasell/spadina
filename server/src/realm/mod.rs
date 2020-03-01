pub(crate) mod convert;
pub(crate) mod navigation;

pub(crate) enum LoadError {
  Diesel(diesel::result::Error),
  Serde(rmp_serde::decode::Error),
  Rmp(rmp::decode::ValueReadError),
  NoOwner,
  BadRealm,
}

#[derive(Eq, PartialEq)]
pub(crate) enum Multi<T> {
  Single(T),
  Multi(T, std::collections::BTreeMap<u8, T>),
}

#[derive(Clone)]
pub(crate) struct PlayerMovement {
  pub(crate) player: crate::PlayerKey,
  pub(crate) movement: puzzleverse_core::CharacterMotion<puzzleverse_core::Point>,
  pub(crate) leave_pieces: Vec<usize>,
  pub(crate) enter_pieces: Vec<usize>,
}

pub(crate) struct RealmPuzzleState {
  pub(crate) active_players: std::collections::HashMap<crate::PlayerKey, (puzzleverse_core::Point, Vec<puzzleverse_core::Action>, Option<u8>)>,
  pub(crate) committed_movements: Vec<PlayerMovement>,
  pub(crate) current_states: std::collections::HashMap<puzzleverse_core::PropertyKey, Multi<puzzleverse_core::PropertyValue>>,
  pub(crate) last_update: chrono::DateTime<chrono::Utc>,
  pub(crate) manifold: crate::realm::navigation::RealmManifold,
  pub(crate) pieces: Vec<Box<dyn crate::puzzle::PuzzlePiece>>,
  pub(crate) player_effects: std::collections::BTreeMap<u8, puzzleverse_core::avatar::Effect>,
  pub(crate) propagation_rules: Vec<puzzleverse_core::asset::rules::PropagationRule<usize>>,
  pub(crate) settings: RealmSettings,
}

/// Information about an active realm
pub(crate) struct RealmState {
  pub(crate) access_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) activity: std::sync::Arc<std::sync::atomic::AtomicU16>,
  pub(crate) admin_acl: std::sync::Arc<tokio::sync::Mutex<crate::AccessControlSetting>>,
  pub(crate) asset: String,
  pub(crate) capabilities: Vec<String>,
  pub(crate) consent_epoch: std::sync::Arc<std::sync::atomic::AtomicI32>,
  pub(crate) db_id: i32,
  pub(crate) id: String,
  pub(crate) in_directory: std::sync::Arc<tokio::sync::RwLock<bool>>,
  pub(crate) name: std::sync::Arc<tokio::sync::RwLock<String>>,
  pub(crate) owner: String,
  pub(crate) puzzle_state: std::sync::Arc<crate::prometheus_locks::labelled_mutex::PrometheusLabelledMutex<'static, RealmPuzzleState>>,
  pub(crate) seed: i32,
}
pub type RealmSettings = std::collections::BTreeMap<String, puzzleverse_core::RealmSetting>;

impl<T> Multi<T> {
  fn get(&self, state: &Option<u8>) -> &T {
    match self {
      Multi::Single(value) => value,
      Multi::Multi(default, choices) => match state {
        None => default,
        Some(state) => choices.get(state).unwrap_or(default),
      },
    }
  }
  fn into_inner(self, state: Option<u8>) -> T {
    match self {
      Multi::Single(value) => value,
      Multi::Multi(default, choices) => match state {
        None => default,
        Some(state) => choices.into_iter().filter(|(key, _)| *key == state).map(|(_, value)| value).next().unwrap_or(default),
      },
    }
  }
}

impl<T: Clone> Multi<T> {
  fn convolve<K: Clone + std::cmp::Eq + std::hash::Hash, R>(
    input: &std::collections::HashMap<K, Multi<T>>,
    mapper: impl Fn(std::collections::HashMap<K, T>) -> R,
  ) -> Multi<R> {
    let mut default = std::collections::HashMap::new();
    let mut for_states: std::collections::BTreeMap<_, std::collections::HashMap<_, _>> = Default::default();
    for (key, value) in input {
      match value {
        Multi::Single(value) => {
          default.insert(key.clone(), value.clone());
        }
        Multi::Multi(default_value, values) => {
          default.insert(key.clone(), default_value.clone());
          for (state, value) in values {
            for_states.entry(*state).or_default().insert(key.clone(), value.clone());
          }
        }
      }
    }

    if for_states.is_empty() {
      Multi::Single(mapper(default))
    } else {
      let for_states = for_states
        .into_iter()
        .map(|(state, mut output)| {
          for (key, value) in default.iter() {
            if let std::collections::hash_map::Entry::Vacant(v) = output.entry(key.clone()) {
              v.insert(value.clone());
            }
          }
          (state, mapper(output))
        })
        .collect();
      Multi::Multi(mapper(default), for_states)
    }
  }
}

impl RealmPuzzleState {
  /// Trigger a player interaction
  async fn interact(
    &mut self,
    server: &std::sync::Arc<crate::Server>,
    realm_owner: &str,
    target: usize,
    interaction: &puzzleverse_core::InteractionType,
    player_key: &crate::PlayerKey,
    player_server: &str,
    time: &chrono::DateTime<chrono::Utc>,
    train: Option<u16>,
  ) -> puzzleverse_core::InteractionResult {
    if let Some(piece) = self.pieces.get_mut(target) {
      let (result, events) =
        piece.interact(interaction, player_server, self.active_players.get(player_key).map(|(_, _, state)| state.clone()).flatten());
      let mut links = std::collections::HashMap::new();
      crate::puzzle::process(&mut *self, time, &mut links, events.into_iter().map(|event| crate::puzzle::Event::new(target, event)));
      for (p, rl) in links.iter() {
        match rl {
          puzzleverse_core::asset::rules::RealmLink::Spawn(point) => match self.manifold.warp(Some(point)) {
            Some(point) => {
              if let Some((current_position, _, _)) = self.active_players.get(p) {
                self.committed_movements.retain(|m| m.player != *p || m.movement.time() < time);
                self.committed_movements.push(PlayerMovement {
                  player: p.clone(),
                  leave_pieces: self.manifold.active_proximity(current_position).collect(),
                  enter_pieces: vec![],
                  movement: puzzleverse_core::CharacterMotion::Leave { from: (*current_position).clone(), start: *time },
                });
                self.committed_movements.push(PlayerMovement {
                  player: p.clone(),
                  leave_pieces: vec![],
                  enter_pieces: self.manifold.active_proximity(&point).collect(),
                  movement: puzzleverse_core::CharacterMotion::Enter {
                    to: point,
                    end: *time + chrono::Duration::milliseconds(navigation::WARP_TIME.into()),
                  },
                });
              }
            }
            None => (),
          },
          _ => (),
        }
      }
      links.retain(|_, v| !matches!(v, puzzleverse_core::asset::rules::RealmLink::Spawn(_)));
      server.move_players_from_realm(realm_owner, train, links).await;
      result
    } else {
      puzzleverse_core::InteractionResult::Invalid
    }
  }
  pub(crate) async fn make_update_state(
    &self,
    time: &chrono::DateTime<chrono::Utc>,
    players: &slotmap::DenseSlotMap<crate::PlayerKey, crate::player_state::PlayerState>,
  ) -> Multi<puzzleverse_core::RealmResponse> {
    let mut movements = std::collections::HashMap::new();
    for m in &self.committed_movements {
      if let Some(player_name) = players.get(m.player) {
        match movements.entry(player_name.principal.clone()) {
          std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(puzzleverse_core::PlayerState {
              motion: vec![m.movement.clone()],
              effect: self
                .active_players
                .get(&m.player)
                .map(|(_, _, state)| state.as_ref())
                .flatten()
                .map(|state| self.player_effects.get(state))
                .flatten()
                .cloned()
                .unwrap_or(puzzleverse_core::avatar::Effect::Normal),
              avatar: player_name.avatar.read("update_state").await.clone(),
            });
          }
          std::collections::hash_map::Entry::Occupied(mut entry) => {
            entry.get_mut().motion.push(m.movement.clone());
          }
        }
      }
    }
    Multi::convolve(&self.current_states, |state| puzzleverse_core::RealmResponse::UpdateState {
      time: time.clone(),
      state,
      player: movements.clone(),
    })
  }
  /// Process an event sent by a client
  pub async fn process_realm_event(
    &mut self,
    server: &std::sync::Arc<crate::Server>,
    db_id: i32,
    mut active_player: Option<(&crate::PlayerKey, &mut crate::player_state::MutablePlayerState)>,
    message: Multi<puzzleverse_core::RealmResponse>,
  ) {
    match &message {
      Multi::Single(puzzleverse_core::RealmResponse::NameChanged(name, in_directory)) => {
        if let Err(e) = server.database.realm_rename(db_id, &name, *in_directory) {
          println!("Failed to update realm name: {}", e);
        }
      }
      Multi::Single(puzzleverse_core::RealmResponse::SettingChanged(_, _)) => {
        if let Err(e) = server.database.realm_push_settings(db_id, &self.settings) {
          println!("Failed to update realm settings: {}", e);
        }
      }
      Multi::Single(puzzleverse_core::RealmResponse::MessagePosted { body, sender, timestamp }) => {
        if let Err(e) = server.database.realm_chat_write(db_id, sender, body, timestamp) {
          println!("Failed to update realm name: {}", e);
        }
      }
      _ => (),
    }
    for (player, (_, _, state)) in self.active_players.iter() {
      let message = message.get(state).clone();
      if active_player.as_ref().map(|(id, _)| *id == player).unwrap_or(false) {
        active_player.as_mut().unwrap().1.connection.send(server, player, message).await;
      } else {
        let server = server.clone();
        let player = player.clone();
        tokio::spawn(async move {
          if let Some(state) = server.player_states.read("process_realm_event").await.get(player.clone()) {
            state.mutable.lock().await.connection.send(&server, &player, message).await;
          }
        });
      }
    }
  }
  pub(crate) fn reset(&mut self) {
    let events: Vec<_> = self
      .pieces
      .iter()
      .enumerate()
      .flat_map(|(sender, piece)| piece.reset().into_iter().map(move |event| crate::puzzle::Event::new(sender, event)))
      .collect();
    let mut links = std::collections::HashMap::new();
    crate::puzzle::process(&mut *self, &chrono::Utc::now(), &mut links, events);
    // We can safely throw links away because there are no players to move anywhere.
    if !links.is_empty() {
      eprintln!("Realm wants to move players after reset. It should have no players");
    }
  }
  /// Insert a new player into this realm
  pub(crate) async fn spawn_player(
    &mut self,
    server: &crate::Server,
    realm_name: &str,
    in_directory: bool,
    realm_key: &crate::RealmKey,
    player_key: &crate::PlayerKey,
    player_state: &mut crate::player_state::MutablePlayerState,
  ) {
    match self.manifold.warp(None) {
      Some(point) => {
        player_state.goal = crate::player_state::Goal::InRealm(realm_key.clone(), crate::player_state::RealmGoal::Idle);
        self.active_players.insert(player_key.clone(), (point.clone(), vec![], None));
        self.committed_movements.push(crate::realm::PlayerMovement {
          player: player_key.clone(),
          leave_pieces: vec![],
          enter_pieces: self.manifold.active_proximity(&point).collect(),
          movement: puzzleverse_core::CharacterMotion::Enter { to: point, end: chrono::Utc::now() },
        });
        let players = server.player_states.read("spawn_player").await;
        player_state.connection.send(server, player_key, puzzleverse_core::RealmResponse::NameChanged(realm_name.to_string(), in_directory)).await;
        player_state.connection.send(server, player_key, self.make_update_state(&chrono::Utc::now(), &players).await.into_inner(None)).await;
      }
      None => {
        player_state.goal = crate::player_state::Goal::Undecided;
        self.active_players.remove(player_key);
        player_state.connection.release_player(server).await;
      }
    }
  }
  fn update_locations(&mut self, links: &mut std::collections::HashMap<crate::PlayerKey, puzzleverse_core::asset::rules::RealmLink>) {
    let time = chrono::Utc::now();
    self.committed_movements.sort_by_key(|m| *m.movement.time());
    let possible_movements: Vec<_> =
      self.committed_movements.iter().filter(|m| m.movement.time() > &self.last_update && m.movement.time() < &time).cloned().collect();
    for m in possible_movements {
      if let Some(location) = match &m.movement {
        puzzleverse_core::CharacterMotion::Leave { .. } => None,
        puzzleverse_core::CharacterMotion::Enter { to, .. } => Some((*to).clone()),
        puzzleverse_core::CharacterMotion::Internal { to, .. } => Some((*to).clone()),
        puzzleverse_core::CharacterMotion::Interaction { at, .. } => Some((*at).clone()),
        puzzleverse_core::CharacterMotion::DirectedEmote { at, .. } => Some((*at).clone()),
        puzzleverse_core::CharacterMotion::ConsensualEmoteInitiator { at, .. } => Some((*at).clone()),
        puzzleverse_core::CharacterMotion::ConsensualEmoteRecipient { at, .. } => Some((*at).clone()),
      } {
        match self.active_players.entry(m.player.clone()) {
          std::collections::hash_map::Entry::Vacant(v) => {
            v.insert((location, vec![], None));
          }
          std::collections::hash_map::Entry::Occupied(mut o) => {
            let (l, v, _) = o.get_mut();
            *l = location;
            v.clear();
          }
        }
      } else {
        self.active_players.remove(&m.player);
      }
      for (&target, event) in m
        .leave_pieces
        .iter()
        .zip(std::iter::repeat(crate::realm::navigation::PlayerNavigationEvent::Leave))
        .chain(m.enter_pieces.iter().zip(std::iter::repeat(crate::realm::navigation::PlayerNavigationEvent::Enter)))
      {
        let move_events = match self.pieces.get_mut(target) {
          Some(piece) => piece
            .walk(&m.player, self.active_players.get(&m.player).map(|(_, _, state)| state.clone()).flatten(), event)
            .into_iter()
            .map(|(event_name, value)| crate::puzzle::Event { sender: target, name: event_name, value })
            .collect(),
          None => vec![],
        };
        crate::puzzle::process(self, &time, links, move_events.into_iter());
      }
    }
    self.committed_movements.retain(|m| m.movement.time() < &(time - chrono::Duration::seconds(10)));
    self.last_update = time;
  }
  /// Forcibly extract a player from this realm
  pub(crate) fn yank(
    &mut self,
    player_id: &crate::PlayerKey,
    links: &mut std::collections::HashMap<crate::PlayerKey, puzzleverse_core::asset::rules::RealmLink>,
  ) {
    if let Some((point, _, state)) = self.active_players.remove(player_id) {
      let time = chrono::Utc::now();
      self.committed_movements.retain(|m| m.player != *player_id || m.movement.time() < &time);
      self.committed_movements.push(crate::realm::PlayerMovement {
        player: player_id.clone(),
        movement: puzzleverse_core::CharacterMotion::Leave { from: point.clone(), start: time },
        leave_pieces: self.manifold.active_proximity(&point).collect(),
        enter_pieces: vec![],
      });
      let mut output_events = Vec::new();
      for target in self.manifold.active_proximity(&point) {
        match self.pieces.get_mut(target) {
          Some(piece) => output_events.extend(
            piece
              .walk(player_id, state, crate::realm::navigation::PlayerNavigationEvent::Leave)
              .into_iter()
              .map(|(event_name, value)| crate::puzzle::Event { sender: target, name: event_name, value }),
          ),
          None => (),
        }
      }

      crate::puzzle::process(&mut *self, &time, links, output_events.into_iter());
    }
  }
}
impl RealmState {
  /// Load a realm from the database
  pub(crate) async fn load(server: &std::sync::Arc<crate::Server>, principal: &str) -> Result<RealmState, LoadError> {
    let (db_id, initialised, admin_acls_bin, access_acls_bin, puzzle_state_bin, seed, settings_bin, asset, name, in_directory, owner, train) =
      server.database.realm_load(principal).map_err(LoadError::Diesel)?;

    let train = train.map(|t| t as u16 + 1);
    let admin_acls: crate::AccessControlSetting = rmp_serde::from_read(std::io::Cursor::new(admin_acls_bin.as_slice())).map_err(LoadError::Serde)?;
    let access_acls: crate::AccessControlSetting =
      rmp_serde::from_read(std::io::Cursor::new(access_acls_bin.as_slice())).map_err(LoadError::Serde)?;
    let (capabilities, mut puzzle_state) = server
      .load_realm_description(
        &asset,
        Some(seed),
        Err(LoadError::BadRealm),
        |capabilities, piece_assets, propagation_rules, manifold, player_effects, settings| {
          let now = chrono::Utc::now();
          let mut radio_states = Default::default();
          let pieces = if initialised {
            let mut input = std::io::Cursor::new(puzzle_state_bin.as_slice());
            crate::puzzle::check_length(&mut input, piece_assets.len() as u32).map_err(LoadError::Rmp)?;

            let mut pieces = Vec::new();
            for p in piece_assets.into_iter() {
              pieces.push(p.load(&mut input, &now, &mut radio_states).map_err(LoadError::Rmp)?);
            }
            pieces
          } else {
            piece_assets.into_iter().map(|pa| pa.create(&now, &mut radio_states)).collect()
          };
          let mut state = RealmPuzzleState {
            pieces,
            propagation_rules,
            player_effects,
            manifold,
            active_players: std::collections::HashMap::new(),
            committed_movements: vec![],
            current_states: std::collections::HashMap::new(),
            last_update: chrono::Utc::now(),
            settings,
          };
          crate::puzzle::prepare_consequences(&mut state);
          Ok((capabilities, state))
        },
      )
      .await?;
    for (name, saved_setting) in
      rmp_serde::from_read::<_, std::collections::BTreeMap<String, puzzleverse_core::RealmSetting>>(std::io::Cursor::new(settings_bin.as_slice()))
        .map_err(LoadError::Serde)?
    {
      if let Some(setting) = puzzle_state.settings.get_mut(&name) {
        setting.type_matched_update(&saved_setting);
      }
    }
    puzzle_state.reset();

    let puzzle_state = std::sync::Arc::new(crate::PUZZLE_STATE_LOCK.create(principal.to_string(), puzzle_state));
    let activity = std::sync::Arc::new(std::sync::atomic::AtomicU16::new(0));
    let ps = std::sync::Arc::downgrade(&puzzle_state);
    let s = server.clone();
    let activity_value = activity.clone();
    let realm_owner = owner.as_ref().unwrap().clone();
    // Create a process that updates the realm's internal state and flushes it to players
    tokio::spawn(async move {
      let mut activity_counter = 0;
      loop {
        let last = chrono::Utc::now();
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        if let Some(upgraded) = ps.upgrade() {
          activity_counter = (activity_counter + 1) % 1200;
          let mut state = upgraded.lock("processor").await;
          let now = chrono::Utc::now();
          let mut links = std::collections::HashMap::new();
          crate::puzzle::process_time(&mut *state, &now, &mut links);
          let mut changed = !links.is_empty();
          state.committed_movements.retain(|m| m.movement.time() <= &last || !links.contains_key(&m.player));

          s.move_players_from_realm(&realm_owner, train, links).await;

          let mut had_interaction = false;
          let mut bad_interactions = std::collections::HashMap::new();
          let interactions: Vec<_> = state
            .committed_movements
            .iter()
            .filter(|m| m.movement.time() > &last && m.movement.time() <= &now)
            .flat_map(|m| match &m.movement {
              puzzleverse_core::CharacterMotion::Interaction { interaction, at, target, start, .. } => {
                Some((m.player.clone(), interaction.clone(), at.clone(), target.clone(), start.clone()))
              }
              _ => None,
            })
            .collect();

          for (player, interaction, at, key, time) in interactions.into_iter() {
            if !bad_interactions.contains_key(&player) {
              if let Some(piece_id) = state.manifold.interaction_target(&at, &key) {
                match state
                  .interact(
                    &s,
                    &realm_owner,
                    piece_id,
                    &interaction,
                    &player,
                    s.player_states.read("processor").await.get(player.clone()).map(|p| p.server.as_ref()).flatten().unwrap_or(&s.name),
                    &now,
                    train,
                  )
                  .await
                {
                  puzzleverse_core::InteractionResult::Invalid => eprintln!("Invalid interaction on puzzle piece {} in realm {}", piece_id, db_id),
                  puzzleverse_core::InteractionResult::Failed => {
                    bad_interactions.insert(player, time);
                  }
                  puzzleverse_core::InteractionResult::Accepted => {
                    had_interaction = true;
                  }
                }
              }
            }
          }
          state.committed_movements.retain(|m| match bad_interactions.get(&m.player) {
            None => true,
            Some(time) => m.movement.time() <= time,
          });

          let mut links2 = std::collections::HashMap::new();
          state.update_locations(&mut links2);
          changed |= !links2.is_empty();
          state.committed_movements.retain(|m| m.movement.time() <= &last || !links2.contains_key(&m.player));
          s.move_players_from_realm(&realm_owner, train, links2).await;

          if crate::puzzle::prepare_consequences(&mut state) || changed || had_interaction {
            let players = s.player_states.read("update_state").await;
            let update = state.make_update_state(&now, &players).await;
            state.process_realm_event(&s, db_id, None, update).await;
          }
          let mut cursor = std::io::Cursor::new(Vec::new());
          match rmp::encode::write_array_len(&mut cursor, state.pieces.len() as u32) {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to serialise {}: {}", db_id, e),
          }
          for (piece_id, piece) in state.pieces.iter().enumerate() {
            match piece.serialize(&mut cursor) {
              Ok(_) => {}
              Err(e) => {
                eprintln!("Failed to serialise {} piece {}: {}", db_id, piece_id, e)
              }
            }
          }
          if let Err(e) = &s.database.realm_push_state(db_id, cursor.into_inner()) {
            eprintln!("Failed to write state for realm {}: {}", db_id, e);
          }
          if activity_counter == 0 {
            let player_count = s.player_states.read("update_activity").await.len().try_into().unwrap_or(std::u16::MAX);
            std::mem::drop(activity_value.fetch_update(std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed, |old| {
              Some(old.saturating_add(player_count) / 2)
            }));
          }
        } else {
          break;
        }
      }
    });

    Ok(RealmState {
      access_acl: std::sync::Arc::new(tokio::sync::Mutex::new(access_acls)),
      activity,
      admin_acl: std::sync::Arc::new(tokio::sync::Mutex::new(admin_acls)),
      asset,
      capabilities,
      consent_epoch: std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0)),
      db_id,
      id: principal.to_string(),
      in_directory: std::sync::Arc::new(tokio::sync::RwLock::new(in_directory)),
      name: std::sync::Arc::new(tokio::sync::RwLock::new(name)),
      owner: format!("{}@{}", owner.ok_or(LoadError::NoOwner)?, server.name),
      puzzle_state,
      seed,
    })
  }
}
impl std::fmt::Debug for LoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LoadError::Diesel(v) => v.fmt(f),
      LoadError::Serde(v) => v.fmt(f),
      LoadError::Rmp(v) => v.fmt(f),
      LoadError::NoOwner => f.write_str("Realm has no owner. Missing foreign key constraint in DB?"),
      LoadError::BadRealm => f.write_str("Asset is not a realm that this server can handle."),
    }
  }
}
