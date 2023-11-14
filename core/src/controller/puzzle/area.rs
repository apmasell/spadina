pub trait AreaCounter {
  fn count_players(&self, filter: &PlayerStateCondition) -> u32;
}
pub struct CountCollection<C>(pub C);
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum PlayerStateCondition {
  All(Vec<PlayerStateCondition>),
  Always,
  Any(Vec<PlayerStateCondition>),
  AnyBits { offset: u8, length: u8 },
  HasBit(u8),
  NoBits { offset: u8, length: u8 },
  NotHasBit(u8),
}

impl PlayerStateCondition {
  pub fn check(&self, value: u32) -> bool {
    match self {
      PlayerStateCondition::All(conditions) => conditions.iter().all(|c| c.check(value)),
      PlayerStateCondition::Always => true,
      PlayerStateCondition::Any(conditions) => conditions.iter().any(|c| c.check(value)),
      PlayerStateCondition::AnyBits { offset, length } => {
        1_u32.checked_shl(*length as u32).map(|v| (v - 1).checked_shl(*offset as u32)).flatten().map(|v| value & v != 0).unwrap_or(false)
      }
      PlayerStateCondition::HasBit(b) => 1_u32.checked_shl(*b as u32).map(|v| value & v != 0).unwrap_or(false),
      PlayerStateCondition::NoBits { offset, length } => {
        1_u32.checked_shl(*length as u32).map(|v| (v - 1).checked_shl(*offset as u32)).flatten().map(|v| value & v == 0).unwrap_or(true)
      }
      PlayerStateCondition::NotHasBit(b) => 1_u32.checked_shl(*b as u32).map(|v| value & v == 0).unwrap_or(true),
    }
  }
}
impl<C> AreaCounter for CountCollection<C>
where
  for<'a> &'a C: IntoIterator<Item = &'a u32>,
{
  fn count_players(&self, filter: &PlayerStateCondition) -> u32 {
    self.0.into_iter().filter(|&v| filter.check(*v)).count() as u32
  }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum PlayerStateModification {
  ClearAll,
  ClearBit(u8),
  SetAll,
  SetBit(u8),
  Set { offset: u8, length: u8, value: u8 },
}
