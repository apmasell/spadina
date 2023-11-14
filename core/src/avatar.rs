use crate::scene::Color;
use serde::Deserialize;
use serde::Serialize;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Avatar {
  timezone: chrono_tz::Tz,
  color: Color,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Effect {
  Normal,
  Sparkling(Color),
  Tinted(Color),
}

impl Avatar {
  pub fn default_for(name: &str) -> Self {
    let mut hasher = std::hash::DefaultHasher::new();
    name.hash(&mut hasher);
    Self { timezone: chrono_tz::Tz::UTC, color: Color::Hsl((hasher.finish() % 0xFF) as u8, 0xCC, 0x99) }
  }
}
