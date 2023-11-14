use crate::controller::puzzle::area::{PlayerStateCondition, PlayerStateModification};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum Action<Area, Setting> {
  Link { from: Area, matches: PlayerStateCondition, target: LinkTarget<Setting> },
  Move { from: Area, to: Area, matches: PlayerStateCondition },
  Mark { location: Area, matches: PlayerStateCondition, modification: PlayerStateModification },
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum LinkTarget<Setting> {
  Next,
  Home,
  Setting(Setting),
}
