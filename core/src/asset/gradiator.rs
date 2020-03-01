use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Current<T, N, B> {
  Altitude { top_value: T, bottom_value: T, top_altitude: u32, bottom_altitude: u32 },
  BoolControlled { when_true: T, when_false: T, value: B, transition: super::Transition },
  Fixed(T),
  NumControlled { default_value: T, values: Vec<T>, value: N, transition: super::Transition },
  Setting(String),
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
pub struct Gradiator<T> {
  pub sources: Vec<Source<T, String, String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source<T, N, B> {
  pub source: Current<T, N, B>,
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
impl<T, N, B> Current<T, N, B> {
  async fn map<R, E, F: core::future::Future<Output = Result<R, E>>, M: Fn(T) -> F + Send>(self, mapper: &M) -> Result<Current<R, N, B>, E> {
    Ok(match self {
      Current::Altitude { top_value, bottom_value, top_altitude, bottom_altitude } => {
        Current::Altitude { top_value: mapper(top_value).await?, bottom_value: mapper(bottom_value).await?, top_altitude, bottom_altitude }
      }
      Current::BoolControlled { when_true, when_false, value, transition } => {
        Current::BoolControlled { when_true: mapper(when_true).await?, when_false: mapper(when_false).await?, value, transition }
      }
      Current::Fixed(v) => Current::Fixed(mapper(v).await?),
      Current::NumControlled { default_value, values, value, transition } => Current::NumControlled {
        default_value: mapper(default_value).await?,
        values: futures::stream::iter(values.into_iter()).map(Ok).and_then(|v| mapper(v)).try_collect().await?,
        value,
        transition,
      },
      Current::Setting(s) => Current::Setting(s),
    })
  }
  fn resolve<R: Resolver<N, B>>(self, resolver: &mut R) -> Current<T, R::Num, R::Bool> {
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
impl<T> Gradiator<T> {
  pub async fn map<R, E, F: core::future::Future<Output = Result<R, E>>, M: Fn(T) -> F + Send>(self, mapper: &M) -> Result<Gradiator<R>, E> {
    Ok(Gradiator { sources: futures::stream::iter(self.sources.into_iter()).map(Ok).and_then(|s| s.map(mapper)).try_collect().await? })
  }
  pub fn resolve<R: Resolver<String, String>>(self, mapper: &mut R) -> Vec<Source<T, R::Num, R::Bool>> {
    self.sources.into_iter().map(|s| s.resolve(mapper)).collect()
  }
}

impl<T, N, B> Source<T, N, B> {
  pub async fn map<R, E, F: core::future::Future<Output = Result<R, E>>, M: Fn(T) -> F + Send>(self, mapper: &M) -> Result<Source<R, N, B>, E> {
    Ok(Source { source: self.source.map(mapper).await?, function: self.function })
  }
  pub fn resolve<R: Resolver<N, B>>(self, resolver: &mut R) -> Source<T, R::Num, R::Bool> {
    Source { source: self.source.resolve(resolver), function: self.function }
  }
}

fn abs_difference<T: std::ops::Sub<Output = T> + Ord>(x: T, y: T) -> T {
  if x < y {
    y - x
  } else {
    x - y
  }
}
