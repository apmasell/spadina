use crate::player_event::PlayerEvent;
use crate::player_location_update::PlayerLocationUpdate;
use spadina_core::avatar::Avatar;
use spadina_core::player::SharedPlayerIdentifier;
use tokio::sync::mpsc;

pub struct JoinRequest {
  pub avatar: Avatar,
  pub is_superuser: bool,
  pub name: SharedPlayerIdentifier,
  pub tx: mpsc::Sender<PlayerLocationUpdate>,
  pub rx: mpsc::Receiver<PlayerEvent>,
}

impl std::fmt::Debug for JoinRequest {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("JoinRequest").field("is_superuser", &self.is_superuser).field("name", &self.name).finish()
  }
}
