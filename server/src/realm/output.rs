#[derive(Eq, PartialEq)]
pub(crate) enum Multi<T> {
  Single(T),
  Multi(T, std::collections::BTreeMap<u8, T>),
}

impl<T: Clone> Multi<T> {
  pub fn convolve<K: Clone + std::cmp::Eq + std::hash::Hash, R>(
    input: &std::collections::HashMap<K, Multi<T>>,
    mapper: impl Fn(std::collections::HashMap<K, T>) -> R,
  ) -> Multi<R> {
    let mut default = std::collections::HashMap::new();
    let mut for_states: std::collections::BTreeMap<_, std::collections::HashMap<_, _>> = Default::default();
    for (key, value) in input {
      match value {
        Multi::Single(value) => {
          default.insert(key.clone(), value.clone());
        }
        Multi::Multi(default_value, values) => {
          default.insert(key.clone(), default_value.clone());
          for (state, value) in values {
            for_states.entry(*state).or_default().insert(key.clone(), value.clone());
          }
        }
      }
    }

    if for_states.is_empty() {
      Multi::Single(mapper(default))
    } else {
      let for_states = for_states
        .into_iter()
        .map(|(state, mut output)| {
          for (key, value) in default.iter() {
            if let std::collections::hash_map::Entry::Vacant(v) = output.entry(key.clone()) {
              v.insert(value.clone());
            }
          }
          (state, mapper(output))
        })
        .collect();
      Multi::Multi(mapper(default), for_states)
    }
  }
}
