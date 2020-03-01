/// This is the time in ms it takes to walk one tile
pub(crate) const STEP_TIME: u32 = 500;
// The time in ms it takes to warp in/out
pub(crate) const WARP_TIME: u32 = 800;
// The time in ms it takes to touch/interact with an item
pub(crate) const TOUCH_TIME: u32 = 1000;
pub(crate) const ROTATE_TIME: u32 = 300;

/// A connection between two grids where players can navigate
#[derive(Clone)]
pub struct Bridge {
  target: spadina_core::realm::Point,
  condition: BridgeCondition,
}

/// The rule to deicde the transition between two grids
#[derive(Clone)]
pub enum BridgeCondition {
  /// The player can always walk with the animation and time penalty given
  Static(spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>, u32),
  /// The player can only walk when the puzzle has permitted it. If they can, they use the animation and time penalty given
  PuzzleGated(std::sync::Arc<std::sync::atomic::AtomicBool>, spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>, u32),
}

/// What player interaction is available at a location on the ground
#[derive(Clone)]
pub struct InteractionInformation {
  /// The piece ID
  pub piece: usize,
  /// The animation that should be shown when interaction is occuring
  pub animation: spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>,
  /// The duration of the animation
  pub duration: u32,
}

/// The spaces where a player could move
#[derive(Clone)]
pub enum Ground {
  Obstacle,
  GatedObstacle(std::sync::Arc<std::sync::atomic::AtomicBool>),
  Pieces {
    interaction: std::collections::HashMap<spadina_core::realm::InteractionKey<std::sync::Arc<str>>, InteractionInformation>,
    proximity: Vec<usize>,
  },
  Connection(Vec<Bridge>),
}

/// A grid of possible player locations
pub struct Platform {
  pub width: u32,
  pub length: u32,
  pub terrain: std::collections::BTreeMap<(u32, u32), Ground>,
  pub animation: spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>,
}

