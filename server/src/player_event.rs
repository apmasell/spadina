use spadina_core::avatar::Avatar;
use spadina_core::location::protocol::LocationRequest;

pub enum PlayerEvent {
  Avatar(Avatar),
  Request(LocationRequest<String, Vec<u8>>),
}
