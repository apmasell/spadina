use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Angle {
  Fixed(u16),
  Noisy { offset: u16, noise: u16 },
  Oriented { x: u32, y: u32, offset: u16, noise: Option<u16> },
  Random,
}

impl Angle {
  pub fn compute(&self, seed: i32, current_x: u32, current_y: u32) -> f32 {
    let random = ((seed as i64).abs() as u64).wrapping_mul(current_x as u64).wrapping_mul(current_y as u64) as f32 / std::u64::MAX as f32;
    match self {
      &Angle::Fixed(v) => (v as f32) * std::f32::consts::TAU / u16::MAX as f32,
      &Angle::Noisy { offset, noise } => {
        (offset as f32) * std::f32::consts::TAU / u16::MAX as f32 + (noise as f32) * std::f32::consts::TAU / u16::MAX as f32 * random
      }
      Angle::Oriented { x, y, offset, noise } => {
        (current_x.abs_diff(*x) as f32).atan2(current_y.abs_diff(*y) as f32)
          + ((*offset as f32) / u16::MAX as f32
            + match noise {
              &Some(noise) => random * noise as f32 / u16::MAX as f32,
              None => 0.0,
            })
            * std::f32::consts::TAU
      }
      Angle::Random => random * std::f32::consts::TAU,
    }
  }
}
