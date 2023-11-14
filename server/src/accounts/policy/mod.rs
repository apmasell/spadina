use spadina_core::access::SimpleAccess;
use spadina_core::UpdateResult;
use std::future::Future;

pub trait Policy: Send + Sync {
  fn can_create(&self, player: &str) -> impl Future<Output = bool> + Send;
  fn is_administrator(&self, player: &str) -> impl Future<Output = bool> + Send;
  fn request(&self, request: PolicyRequest) -> impl Future<Output = UpdateResult> + Send;
}

pub enum PolicyRequest {
  AddAdmin(String),
  AddCreator(String),
  RemoveAdmin(String),
  RemoveCreator(String),
  SetCreator(SimpleAccess),
  SetAdmin(SimpleAccess),
}
