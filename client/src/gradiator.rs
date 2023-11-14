use spadina_core::scene::gradiator::{Current, Resolver, Source};
use std::borrow::Cow;
use std::collections::{btree_map, BTreeMap};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

pub struct BoolUpdateState(Arc<AtomicBool>, bool);
pub struct NumUpdateState(Arc<AtomicU32>, u32);

pub struct Gradiator<Value, Update> {
  sources: Vec<Source<Value, NumUpdateState, BoolUpdateState, String>>,
  points: BTreeMap<(u32, u32, u32), Update>,
}
pub trait Gradiate: Sized + Clone {
  fn mix<'a>(values: impl IntoIterator<Item = (f64, Self)>) -> Self;
}
pub trait Change<Value>: Default {
  type World;
  fn change(&self, value: Value, world: &mut Self::World);
}

impl Default for BoolUpdateState {
  fn default() -> Self {
    Self(Default::default(), Default::default())
  }
}
impl Default for NumUpdateState {
  fn default() -> Self {
    Self(Default::default(), Default::default())
  }
}

impl<Value: Gradiate, Update: Change<Value>> Gradiator<Value, Update> {
  pub fn update(&mut self, world: &mut Update::World) {
    let mut update = false;
    for source in self.sources.iter_mut() {
      if Self::check_dirty(&mut source.source) {
        update = true;
      }
    }
    if update {
      for (&(x, y, z), target) in self.points.iter() {
        let value = Value::mix(self.sources.iter().map(|s| (s.function.distance(x, y, z), Self::value(&s.source, z))));
        target.change(value, world);
      }
    }
  }
  pub fn register(&mut self, x: u32, y: u32, z: u32) -> &mut Update {
    self.points.entry((x, y, z)).or_default()
  }
  fn check_dirty(current: &mut Current<Value, NumUpdateState, BoolUpdateState, String>) -> bool {
    match current {
      Current::Altitude { .. } => false,
      Current::BoolControlled { value, .. } => {
        let current = value.0.load(std::sync::atomic::Ordering::Relaxed);
        if current == value.1 {
          false
        } else {
          value.1 = current;
          true
        }
      }
      Current::Fixed(_) => false,
      Current::NumControlled { value, .. } => {
        let current = value.0.load(std::sync::atomic::Ordering::Relaxed);
        if current == value.1 {
          false
        } else {
          value.1 = current;
          true
        }
      }
      Current::Setting(_) => todo!(),
    }
  }
  fn value(current: &Current<Value, NumUpdateState, BoolUpdateState, String>, z: u32) -> Value {
    match current {
      Current::Altitude { top_value, top_altitude, bottom_value, bottom_altitude } => {
        if z <= *bottom_altitude {
          bottom_value.clone()
        } else if z >= *top_altitude {
          top_value.clone()
        } else {
          let ratio = (z - *bottom_altitude) as f64 / (*top_altitude - *bottom_altitude) as f64;
          Value::mix(vec![(1.0 - ratio, top_value.clone()), (ratio, bottom_value.clone())])
        }
      }
      Current::BoolControlled { value, when_true, when_false, .. } => {
        if value.1 {
          when_true.clone()
        } else {
          when_false.clone()
        }
      }
      Current::Fixed(v) => v.clone(),
      Current::NumControlled { value, values, default_value, .. } => values.get(value.1 as usize).unwrap_or(default_value).clone(),
      Current::Setting(_) => todo!(),
    }
  }
}

impl<T, C: Change<T>> Change<T> for Arc<C> {
  type World = C::World;

  fn change(&self, value: T, world: &mut Self::World) {
    C::change(&*self, value, world)
  }
}
impl<T: Clone, C: Change<T>> Change<T> for Vec<C> {
  type World = C::World;

  fn change(&self, value: T, world: &mut Self::World) {
    for entry in self {
      entry.change(value.clone(), world)
    }
  }
}
impl<T, C: Change<T>> Change<T> for std::sync::Mutex<C> {
  type World = C::World;

  fn change(&self, value: T, world: &mut Self::World) {
    self.lock().unwrap().change(value, world);
  }
}
pub trait IntoGradiator<T>: Sized {
  type Error: Into<Cow<'static, str>>;
  type Update: Change<Self>;
  fn convert(input: T) -> Result<Self, Self::Error>;
}
pub(crate) fn load<T, C: IntoGradiator<T>>(
  bool_updates: &mut BTreeMap<String, Arc<AtomicBool>>,
  num_updates: &mut BTreeMap<String, Arc<AtomicU32>>,
) -> Result<BTreeMap<String, Gradiator<C, <C::Update as Change<C>>::World>>, C::Error> {
  struct GradiatorVariables<'a> {
    bool_updates: &'a mut BTreeMap<String, Arc<AtomicBool>>,
    num_updates: &'a mut BTreeMap<String, Arc<AtomicU32>>,
  }
  impl<'a> Resolver<String, String> for GradiatorVariables<'a> {
    type Bool = BoolUpdateState;
    type Num = NumUpdateState;
    fn resolve_bool(&mut self, value: String) -> Self::Bool {
      BoolUpdateState(
        match self.bool_updates.entry(value) {
          btree_map::Entry::Vacant(v) => {
            let value = Arc::new(AtomicBool::new(false));
            v.insert(value.clone());
            value
          }
          btree_map::Entry::Occupied(o) => o.get().clone(),
        },
        false,
      )
    }
    fn resolve_num(&mut self, value: String, _len: usize) -> Self::Num {
      NumUpdateState(
        match self.num_updates.entry(value) {
          btree_map::Entry::Vacant(v) => {
            let value = Arc::new(AtomicU32::new(0));
            v.insert(value.clone());
            value
          }
          btree_map::Entry::Occupied(o) => o.get().clone(),
        },
        0,
      )
    }
  }
  todo!()
}
