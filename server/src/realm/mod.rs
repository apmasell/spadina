use spadina_core::asset_store::AsyncAssetStore;

use crate::realm::puzzle::PlayerKey;

pub mod convert;
pub mod navigation;
pub mod output;
pub mod persistence;
pub mod puzzle;

struct ActivePlayer {
  committed_movements: Vec<spadina_core::realm::CharacterMotion<spadina_core::realm::Point, std::sync::Arc<str>>>,
  action_gate: ActionGate,
  current_direction: spadina_core::realm::Direction,
  current_position: spadina_core::realm::Point,
  principal: crate::destination::SharedPlayerId,
  remaining_actions: std::collections::VecDeque<spadina_core::realm::Action<std::sync::Arc<str>>>,
  state: Option<u8>,
}

#[derive(Eq, PartialEq)]
enum ActionGate {
  Enter(Vec<usize>),
  Interact(spadina_core::realm::InteractionKey<std::sync::Arc<str>>, spadina_core::realm::InteractionType<std::sync::Arc<str>>),
  Transition { leave: Vec<usize>, enter: Vec<usize> },
  Stop,
}

/// Information about an active realm
#[pin_project::pin_project(project = RealmProjection)]
pub(crate) struct Realm {
  access_acl: crate::database::persisted::PersistedLocal<persistence::RealmAccess>,
  active_players: std::collections::HashMap<PlayerKey, ActivePlayer>,
  admin_acl: crate::database::persisted::PersistedLocal<persistence::RealmAdmin>,
  announcements: crate::database::persisted::PersistedLocal<persistence::RealmAnnouncements>,
  asset: std::sync::Arc<str>,
  capabilities: std::collections::BTreeSet<&'static str>,
  current_states: std::collections::HashMap<spadina_core::realm::PropertyKey<crate::shstr::ShStr>, output::Multi<spadina_core::realm::PropertyValue>>,
  database: std::sync::Arc<crate::database::Database>,
  db_id: i32,
  #[pin]
  next_update: std::pin::Pin<Box<tokio::time::Sleep>>,
  manifold: crate::realm::navigation::RealmManifold,
  name_and_in_directory: crate::database::persisted::PersistedLocal<persistence::NameAndInDirectory>,
  owner: std::sync::Arc<str>,
  pieces: Vec<Box<dyn puzzle::PuzzlePiece>>,
  player_effects: std::collections::BTreeMap<u8, spadina_core::avatar::Effect>,
  player_principals: std::collections::HashMap<crate::destination::SharedPlayerId, PlayerKey>,
  propagation_rules: Vec<spadina_core::asset::rules::PropagationRule<usize, crate::shstr::ShStr>>,
  seed: i32,
  server_name: std::sync::Arc<str>,
  settings: crate::database::persisted::PersistedLocal<persistence::Settings>,
  solved: bool,
  train_next: Option<u16>,
}

pub type RealmSettings = std::collections::BTreeMap<crate::shstr::ShStr, spadina_core::realm::RealmSetting<crate::shstr::ShStr>>;

enum Special {
  Normal,
  Dirty,
  DeadPlayer(spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>, spadina_core::realm::PlayerState<crate::shstr::ShStr>),
}

impl crate::destination::Owner for spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>> {
  fn owner(&self) -> &std::sync::Arc<str> {
    &self.owner
  }
}

