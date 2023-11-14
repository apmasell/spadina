use crate::client::Client;
use crate::socket_entity::Outgoing;
use futures::FutureExt;
use futures::StreamExt;
use serde::Serialize;
use spadina_core::net::server::ClientResponse;
use spadina_core::player::OnlineState;
use std::hash::Hash;
use tokio::sync::oneshot;

pub fn watch_online<S: AsRef<str> + Hash + Eq + Ord + Serialize + Send + Sync + 'static>(
  id: u32,
  state: Result<OnlineState<S>, oneshot::Receiver<OnlineState<S>>>,
) -> Vec<Outgoing<Client>> {
  let response = match state {
    Ok(state) => Outgoing::Send(ClientResponse::<_, &[u8]>::PlayerOnlineState { id, state }.into()),
    Err(rx) => Outgoing::SideTask(
      async move {
        let response = Outgoing::Send(ClientResponse::<_, &[u8]>::PlayerOnlineState { id, state: rx.await.unwrap_or(OnlineState::Unknown) }.into());
        vec![response]
      }
      .into_stream()
      .boxed(),
    ),
  };
  vec![response]
}
