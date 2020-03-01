pub(crate) fn start(
  path: String,
  directory: std::sync::Weak<crate::destination::Directory>,
  authnz: std::sync::Arc<crate::access::AuthNZ>,
  database: std::sync::Arc<crate::database::Database>,
) {
  let listener = tokio::net::UnixListener::bind(&path).expect("Failed to create UNIX socket");
  if std::path::Path::new(&path).exists() {
    if let Err(e) = std::fs::remove_file(&path) {
      eprintln!("Failed to remove UNIX socket {}: {}", &path, e);
    }
  }

  tokio::spawn(async move {
    let mut death = authnz.give_me_death();
    loop {
      match tokio::select! {
      conn = listener.accept() => conn,
      _ = death.recv() => break,
      } {
        Ok((stream, _)) => {
          let mut capabilities = Default::default();
          let mut player_name = Default::default();
          let mut is_superuser = false;
          match tokio_tungstenite::accept_hdr_async(
            spadina_core::net::IncomingConnection::Unix(stream),
            LoginStore { capabilities: &mut capabilities, player_name: &mut player_name, is_superuser: &mut is_superuser },
          )
          .await
          {
            Ok(connection) => {
              let player_name: std::sync::Arc<str> = std::sync::Arc::from(player_name);
              match directory.upgrade() {
                Some(directory) => {
                  if let Some(old_client) = directory.players.insert(
                    player_name.clone(),
                    crate::client::Client::new(
                      player_name.clone(),
                      authnz.clone(),
                      database.clone(),
                      is_superuser,
                      std::sync::Arc::new(capabilities),
                      std::sync::Arc::downgrade(&directory),
                      connection,
                    ),
                  ) {
                    old_client.kill().await;
                  }
                }
                None => (),
              }
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
  capabilities: &'a mut std::collections::BTreeSet<&'static str>,
  player_name: &'a mut String,
  is_superuser: &'a mut bool,
}
impl<'a> tokio_tungstenite::tungstenite::handshake::server::Callback for LoginStore<'a> {
  fn on_request(
    self,
    request: &tokio_tungstenite::tungstenite::handshake::server::Request,
    response: tokio_tungstenite::tungstenite::handshake::server::Response,
  ) -> Result<tokio_tungstenite::tungstenite::handshake::server::Response, tokio_tungstenite::tungstenite::handshake::server::ErrorResponse> {
    let parts: Vec<_> = request.uri().path().splitn(2, '/').collect();
    match (spadina_core::capabilities::capabilities_from_header(request), parts[0], parts.get(1).map(|s| s.parse().ok()).flatten()) {
      (_, player_name, _) if player_name.is_empty() => {
        Err(http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(Some("No user name provided".to_string())).unwrap())
      }
      (Ok(found), player_name, Some(is_superuser)) => {
        *self.player_name = player_name.into();
        *self.capabilities = found;
        *self.is_superuser = is_superuser;
        Ok(response)
      }
      (Err(e), _, _) => Err(http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(Some(e.to_string())).unwrap()),
      (_, _, None) => Err(http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(Some("Super user not supplied".to_string())).unwrap()),
    }
  }
}
