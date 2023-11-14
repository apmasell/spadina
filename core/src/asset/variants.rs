use crate::asset::extraction::ExtractChildren;
use crate::asset::Asset;
use crate::controller::GenericControllerTemplate;
use crate::realm::room_world;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize, Deserialize)]
pub enum AllSupportedAssets<S: AsRef<str> + Ord> {
  RoomWorld(room_world::World<u16, u16, u16, S>),
}

impl<S: AsRef<str> + Ord> AllSupportedAssets<S> {
  pub fn create_realm_template<B: AsRef<[u8]>>(
    &self,
    children: &BTreeMap<String, impl AsRef<Asset<S, B>>>,
  ) -> Result<GenericControllerTemplate, Option<Vec<S>>> {
    todo!()
  }
  pub fn validate(&self) -> Result<(), Cow<'static, str>> {
    todo!()
  }
}

impl<S: AsRef<str> + Ord> ExtractChildren<S> for AllSupportedAssets<S> {
  fn extract_children(&self, assets: &mut BTreeSet<S>) {
    match self {
      AllSupportedAssets::RoomWorld(a) => a.extract_children(assets),
    }
  }
}
