use serde::{Deserialize, Serialize};

pub mod angle;
pub mod gradiator;
pub mod value;

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum Color {
  Rgb(u8, u8, u8),
  Hsl(u8, u8, u8),
}
