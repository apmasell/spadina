use crate::asset::Loaded;

pub trait ExtractChildren<S: AsRef<str>> {
  fn extract_children(&self, assets: &mut std::collections::BTreeSet<S>);
}

impl<S: AsRef<str> + Ord + Clone> ExtractChildren<S> for S {
  fn extract_children(&self, assets: &mut std::collections::BTreeSet<S>) {
    assets.insert(self.clone());
  }
}

impl<S: AsRef<str> + Ord + Clone, T: ExtractChildren<S>> ExtractChildren<S> for Loaded<T, S> {
  fn extract_children(&self, assets: &mut std::collections::BTreeSet<S>) {
    assets.insert(self.asset().clone());
    self.value.extract_children(assets);
  }
}
