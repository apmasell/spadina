use crate::abs_difference;
use crate::scene::value::Transition;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Current<T, N, B, S> {
  Altitude { top_value: T, bottom_value: T, top_altitude: u32, bottom_altitude: u32 },
  BoolControlled { when_true: T, when_false: T, value: B, transition: Transition },
  Fixed(T),
  NumControlled { default_value: T, values: Vec<T>, value: N, transition: Transition },
  Setting(S),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Decay {
  Constant,
  Euclidean,
  Inverted(Box<Decay>),
  Linear,
  Shell,
  Step(f64, Box<Decay>),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Function {
  PointSource { x: u32, y: u32, z: u32, decay: Decay },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Gradiator<T, S: AsRef<str>> {
  pub sources: Vec<Source<T, S, S, S>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source<T, N, B, S> {
  pub source: Current<T, N, B, S>,
  pub function: Function,
}
impl Decay {
  pub fn compute(&self, delta_x: f64, delta_y: f64, delta_z: f64) -> f64 {
    match self {
      Decay::Constant => 1.0,
      Decay::Euclidean => (delta_x * delta_x + delta_y * delta_y + delta_z * delta_z).sqrt(),
      Decay::Inverted(original) => 1.0 - original.compute(delta_x, delta_y, delta_z),
      Decay::Linear => delta_x * delta_x + delta_y * delta_y + delta_z * delta_z,
      Decay::Shell => (delta_x * delta_x + delta_y * delta_y + delta_z * delta_z).log(1.0 / 3.0),
      Decay::Step(cutoff, original) => {
        let v = original.compute(delta_x, delta_y, delta_z);
        if v < *cutoff {
          0.0
        } else {
          v
        }
      }
    }
  }
}
pub trait Resolver<N, B> {
  type Bool;
  type Num;
  fn resolve_bool(&mut self, value: B) -> Self::Bool;
  fn resolve_num(&mut self, value: N, len: usize) -> Self::Num;
}
impl<T, N, B, S> Current<T, N, B, S> {
  fn map<R, E, F: FnMut(T) -> Result<R, E>>(self, mut mapper: F) -> Result<Current<R, N, B, S>, E> {
    Ok(match self {
      Current::Altitude { top_value, bottom_value, top_altitude, bottom_altitude } => {
        Current::Altitude { top_value: mapper(top_value)?, bottom_value: mapper(bottom_value)?, top_altitude, bottom_altitude }
      }
      Current::BoolControlled { when_true, when_false, value, transition } => {
        Current::BoolControlled { when_true: mapper(when_true)?, when_false: mapper(when_false)?, value, transition }
      }
      Current::Fixed(v) => Current::Fixed(mapper(v)?),
      Current::NumControlled { default_value, values, value, transition } => {
        let mut new_values = Vec::new();
        for v in values {
          new_values.push(mapper(v)?);
        }

        Current::NumControlled { default_value: mapper(default_value)?, values: new_values, value, transition }
      }
      Current::Setting(s) => Current::Setting(s),
    })
  }
  fn resolve<R: Resolver<N, B>>(self, resolver: &mut R) -> Current<T, R::Num, R::Bool, S> {
    match self {
      Current::Altitude { top_value, bottom_value, top_altitude, bottom_altitude } => {
        Current::Altitude { top_value, bottom_value, top_altitude, bottom_altitude }
      }
      Current::BoolControlled { when_true, when_false, value, transition } => {
        Current::BoolControlled { when_true, when_false, value: resolver.resolve_bool(value), transition }
      }
      Current::Fixed(v) => Current::Fixed(v),
      Current::NumControlled { default_value, values, value, transition } => {
        Current::NumControlled { default_value, value: resolver.resolve_num(value, values.len()), values, transition }
      }
      Current::Setting(s) => Current::Setting(s),
    }
  }
}
impl Function {
  pub fn distance(&self, ix: u32, iy: u32, iz: u32) -> f64 {
    match self {
      Function::PointSource { x, y, z, decay } => {
        decay.compute(abs_difference(*x, ix) as f64, abs_difference(*y, iy) as f64, abs_difference(*z, iz) as f64)
      }
    }
  }
}
impl<A, S: AsRef<str>> Gradiator<A, S> {
  pub fn map<R, E, F: FnMut(A) -> Result<R, E>>(self, mut mapper: F) -> Result<Gradiator<R, S>, E> {
    let mut sources = Vec::new();
    for source in self.sources {
      sources.push(source.map(&mut mapper)?);
    }
    Ok(Gradiator { sources })
  }
  pub fn resolve<R: Resolver<S, S>>(self, mapper: &mut R) -> Vec<Source<A, R::Num, R::Bool, S>> {
    self.sources.into_iter().map(|s| s.resolve(mapper)).collect()
  }
}

impl<A, N, B, S> Source<A, N, B, S> {
  pub fn map<R, E, F: FnMut(A) -> Result<R, E>>(self, mapper: F) -> Result<Source<R, N, B, S>, E> {
    Ok(Source { source: self.source.map(mapper)?, function: self.function })
  }
  pub fn resolve<R: Resolver<N, B>>(self, resolver: &mut R) -> Source<A, R::Num, R::Bool, S> {
    Source { source: self.source.resolve(resolver), function: self.function }
  }
}
