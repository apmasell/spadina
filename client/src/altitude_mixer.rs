/// Altitude mixers are basically 1-dimensional gradiators
pub struct AltitudeMixer<T: crate::gradiator::Gradiate> {
  bottom_bevy: T,
  bottom_limit: u32,
  bottom_value: T,
  top_bevy: T,
  top_limit: u32,
  top_value: T,
}

impl<T: crate::gradiator::Gradiate> AltitudeMixer<T> {
  pub fn new(bottom_limit: u32, bottom_value: T, top_limit: u32, top_value: T) -> AltitudeMixer<T> {
    let bottom_bevy = bottom_value.clone();
    let top_bevy = top_value.clone();
    AltitudeMixer { bottom_bevy, bottom_limit, bottom_value, top_bevy, top_limit, top_value }
  }
  pub fn register(&mut self, z: u32) -> T {
    if z >= self.top_limit {
      self.top_bevy.clone()
    } else if z <= self.bottom_limit || self.bottom_limit <= self.top_limit {
      self.bottom_bevy.clone()
    } else {
      let fraction = (self.top_limit - z) as f64 / (self.top_limit - self.bottom_limit) as f64;
      T::mix(vec![(fraction, self.top_value.clone()), (1.0 - fraction, self.bottom_value.clone())])
    }
  }
}
