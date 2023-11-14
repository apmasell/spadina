use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::protocol::LocationResponse;
use spadina_core::location::target::UnresolvedTarget;
use spadina_core::shared_ref::SharedRef;
use std::sync::Arc;

pub enum PlayerLocationUpdate {
  Move(UnresolvedTarget<SharedRef<str>>),
  ResolveUpdate(LocationChangeResponse<Arc<str>>),
  ResponseSingle(LocationResponse<String, Vec<u8>>),
  ResponseShared(LocationResponse<Arc<str>, Arc<[u8]>>),
}