impl Realm {
  pub fn new(
    database: std::sync::Arc<crate::database::Database>,
    directory: std::sync::Arc<crate::destination::Directory>,
    server_name: std::sync::Arc<str>,
    launch: crate::destination::RealmLaunch,
  ) -> crate::destination::manager::DestinationManager<Self> {
    let identifier = match &launch {
      crate::destination::RealmLaunch::Existing { owner, asset, .. } => {
        spadina_core::realm::LocalRealmTarget { asset: asset.clone(), owner: owner.clone() }
      }
      crate::destination::RealmLaunch::New { owner, asset, .. } => {
        spadina_core::realm::LocalRealmTarget { asset: asset.clone(), owner: owner.clone() }
      }
    };
    crate::destination::manager::DestinationManager::new(
      identifier,
      server_name.clone(),
      std::sync::Arc::downgrade(&directory),
      tokio::spawn(async move {
        let realm_asset_id = match &launch {
          crate::destination::RealmLaunch::Existing { asset, .. } => asset,
          crate::destination::RealmLaunch::New { asset, .. } => asset,
        }
        .clone();
        let realm_asset = match directory.asset_manager().pull(&realm_asset_id).await {
          Ok(v) => v,
          Err(e) => {
            eprintln!("Failed to load {}: {}", &realm_asset_id, e);
            return Err(match e {
              spadina_core::asset_store::LoadError::Corrupt | spadina_core::asset_store::LoadError::InternalError => {
                spadina_core::location::LocationResponse::InternalError
              }
              spadina_core::asset_store::LoadError::Unknown => spadina_core::location::LocationResponse::ResolutionFailed,
            });
          }
        };
        let (realm_asset, capabilities) =
          match spadina_core::asset::AssetAnyRealm::<std::sync::Arc<str>>::load(realm_asset, directory.asset_manager()).await {
            Ok(v) => v,
            Err(e) => {
              eprintln!("Failed to load {}: {}", &realm_asset_id, e);
              return Err(match e {
                spadina_core::AssetError::DecodeFailure
                | spadina_core::AssetError::InternalError
                | spadina_core::AssetError::Invalid
                | spadina_core::AssetError::PermissionError
                | spadina_core::AssetError::UnknownKind => spadina_core::location::LocationResponse::InternalError,
                spadina_core::AssetError::Missing(_) => spadina_core::location::LocationResponse::ResolutionFailed,
              });
            }
          };

        let capabilities = match spadina_core::capabilities::all_supported(capabilities) {
          Ok(capabilities) => capabilities,
          Err(capability) => return Err(spadina_core::location::LocationResponse::MissingCapabilities { capabilities: vec![capability] }),
        };
        let (owner, db_id, state, seed, solved, train_next) = match launch {
          crate::destination::RealmLaunch::Existing { db_id, owner, .. } => match database.realm_load(db_id) {
            Err(e) => {
              eprintln!("Failed to load realm {}: {}", db_id, e);
              return Err(spadina_core::location::LocationResponse::InternalError);
            }
            Ok(crate::database::RealmLoadInfo { seed, solved, state, train }) => (owner, db_id, state, seed, solved, train.map(|v| v + 1)),
          },
          crate::destination::RealmLaunch::New { owner, asset, train } => {
            use rand::Rng;
            let seed: i32 = rand::thread_rng().gen();
            let name = realm_asset.name_for(&owner);
            match database.realm_create(&asset, &owner, &name, seed, train) {
              Ok(db_id) => (owner, db_id, None, seed, false, train.map(|v| v + 1)),
              Err(e) => {
                eprintln!("Failed to create realm {} for {}: {}", &asset, &owner, e);
                return Err(spadina_core::location::LocationResponse::InternalError);
              }
            }
          }
        };

        let realm_asset_result = match realm_asset {
          spadina_core::asset::AssetAnyRealm::Simple(realm) => convert::convert_realm(realm, Some(seed), &server_name),
        };
        let now = chrono::Utc::now();
        let (pieces, mechanics) = match realm_asset_result {
          Err(e) => {
            eprintln!("Failed to convert realm {}: {}", db_id, e);
            return Err(spadina_core::location::LocationResponse::InternalError);
          }
          Ok((puzzle_assets, mechanics)) => (
            {
              let mut radio_states = Default::default();
              match state {
                Some(state) if state.len() == puzzle_assets.len() => {
                  match state
                    .into_iter()
                    .zip(puzzle_assets)
                    .map(|(state, asset)| asset.load(state, &now, &mut radio_states))
                    .collect::<Result<Vec<_>, _>>()
                  {
                    Err(e) => {
                      eprintln!("Failed to load realm state {}: {}", db_id, e);
                      return Err(spadina_core::location::LocationResponse::InternalError);
                    }
                    Ok(states) => states,
                  }
                }
                _ => puzzle_assets.into_iter().map(|asset| asset.create(&now, &mut radio_states)).collect(),
              }
            },
            mechanics,
          ),
        };
        let mut settings = match crate::database::persisted::PersistedLocal::new(database.clone(), persistence::Settings(db_id)) {
          Ok(v) => v,
          Err(e) => {
            eprintln!("Failed to load realm settings {}: {}", db_id, e);
            return Err(spadina_core::location::LocationResponse::InternalError);
          }
        };
        settings.mutate(|current_settings| {
          current_settings.retain(|k, v| mechanics.settings.get(k).map(|value| value.type_name() == v.type_name()).unwrap_or(false));
          for (key, value) in mechanics.settings {
            std::mem::drop(current_settings.try_insert(key, value));
          }
          spadina_core::UpdateResult::Success
        });
        let mut realm = Realm {
          access_acl: match crate::database::persisted::PersistedLocal::new(database.clone(), persistence::RealmAccess(db_id)) {
            Ok(v) => v,
            Err(e) => {
              eprintln!("Failed to load realm access ACL {}: {}", db_id, e);
              return Err(spadina_core::location::LocationResponse::InternalError);
            }
          },
          active_players: Default::default(),
          asset: realm_asset_id.clone(),
          admin_acl: match crate::database::persisted::PersistedLocal::new(database.clone(), persistence::RealmAdmin(db_id)) {
            Ok(v) => v,
            Err(e) => {
              eprintln!("Failed to load realm admin ACL {}: {}", db_id, e);
              return Err(spadina_core::location::LocationResponse::InternalError);
            }
          },
          announcements: match crate::database::persisted::PersistedLocal::new(database.clone(), persistence::RealmAnnouncements(db_id)) {
            Ok(v) => v,
            Err(e) => {
              eprintln!("Failed to load realm admin ACL {}: {}", db_id, e);
              return Err(spadina_core::location::LocationResponse::InternalError);
            }
          },
          capabilities,
          current_states: Default::default(),
          database: database.clone(),
          db_id,
          next_update: Box::pin(tokio::time::sleep(std::time::Duration::from_secs(0))),
          manifold: mechanics.manifold,
          name_and_in_directory: match crate::database::persisted::PersistedLocal::new(database, persistence::NameAndInDirectory(db_id)) {
            Ok(v) => v,
            Err(e) => {
              eprintln!("Failed to load realm name and directory status {}: {}", db_id, e);
              return Err(spadina_core::location::LocationResponse::InternalError);
            }
          },
          owner,
          pieces,
          player_effects: mechanics.effects,
          player_principals: Default::default(),
          propagation_rules: mechanics.rules,
          seed,
          server_name,
          settings,
          solved,
          train_next,
        };
        let events: Vec<_> = realm
          .pieces
          .iter()
          .enumerate()
          .flat_map(|(sender, piece)| piece.reset().into_iter().map(move |event| crate::realm::puzzle::Event::new(sender, event)))
          .collect();
        let mut links = std::collections::HashMap::new();
        crate::realm::puzzle::process(&mut realm, &chrono::Utc::now(), &mut links, events);
        // We can safely throw links away because there are no players to move anywhere.
        if !links.is_empty() {
          eprintln!("Realm {} wants to move players after reset. It should have no players", &db_id);
        }

        Ok(realm)
      }),
    )
  }
  async fn process_events_special(
    &mut self,
    events: Vec<puzzle::Event>,
    special: Special,
  ) -> Vec<crate::destination::DestinationControl<spadina_core::realm::RealmResponse<crate::shstr::ShStr>>> {
    let time = chrono::Utc::now();
    let mut links = std::collections::HashMap::new();
    let mut output = Vec::new();
    let mut dirty = !links.is_empty() || special.is_dirty();
    puzzle::process(self, &time, &mut links, events);
    for (player, link_out) in links {
      if let Some(player_info) = self.active_players.get_mut(&player) {
        match link_out {
          spadina_core::asset::rules::LinkOut::Spawn(point) => match self.manifold.warp(Some(point.as_str())) {
            Some(point) => {
              player_info
                .committed_movements
                .push(spadina_core::realm::CharacterMotion::Leave { from: player_info.current_position.clone(), start: time });
              player_info.committed_movements.push(spadina_core::realm::CharacterMotion::Enter {
                to: point,
                end: time + chrono::Duration::milliseconds(navigation::WARP_TIME.into()),
              });
              player_info.current_position = point;
              player_info.action_gate = ActionGate::Enter(self.manifold.active_proximity(&point).collect());
            }
            None => (),
          },
          spadina_core::asset::rules::LinkOut::Realm(realm) => {
            player_info
              .committed_movements
              .push(spadina_core::realm::CharacterMotion::Leave { from: player_info.current_position.clone(), start: time });
            output.push(crate::destination::DestinationControl::Move(player, Some(realm.convert_str())));
          }
          spadina_core::asset::rules::LinkOut::TrainNext => {
            if !self.solved {
              self.solved = true;
            }
            player_info
              .committed_movements
              .push(spadina_core::realm::CharacterMotion::Leave { from: player_info.current_position.clone(), start: time });
            output.push(match &self.train_next {
              None => crate::destination::DestinationControl::Move(player, None),
              Some(train_next) => crate::destination::DestinationControl::MoveTrain(player, self.owner.clone(), *train_next),
            });
          }
        }
      }
    }
    match self.pieces.iter().map(|piece| piece.serialize()).collect() {
      Ok(state) => {
        if let Err(e) = self.database.realm_push_state(self.db_id, state, self.solved) {
          eprintln!("Failed to write realm state to database for {}: {}", self.db_id, e);
        }
      }
      Err(e) => {
        eprintln!("Failed to serialize realm state for {}: {}", self.db_id, e);
      }
    }
    if puzzle::prepare_consequences(self) {
      dirty = true;
    }
    let now = chrono::Utc::now();
    for player_info in self.active_players.values_mut() {
      if !player_info.remaining_actions.is_empty() && player_info.action_gate == ActionGate::Stop {
        dirty = true;
        let mut start = now;
        let mut bad = false;
        while let Some(action) = player_info.remaining_actions.pop_front() {
          match action {
            spadina_core::realm::Action::Emote { animation, duration } => {
              player_info.committed_movements.push(spadina_core::realm::CharacterMotion::DirectedEmote {
                start,
                animation,
                direction: player_info.current_direction,
                at: player_info.current_position.clone(),
              });
              start += chrono::Duration::milliseconds(duration.into());
            }
            spadina_core::realm::Action::Interaction { target, interaction } => {
              player_info.action_gate = ActionGate::Interact(target, interaction);
              break;
            }
            spadina_core::realm::Action::Move { mut length } => {
              let mut position = player_info.current_position;
              while length > 0 {
                length -= 1;
                position = match position.neighbour(player_info.current_direction) {
                  Some(p) => p,
                  None => {
                    bad = true;
                    break;
                  }
                };
                if !self.manifold.verify(&position) {
                  bad = true;
                  break;
                }
                let old_pieces: std::collections::BTreeSet<_> = self.manifold.active_proximity(&player_info.current_position).collect();
                let new_pieces: std::collections::BTreeSet<_> = self.manifold.active_proximity(&position).collect();
                player_info.action_gate = if old_pieces == new_pieces {
                  ActionGate::Stop
                } else {
                  ActionGate::Transition {
                    leave: old_pieces.iter().copied().filter(|p| new_pieces.contains(p)).collect(),
                    enter: new_pieces.iter().copied().filter(|p| old_pieces.contains(p)).collect(),
                  }
                };
                let (animation, duration) = self.manifold.animation(&player_info.current_position, &position);

                player_info.committed_movements.push(spadina_core::realm::CharacterMotion::Move {
                  from: player_info.current_position,
                  to: position,
                  start,
                  end: start + duration,
                  animation: animation.clone(),
                });
                start += duration;
                player_info.current_position = position;
              }
              if bad {
                break;
              }
            }
            spadina_core::realm::Action::Rotate { direction } => {
              let end = start + chrono::Duration::milliseconds(navigation::ROTATE_TIME as i64);
              player_info.committed_movements.push(spadina_core::realm::CharacterMotion::Rotate {
                at: player_info.current_position.clone(),
                start,
                end,
                direction,
              });
              player_info.current_direction = direction;
              start = end;
            }
          }
        }
        if bad {
          player_info.remaining_actions.clear();
        }
      }
    }

    if dirty {
      let movements: spadina_core::realm::PlayerStates<crate::shstr::ShStr> = special
        .into_iter()
        .chain(self.active_players.values().map(|player_info| {
          (
            player_info.principal.clone().convert_str(),
            spadina_core::realm::PlayerState {
              effect: player_info
                .state
                .as_ref()
                .map(|s| self.player_effects.get(s))
                .flatten()
                .cloned()
                .unwrap_or(spadina_core::avatar::Effect::Normal),
              final_position: player_info.current_position.clone(),
              final_direction: player_info.current_direction.clone(),
              motion: player_info.committed_movements.iter().map(|m| m.clone().convert_str()).collect(),
            },
          )
        }))
        .collect();
      match output::Multi::convolve(&self.current_states, |state| spadina_core::realm::RealmResponse::UpdateState {
        time: time.clone(),
        state,
        player: movements.clone(),
      }) {
        output::Multi::Single(response) => output.push(crate::destination::DestinationControl::Broadcast(response)),
        output::Multi::Multi(default, special) => output.extend(self.active_players.iter().map(|(key, active_player)| {
          crate::destination::DestinationControl::Response(
            key.clone(),
            active_player.state.as_ref().map(|state| special.get(state)).flatten().unwrap_or(&default).clone(),
          )
        })),
      }
    }
    self.next_update = Box::pin(tokio::time::sleep(
      self
        .pieces
        .iter()
        .flat_map(|piece| piece.next())
        .chain(self.active_players.values().flat_map(|player_info| {
          if player_info.action_gate == ActionGate::Stop {
            player_info.committed_movements.last().map(|m| (now - *m.time()).to_std().ok()).flatten()
          } else {
            Some(std::time::Duration::from_secs(0))
          }
        }))
        .min()
        .unwrap_or(std::time::Duration::from_secs(86400)),
    ));
    output
  }
}

