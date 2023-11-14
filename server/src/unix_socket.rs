use crate::accounts::login::Login;
use crate::directory::Directory;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_tungstenite::tungstenite::handshake::server::{Callback, ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http;

pub(crate) fn start(path: String, directory: Directory) {
  if Path::new(&path).exists() {
    if let Err(e) = std::fs::remove_file(&path) {
      eprintln!("Failed to remove UNIX socket {}: {}", &path, e);
      return;
    }
  }
  let listener = UnixListener::bind(&path).expect("Failed to create UNIX socket");

  tokio::spawn(async move {
    let mut death = directory.access_management.give_me_death();
    loop {
      match tokio::select! {
      conn = listener.accept() => conn,
      _ = death.recv() => break,
      } {
        Ok((stream, _)) => {
          let mut player_name = Default::default();
          match tokio_tungstenite::accept_hdr_async(
            spadina_core::net::mixed_connection::MixedConnection::Unix(stream),
            LoginStore { directory: &directory, player_name: &mut player_name },
          )
          .await
          {
            Ok(connection) => {
              let _ = directory.register_player(Arc::from(player_name), connection);
            }
            Err(e) => {
              eprintln!("Failed to connect on UNIX socket: {}", e);
            }
          }
        }
        Err(e) => {
          eprintln!("Failed to connect on UNIX socket: {}", e);
        }
      }
    }
    if let Err(e) = std::fs::remove_file(&path) {
      eprintln!("Failed to remove UNIX socket {}: {}", &path, e);
    }
  });
}
struct LoginStore<'a> {
  directory: &'a Directory,
  player_name: &'a mut String,
}
impl<'a> Callback for LoginStore<'a> {
  fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    match futures::executor::block_on(self.directory.access_management.accounts.normalize_username(request.uri().path().to_string())) {
      Ok(player_name) => {
        *self.player_name = player_name;
        Ok(response)
      }
      Err(()) => Err(Response::builder().status(http::StatusCode::FORBIDDEN).body(None).unwrap()),
    }
  }
}