#[derive(Clone)]
pub enum PlayerNavigationEvent {
  Enter,
  Leave,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnArea {
  pub platform: usize,
  pub x1: u32,
  pub y1: u32,
  pub x2: u32,
  pub y2: u32,
}

pub struct RealmManifold {
  pub platforms: Vec<Platform>,
  pub default_spawn: SpawnArea,
  pub spawn_points: std::collections::HashMap<std::sync::Arc<str>, SpawnArea>,
}

impl RealmManifold {
  /// Determine if there are any puzzle pieces buried in at a particular location that the player can interact with
  pub fn active_proximity(&self, position: &spadina_core::realm::Point) -> impl std::iter::Iterator<Item = usize> + '_ {
    (match self.platforms.get(position.platform as usize).map(|platform| platform.terrain.get(&(position.x, position.y))).flatten() {
      Some(Ground::Pieces { proximity, .. }) => Some(proximity.iter().copied()),
      _ => None,
    })
    .into_iter()
    .flatten()
  }
  /// Determine the animation that should be used as a player navigates between two points
  pub fn animation(
    &self,
    source: &spadina_core::realm::Point,
    target: &spadina_core::realm::Point,
  ) -> (&spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>, chrono::Duration) {
    if source.platform == target.platform {
      (
        self.platforms.get(target.platform as usize).map(|s| &s.animation).unwrap_or(&spadina_core::realm::CharacterAnimation::Walk),
        chrono::Duration::milliseconds(STEP_TIME.into()),
      )
    } else {
      self
        .platforms
        .get(source.platform as usize)
        .map(|p| {
          p.terrain.get(&(source.x, source.y)).map(|g| match g {
            Ground::Connection(connections) => connections
              .iter()
              .filter(|ts| ts.target.platform == target.platform)
              .map(|ts| match &ts.condition {
                BridgeCondition::Static(animation, time) => (animation, chrono::Duration::milliseconds(*time as i64)),
                BridgeCondition::PuzzleGated(gate, animation, time) => {
                  if gate.load(std::sync::atomic::Ordering::Relaxed) {
                    (animation, chrono::Duration::milliseconds(*time as i64))
                  } else {
                    (&spadina_core::realm::CharacterAnimation::Confused, chrono::Duration::milliseconds(700))
                  }
                }
              })
              .next(),
            _ => None,
          })
        })
        .flatten()
        .flatten()
        .unwrap_or((&spadina_core::realm::CharacterAnimation::Walk, chrono::Duration::milliseconds(STEP_TIME.into())))
    }
  }
  pub fn find_adjacent_or_same(&self, target: &spadina_core::realm::Point) -> spadina_core::realm::Point {
    if let Some(platform) = self.platforms.get(target.platform as usize) {
      let mut possibilites = Vec::new();
      if target.x > 0 {
        possibilites.push(spadina_core::realm::Point { x: target.x - 1, y: target.y, platform: target.platform });
        if target.y > 0 {
          possibilites.push(spadina_core::realm::Point { x: target.x - 1, y: target.y - 1, platform: target.platform });
        }
        if target.y < platform.length - 1 {
          possibilites.push(spadina_core::realm::Point { x: target.x - 1, y: target.y + 1, platform: target.platform });
        }
      }

      if target.x < platform.width - 1 {
        possibilites.push(spadina_core::realm::Point { x: target.x + 1, y: target.y, platform: target.platform });
        if target.y > 0 {
          possibilites.push(spadina_core::realm::Point { x: target.x + 1, y: target.y - 1, platform: target.platform });
        }
        if target.y < platform.length - 1 {
          possibilites.push(spadina_core::realm::Point { x: target.x + 1, y: target.y + 1, platform: target.platform });
        }
      }
      if target.y > 0 {
        possibilites.push(spadina_core::realm::Point { x: target.x, y: target.y - 1, platform: target.platform });
      }
      if target.y < platform.length - 1 {
        possibilites.push(spadina_core::realm::Point { x: target.x, y: target.y + 1, platform: target.platform });
      }
      possibilites.retain(|p| {
        platform
          .terrain
          .get(&(p.x, p.y))
          .map(|g| match g {
            Ground::Obstacle => false,
            Ground::GatedObstacle(gate) => gate.load(std::sync::atomic::Ordering::Relaxed),
            Ground::Pieces { .. } => true,
            Ground::Connection(_) => true,
          })
          .unwrap_or(true)
      });
      use rand::seq::SliceRandom;
      possibilites.shuffle(&mut rand::thread_rng());
      possibilites.into_iter().next().unwrap_or(target.clone())
    } else {
      target.clone()
    }
  }
  /// Determine what animation should be used to indicate a player is interacting with an item at this location
  pub fn interaction_animation(
    &self,
    target: &spadina_core::realm::Point,
    key: &spadina_core::realm::InteractionKey<std::sync::Arc<str>>,
  ) -> Option<(spadina_core::realm::CharacterAnimation<std::sync::Arc<str>>, chrono::Duration)> {
    self
      .platforms
      .get(target.platform as usize)
      .map(|s| s.terrain.get(&(target.x, target.y)))
      .flatten()
      .map(|ground| match ground {
        Ground::Pieces { interaction, .. } => {
          interaction.get(key).map(|info| (info.animation.clone(), chrono::Duration::milliseconds(info.duration.into())))
        }
        _ => None,
      })
      .flatten()
  }
  /// Determine if there is a puzzle piece to interact with at the specified position
  pub fn interaction_target(
    &self,
    target: &spadina_core::realm::Point,
    key: &spadina_core::realm::InteractionKey<std::sync::Arc<str>>,
  ) -> Option<usize> {
    self
      .platforms
      .get(target.platform as usize)
      .map(|s| s.terrain.get(&(target.x, target.y)))
      .flatten()
      .map(|ground| match ground {
        Ground::Pieces { interaction, .. } => interaction.get(key).map(|info| info.piece),
        _ => None,
      })
      .flatten()
  }
  /// Determine if a player can occupy this position
  pub fn verify(&self, position: &spadina_core::realm::Point) -> bool {
    self
      .platforms
      .get(position.platform as usize)
      .map(|platform| {
        position.y < platform.length
          && position.x < platform.width
          && match platform.terrain.get(&(position.x, position.y)) {
            Some(Ground::Obstacle) => false,
            Some(Ground::GatedObstacle(gate)) => gate.load(std::sync::atomic::Ordering::Relaxed),
            Some(_) => true,
            None => false,
          }
      })
      .unwrap_or(false)
  }
  /// Determine if a player can cross between two grids in the current state
  pub fn verify_join(&self, start: &spadina_core::realm::Point, end: &spadina_core::realm::Point, skip_gate_check: bool) -> bool {
    if start.platform == end.platform {
      true
    } else {
      self
        .platforms
        .get(start.platform as usize)
        .map(|start_platform| {
          start_platform.terrain.get(&(start.x, start.y)).map(|ground| match ground {
            Ground::Connection(connections) => connections.iter().any(|bridge| {
              bridge.target.platform == end.platform
                && bridge.target.x == end.x
                && bridge.target.y == end.y
                && (skip_gate_check
                  || match &bridge.condition {
                    BridgeCondition::Static(_, _) => true,
                    BridgeCondition::PuzzleGated(gate, _, _) => gate.load(std::sync::atomic::Ordering::Relaxed),
                  })
            }),
            _ => false,
          })
        })
        .flatten()
        .unwrap_or(false)
    }
  }
  /// Determine a point at which a player will materialise for a given spawn point
  pub fn warp(&mut self, name: Option<&str>) -> Option<spadina_core::realm::Point> {
    let point = match name {
      Some(n) => self.spawn_points.get(n)?,
      None => &self.default_spawn,
    };
    match self.platforms.get_mut(point.platform) {
      Some(platform) => {
        use rand::seq::SliceRandom;
        let mut targets: Vec<_> = Vec::new();
        for x in point.x1..point.x2 {
          targets.extend(
            (point.y1..point.y2)
              .filter(|&y| match platform.terrain.get(&(x, y)) {
                Some(Ground::Obstacle) => false,
                Some(Ground::GatedObstacle(gate)) => gate.load(std::sync::atomic::Ordering::Relaxed),
                _ => true,
              })
              .map(move |y| (x, y)),
          );
        }

        targets.shuffle(&mut rand::thread_rng());
        targets.get(0).map(|&(x, y)| spadina_core::realm::Point { platform: point.platform as u32, x, y })
      }
      None => None,
    }
  }
}