#[async_trait::async_trait]
impl crate::destination::Destination for Realm {
  type Identifier = spadina_core::realm::LocalRealmTarget<std::sync::Arc<str>>;
  type Request = spadina_core::realm::RealmRequest<crate::shstr::ShStr>;
  type Response = spadina_core::realm::RealmResponse<crate::shstr::ShStr>;
  fn capabilities(&self) -> &std::collections::BTreeSet<&'static str> {
    &self.capabilities
  }
  fn delete(&mut self, requester: Option<crate::destination::SharedPlayerId>) -> spadina_core::UpdateResult {
    if requester
      .map(|requester| self.admin_acl.read().check(&requester, &self.server_name) == spadina_core::access::SimpleAccess::Allow)
      .unwrap_or(true)
    {
      match self.database.realm_delete(self.db_id) {
        Err(e) => {
          eprintln!("Failed to delete realm {}: {}", self.db_id, e);
          spadina_core::UpdateResult::InternalError
        }
        Ok(_) => spadina_core::UpdateResult::Success,
      }
    } else {
      spadina_core::UpdateResult::NotAllowed
    }
  }
  fn get_messages(
    &self,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) -> Vec<spadina_core::location::LocationMessage<String>> {
    match self.database.realm_messages(self.db_id, from, to) {
      Ok(messages) => messages,
      Err(e) => {
        eprintln!("Failed to fetch chat message for {}: {}", self.db_id, e);
        Vec::new()
      }
    }
  }
  async fn handle(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &crate::destination::SharedPlayerId,
    is_superuser: bool,
    request: Self::Request,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    match request {
      spadina_core::realm::RealmRequest::AccessGet { target } => {
        let acls = match target {
          spadina_core::realm::RealmAccessTarget::Access => self.access_acl.read(),
          spadina_core::realm::RealmAccessTarget::Admin => self.admin_acl.read(),
        };
        vec![crate::destination::DestinationControl::Response(
          key.clone(),
          spadina_core::realm::RealmResponse::AccessCurrent { target, rules: acls.rules.clone(), default: acls.default.clone() },
        )]
      }
      spadina_core::realm::RealmRequest::AccessSet { id, target, rules, default } => {
        let result = if is_superuser || self.admin_acl.read().check(&player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
          match target {
            spadina_core::realm::RealmAccessTarget::Access => self.access_acl.mutate(|settings| {
              settings.rules = rules;
              settings.default = default;
              spadina_core::UpdateResult::Success
            }),
            spadina_core::realm::RealmAccessTarget::Admin => self.admin_acl.mutate(|settings| {
              settings.rules = rules;
              settings.default = default;
              spadina_core::UpdateResult::Success
            }),
          }
        } else {
          spadina_core::UpdateResult::NotAllowed
        };
        vec![crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::AccessChange { id, result })]
      }
      spadina_core::realm::RealmRequest::AnnouncementAdd { id, announcement } => {
        let result = if is_superuser || self.admin_acl.read().check(player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
          self.announcements.mutate(|announcements| {
            announcements.push(announcement);
            let now = chrono::Utc::now();
            announcements.retain(|a| a.when.expires() < now);
            spadina_core::UpdateResult::Success
          })
        } else {
          spadina_core::UpdateResult::NotAllowed
        };
        let mut output = Vec::new();
        if result == spadina_core::UpdateResult::Success {
          output.push(crate::destination::DestinationControl::Broadcast(spadina_core::realm::RealmResponse::Announcements(Vec::new())));
        }
        output
          .push(crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::AnnouncementUpdate { id, result }));
        output
      }
      spadina_core::realm::RealmRequest::AnnouncementClear { id } => {
        let result = if is_superuser || self.admin_acl.read().check(player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
          self.announcements.mutate(|announcements| {
            announcements.clear();
            spadina_core::UpdateResult::Success
          })
        } else {
          spadina_core::UpdateResult::NotAllowed
        };
        let mut output = Vec::new();
        if result == spadina_core::UpdateResult::Success {
          output.push(crate::destination::DestinationControl::Broadcast(spadina_core::realm::RealmResponse::Announcements(Vec::new())));
        }
        output
          .push(crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::AnnouncementUpdate { id, result }));
        output
      }
      spadina_core::realm::RealmRequest::AnnouncementList => {
        vec![crate::destination::DestinationControl::Response(
          key.clone(),
          spadina_core::realm::RealmResponse::Announcements(self.announcements.read().clone()),
        )]
      }
      spadina_core::realm::RealmRequest::Delete => {
        if self.admin_acl.read().check(player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
          match self.database.realm_delete(self.db_id) {
            Err(e) => {
              eprintln!("Failed to delete realm {}: {}", self.db_id, e);
              Vec::new()
            }
            Ok(_) => {
              vec![crate::destination::DestinationControl::Quit]
            }
          }
        } else {
          Vec::new()
        }
      }
      spadina_core::realm::RealmRequest::ChangeName { id, name, in_directory } => {
        let result = if self.admin_acl.read().check(&player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
          self.name_and_in_directory.mutate(|(n, in_d)| {
            if let Some(name) = name {
              *n = name.to_arc();
            }
            if let Some(in_directory) = in_directory {
              *in_d = in_directory;
            }
            spadina_core::UpdateResult::Success
          })
        } else {
          spadina_core::UpdateResult::NotAllowed
        };
        let mut controls = Vec::new();
        controls.push(crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::NameChange { id, result }));
        if result == spadina_core::UpdateResult::Success {
          let (name, in_directory) = self.name_and_in_directory.read().clone();
          controls.push(crate::destination::DestinationControl::Response(
            key.clone(),
            spadina_core::realm::RealmResponse::NameChanged { name: name.into(), in_directory },
          ));
        }
        controls
      }
      spadina_core::realm::RealmRequest::ChangeSetting { id, name, value } => {
        let mut output = Vec::new();
        let result = if let Some(value) = value.clean() {
          if is_superuser || self.access_acl.read().check(&player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
            self.settings.mutate(|setting| match setting.get_mut(&name) {
              None => spadina_core::UpdateResult::NotAllowed,
              Some(setting) => {
                if setting.type_matched_update(&value) {
                  output.push(crate::destination::DestinationControl::Broadcast(spadina_core::realm::RealmResponse::SettingChanged { name, value }));
                  spadina_core::UpdateResult::Success
                } else {
                  spadina_core::UpdateResult::NotAllowed
                }
              }
            })
          } else {
            spadina_core::UpdateResult::NotAllowed
          }
        } else {
          spadina_core::UpdateResult::NotAllowed
        };
        output.push(crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::SettingChange { id, result }));
        output
      }
      spadina_core::realm::RealmRequest::Kick { id, target } => {
        let target = target.localize(&self.server_name);
        let mut output = Vec::new();
        let result = if is_superuser || self.admin_acl.read().check(&player, &self.owner) == spadina_core::access::SimpleAccess::Allow {
          if let Some(key) = self.player_principals.remove(&target.convert_str()) {
            let state = match self.active_players.remove(&key) {
              Some(active_player) => active_player.state,
              None => None,
            };
            let events: Vec<_> = self
              .pieces
              .iter_mut()
              .enumerate()
              .flat_map(|(sender, piece)| {
                piece.walk(&key, state, crate::realm::navigation::PlayerNavigationEvent::Leave).into_iter().map(move |(name, value)| puzzle::Event {
                  name,
                  value,
                  sender,
                })
              })
              .collect();
            output.extend(self.process_events(events).await);
            output.push(crate::destination::DestinationControl::Move(key.clone(), None));
            true
          } else {
            false
          }
        } else {
          false
        };
        output.push(crate::destination::DestinationControl::Response(key.clone(), spadina_core::realm::RealmResponse::Kick { id, result }));
        output
      }
      spadina_core::realm::RealmRequest::NoOperation => Vec::new(),
      spadina_core::realm::RealmRequest::Perform(actions) => {
        if let Some(info) = self.active_players.get_mut(&key) {
          info.remaining_actions.clear();
          info.remaining_actions.extend(actions.into_iter().map(|a| a.convert_str()));
        }
        self.process_events(vec![]).await
      }
    }
  }
  async fn process_events(
    &mut self,
    events: Vec<puzzle::Event>,
  ) -> Vec<crate::destination::DestinationControl<spadina_core::realm::RealmResponse<crate::shstr::ShStr>>> {
    self.process_events_special(events, Special::Normal).await
  }

  fn quit(&mut self) {}
  async fn remove_player(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    if let Some(active_player) = self.active_players.remove(key) {
      self.player_principals.remove(&player);
      let events: Vec<_> = self
        .pieces
        .iter_mut()
        .enumerate()
        .flat_map(|(sender, piece)| {
          piece
            .walk(key, active_player.state.clone(), crate::realm::navigation::PlayerNavigationEvent::Leave)
            .into_iter()
            .map(move |(name, value)| puzzle::Event { sender, name, value })
        })
        .collect();
      let mut motion: Vec<_> = active_player.committed_movements.into_iter().map(|m| m.convert_str()).collect();
      let start = motion.last().map(|m| m.time().clone()).unwrap_or_else(chrono::Utc::now);
      motion.push(spadina_core::realm::CharacterMotion::Leave { from: active_player.current_position.clone(), start });
      let effect = active_player.state.map(|s| self.player_effects.get(&s)).flatten().cloned().unwrap_or(spadina_core::avatar::Effect::Normal);
      self
        .process_events_special(
          events,
          Special::DeadPlayer(
            active_player.principal.convert_str(),
            spadina_core::realm::PlayerState {
              effect,
              final_direction: active_player.current_direction,
              final_position: active_player.current_position,
              motion,
            },
          ),
        )
        .await
    } else {
      Vec::new()
    }
  }
  async fn send_message(
    &mut self,
    _: Option<&crate::realm::puzzle::PlayerKey>,
    player: &crate::destination::SharedPlayerId,
    body: &spadina_core::communication::MessageBody<
      impl AsRef<str> + serde::Serialize + std::fmt::Debug + std::cmp::PartialEq + std::cmp::Eq + Sync + Into<std::sync::Arc<str>>,
    >,
  ) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now();
    if body.is_transient() {
      Some(now)
    } else {
      match self.database.realm_chat_write(self.db_id, player, body, &now) {
        Ok(()) => Some(now),
        Err(e) => {
          eprintln!("Failed to write chat message for {}: {}", self.db_id, e);
          None
        }
      }
    }
  }
  async fn consensual_emote(
    &mut self,
    requester_key: &crate::realm::puzzle::PlayerKey,
    _: &crate::destination::SharedPlayerId,
    target_key: &crate::realm::puzzle::PlayerKey,
    _: &crate::destination::SharedPlayerId,
    emote: std::sync::Arc<str>,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    if let Some([requester, target]) = self.active_players.get_many_mut([requester_key, target_key]) {
      if requester.remaining_actions.is_empty()
        && requester.action_gate == ActionGate::Stop
        && target.remaining_actions.is_empty()
        && target.action_gate == ActionGate::Stop
        && requester.current_position.is_neighbour(&target.current_position)
      {
        let start = chrono::Utc::now();
        requester.committed_movements.push(spadina_core::realm::CharacterMotion::ConsensualEmoteInitiator {
          start,
          animation: emote.clone(),
          at: requester.current_position,
        });
        target.committed_movements.push(spadina_core::realm::CharacterMotion::ConsensualEmoteRecipient {
          start,
          animation: emote.clone(),
          at: requester.current_position,
        });
        self.process_events_special(Vec::new(), Special::Dirty).await
      } else {
        Vec::new()
      }
    } else {
      Vec::new()
    }
  }
  async fn follow(
    &mut self,
    requester_key: &crate::realm::puzzle::PlayerKey,
    _: &crate::destination::SharedPlayerId,
    target_key: &crate::realm::puzzle::PlayerKey,
    _: &crate::destination::SharedPlayerId,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    if let Some([requester, target]) = self.active_players.get_many_mut([requester_key, target_key]) {
      if requester.remaining_actions.is_empty()
        && requester.action_gate == ActionGate::Stop
        && target.remaining_actions.is_empty()
        && target.action_gate == ActionGate::Stop
        && requester.current_position.is_neighbour(&target.current_position)
      {
        let start = chrono::Utc::now();
        let to = self.manifold.find_adjacent_or_same(&target.current_position);
        requester.committed_movements.push(spadina_core::realm::CharacterMotion::Leave { from: requester.current_position, start });
        requester
          .committed_movements
          .push(spadina_core::realm::CharacterMotion::Enter { to, end: start + chrono::Duration::milliseconds((navigation::WARP_TIME * 2) as i64) });
        let events = self
          .manifold
          .active_proximity(&requester.current_position)
          .map(|sender| (sender, navigation::PlayerNavigationEvent::Leave))
          .chain(self.manifold.active_proximity(&target.current_position).map(|sender| (sender, navigation::PlayerNavigationEvent::Enter)))
          .flat_map(|(sender, event)| {
            self.pieces[sender].walk(&requester_key, requester.state.clone(), event).into_iter().map(move |(name, value)| puzzle::Event {
              sender,
              name,
              value,
            })
          })
          .collect();
        requester.current_position = to;
        self.process_events_special(events, Special::Dirty).await
      } else {
        Vec::new()
      }
    } else {
      Vec::new()
    }
  }
  async fn try_add(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
    is_superuser: bool,
  ) -> Result<(spadina_core::location::LocationResponse<crate::shstr::ShStr>, Vec<crate::destination::DestinationControl<Self::Response>>), ()> {
    if is_superuser || self.access_acl.read().check(player, &self.server_name) == spadina_core::access::SimpleAccess::Allow {
      match self.manifold.warp(None) {
        Some(point) => {
          use rand::Rng;
          self.active_players.insert(
            key.clone(),
            ActivePlayer {
              action_gate: ActionGate::Stop,
              committed_movements: vec![spadina_core::realm::CharacterMotion::Enter { to: point.clone(), end: chrono::Utc::now() }],
              current_direction: rand::thread_rng().gen(),
              current_position: point.clone(),
              principal: player.clone(),
              remaining_actions: Default::default(),
              state: None,
            },
          );
          self.player_principals.insert(player.clone(), key.clone());
          let events: Vec<_> = self
            .manifold
            .active_proximity(&point)
            .flat_map(|sender| {
              self.pieces[sender]
                .walk(key, None, crate::realm::navigation::PlayerNavigationEvent::Enter)
                .into_iter()
                .map(move |(name, value)| puzzle::Event { name, value, sender })
            })
            .collect();
          let output = self.process_events(events).await;
          let (name, in_directory) = self.name_and_in_directory.read().clone();
          Ok((
            spadina_core::location::LocationResponse::Realm {
              owner: self.owner.clone().into(),
              server: self.server_name.clone().into(),
              name: name.into(),
              asset: self.asset.clone().into(),
              in_directory,
              seed: self.seed,
              settings: self.settings.read().clone(),
            },
            output,
          ))
        }
        None => Err(()),
      }
    } else {
      Err(())
    }
  }
}
impl futures::Stream for Realm {
  type Item = Vec<puzzle::Event>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    use std::future::Future;
    let mut project = self.project();
    match project.next_update.as_mut().poll(cx) {
      std::task::Poll::Pending => std::task::Poll::Pending,
      std::task::Poll::Ready(()) => {
        let now = chrono::Utc::now();
        let mut events: Vec<_> = project
          .pieces
          .iter_mut()
          .enumerate()
          .flat_map(|(sender, piece)| piece.tick(&now).into_iter().map(move |(name, value)| puzzle::Event { name, value, sender }))
          .collect();
        for (player, player_info) in project.active_players.iter_mut() {
          player_info.committed_movements.retain(|m| m.time() > &now);
          if player_info.committed_movements.is_empty() {
            let mut action_gate = ActionGate::Stop;
            std::mem::swap(&mut player_info.action_gate, &mut action_gate);
            match action_gate {
              ActionGate::Enter(pieces) => {
                events.extend(pieces.iter().flat_map(|&sender| {
                  project.pieces[sender]
                    .walk(player, player_info.state.clone(), navigation::PlayerNavigationEvent::Enter)
                    .into_iter()
                    .map(move |(name, value)| puzzle::Event { name, value, sender })
                }));
              }
              ActionGate::Interact(target, interaction) => {
                if let Some(at) = player_info.current_position.neighbour(player_info.current_direction) {
                  let (animation, duration) = project
                    .manifold
                    .interaction_animation(&at, &target)
                    .unwrap_or((spadina_core::realm::CharacterAnimation::Touch, chrono::Duration::milliseconds(navigation::TOUCH_TIME as i64)));
                  player_info.committed_movements.push(spadina_core::realm::CharacterMotion::Interaction {
                    start: now,
                    end: now + duration,
                    animation,
                    at,
                  });
                  events.extend(project.manifold.interaction_target(&player_info.current_position, &target).into_iter().flat_map(|sender| {
                    let (result, output) = project.pieces[sender].interact(&interaction.clone().convert_str(), player_info.state.clone());
                    match result {
                      spadina_core::realm::InteractionResult::Invalid => {
                        eprintln!(
                          "Failed interaction {:?} at {:?} with {:?} in {}",
                          interaction, &player_info.current_position, target, project.db_id
                        );
                      }
                      spadina_core::realm::InteractionResult::Failed => {
                        player_info.remaining_actions.clear();
                      }
                      spadina_core::realm::InteractionResult::Accepted => (),
                    }

                    output.into_iter().map(move |(name, value)| puzzle::Event { name, value, sender })
                  }));
                }
              }

              ActionGate::Stop => (),
              ActionGate::Transition { enter, leave } => {
                events.extend(leave.iter().flat_map(|&sender| {
                  project.pieces[sender]
                    .walk(&player, player_info.state.clone(), navigation::PlayerNavigationEvent::Leave)
                    .into_iter()
                    .map(move |(name, value)| puzzle::Event { name, value, sender })
                }));
                events.extend(enter.iter().flat_map(|&sender| {
                  project.pieces[sender]
                    .walk(&player, player_info.state.clone(), navigation::PlayerNavigationEvent::Enter)
                    .into_iter()
                    .map(move |(name, value)| puzzle::Event { name, value, sender })
                }));
              }
            };
          }
        }
        std::task::Poll::Ready(Some(events))
      }
    }
  }
}

impl Special {
  fn is_dirty(&self) -> bool {
    match self {
      Special::Normal => false,
      _ => true,
    }
  }
}
impl std::iter::IntoIterator for Special {
  type Item = (spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>, spadina_core::realm::PlayerState<crate::shstr::ShStr>);

  type IntoIter =
    std::option::IntoIter<(spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>, spadina_core::realm::PlayerState<crate::shstr::ShStr>)>;

  fn into_iter(self) -> Self::IntoIter {
    match self {
      Special::Normal => None,
      Special::Dirty => None,
      Special::DeadPlayer(player, state) => Some((player, state)),
    }
    .into_iter()
  }
}
