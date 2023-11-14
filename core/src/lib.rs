use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

pub mod access;
pub mod asset;
pub mod asset_store;
pub mod atomic_clock;
pub mod avatar;
pub mod capabilities;
pub mod communication;
pub mod controller;
pub mod location;
pub mod net;
pub mod player;
pub mod realm;
pub mod reference_converter;
pub mod resource;
pub mod scene;
pub mod shared_ref;
pub mod tracking_map;
pub mod user_interface;

/// The result of an update operation that requires permissions
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum UpdateResult {
  InternalError,
  NotAllowed,
  Redundant,
  Success,
}

pub fn abs_difference<T: std::ops::Sub<Output = T> + Ord>(x: T, y: T) -> T {
  if x < y {
    y - x
  } else {
    x - y
  }
}

impl UpdateResult {
  pub fn description(&self) -> &'static str {
    match self {
      UpdateResult::InternalError => "internal error",
      UpdateResult::NotAllowed => "not allowed",
      UpdateResult::Redundant => "already done",
      UpdateResult::Success => "success",
    }
  }
}

impl Display for UpdateResult {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.description())
  }
}
