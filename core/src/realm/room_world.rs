use crate::asset::extraction::ExtractChildren;
use crate::controller::puzzle::state_machine::StateMachinePuzzleTemplate;
use crate::realm::RealmSettings;
use crate::scene::value::{GlobalValue, LocalDiscreteValue};
use crate::scene::Color;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

#[derive(Clone, Serialize, Deserialize)]
pub struct Room<Area, InputIdentifier, OutputIdentifier, Setting> {
  pub size: (u16, u16),
  pub background: GlobalValue<Color, OutputIdentifier, Setting>,
  pub tiles: BTreeMap<(u16, u16), Tile<Area, InputIdentifier, OutputIdentifier, Setting>>,
  pub edge: BTreeMap<Edge, (u8, Edge)>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Tile<Area, InputIdentifier, OutputIdentifier, Setting> {
  Solid(LocalDiscreteValue<Color, OutputIdentifier, Setting>),
  Gated(LocalDiscreteValue<Color, OutputIdentifier, Setting>, OutputIdentifier),
  Input(LocalDiscreteValue<Color, OutputIdentifier, Setting>, InputIdentifier),
  Area(LocalDiscreteValue<Color, OutputIdentifier, Setting>, Area),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Edge {
  X0(u16),
  Y0(u16),
  XM(u16),
  YM(u16),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct World<Area, InputIdentifier, OutputIdentifier, Setting: AsRef<str> + Ord> {
  pub rooms: Vec<Room<Area, InputIdentifier, OutputIdentifier, Setting>>,
  pub puzzle: StateMachinePuzzleTemplate<InputIdentifier, OutputIdentifier, Area>,
  pub settings: RealmSettings<Setting>,
}

impl Edge {
  pub fn in_bounds(&self, size: &(u16, u16)) -> bool {
    match self {
      &Edge::X0(v) | &Edge::XM(v) => v < size.1,
      &Edge::Y0(v) | &Edge::YM(v) => v < size.0,
    }
  }
}

impl<S: AsRef<str> + Ord, Area, InputIdentifier, OutputIdentifier, Setting: AsRef<str> + Ord> ExtractChildren<S>
  for World<Area, InputIdentifier, OutputIdentifier, Setting>
{
  fn extract_children(&self, _assets: &mut BTreeSet<S>) {}
}
impl<Area: Ord + Display, InputIdentifier: Ord + Display, OutputIdentifier: Ord + Display, Setting: Display + Ord + AsRef<str>>
  World<Area, InputIdentifier, OutputIdentifier, Setting>
{
  pub fn validate(&self) -> Result<(), Cow<'static, str>> {
    let mut areas = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    let mut settings = BTreeSet::new();
    for (index, room) in self.rooms.iter().enumerate() {
      room.background.validate(&mut outputs, &mut settings)?;
      for (edge, (target, target_edge)) in &room.edge {
        if !edge.in_bounds(&room.size) {
          return Err(Cow::Owned(format!("Room {} has edge {:?}, but source is out of bounds ({}, {}).", index, edge, room.size.0, room.size.1)));
        }
        let Some(target_room) = self.rooms.get(*target as usize) else {
          return Err(Cow::Owned(format!("Room {} has edge that goes to room {}, which is not present (max {}).", index, target, self.rooms.len())));
        };
        if !target_edge.in_bounds(&target_room.size) {
          return Err(Cow::Owned(format!(
            "Room {} has edge that goes to room {}, but target ({:?}) is out of bounds ({}, {}).",
            index, target, target_edge, target_room.size.0, target_room.size.1
          )));
        }
        for ((x, y), tile) in &room.tiles {
          if *x > room.size.0 || *y > room.size.1 {
            return Err(Cow::Owned(format!("Room {} has tile at ({}, {}) which is out of bounds ({}, {})", index, x, y, room.size.0, room.size.1)));
          }
          match tile {
            Tile::Solid(c) => c.validate(&mut outputs, &mut settings)?,
            Tile::Gated(c, o) => {
              c.validate(&mut outputs, &mut settings)?;
              outputs.insert(o);
            }
            Tile::Input(c, i) => {
              c.validate(&mut outputs, &mut settings)?;
              inputs.insert(i);
            }
            Tile::Area(c, a) => {
              c.validate(&mut outputs, &mut settings)?;
              areas.insert(a);
            }
          }
        }
      }
    }
    self.puzzle.validate(areas, inputs, outputs)?;
    for &setting in &settings {
      if !self.settings.contains_key(setting) {
        return Err(Cow::Owned(format!("Setting {} is used but not defined", setting.as_ref())));
      }
    }
    Ok(())
  }
}
