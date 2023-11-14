use crate::access::AccessManagement;
use crate::http_server::jwt::encode_jwt;
use crate::http_server::{aggregate, WebServer};
use crate::metrics::SharedString;
use crate::peer::net::{PeerClaim, PeerHttpRequestBody};
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{body::Incoming, http, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::error::Error;
use std::io;
use std::io::ErrorKind;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::WebSocketStream;

pub async fn handle(req: Request<Incoming>, web_server: &WebServer) -> http::Result<Response<Full<Bytes>>> {
  let PeerHttpRequestBody { server, token } = match aggregate::<PeerHttpRequestBody<String>>(req).await {
    Ok(request) => request,
    Err(response) => return response,
  };
  {
    let server = match spadina_core::net::parse_server_name(&server) {
      Some(server) => Arc::from(server),
      None => return Response::builder().status(StatusCode::BAD_REQUEST).body("Bad server name".into()),
    };
    if web_server
      .directory
      .access_management
      .banned_peers
      .read("handle_post", |bans| {
        bans.iter().all(|b| match b {
          spadina_core::access::BannedPeer::Peer(b) => *b.as_str() == *server,
          spadina_core::access::BannedPeer::Domain(domain) => spadina_core::net::has_domain_suffix(domain, &server),
        })
      })
      .await
    {
      return Response::builder().status(StatusCode::FORBIDDEN).body("Access denied".into());
    }
    let peer_labels = crate::metrics::PeerLabel { peer: SharedString(server.clone()) };
    let authority = match server.parse::<http::uri::Authority>() {
      Ok(authority) => authority,
      Err(e) => {
        crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
        return Response::builder().status(StatusCode::BAD_REQUEST).body(format!("Invalid peer name: {}", e).into());
      }
    };
    let uri = match hyper::Uri::builder().scheme(http::uri::Scheme::HTTPS).path_and_query(crate::peer::net::PATH_FINISH).authority(authority).build()
    {
      Ok(uri) => uri,
      Err(e) => {
        crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
        return Response::builder().status(StatusCode::BAD_REQUEST).body(format!("Failure to create peer URL: {}", e).into());
      }
    };
    let directory = web_server.directory.clone();
    tokio::spawn(async move {
      let result: Result<(), Box<dyn Error + Send + Sync>> = try {
        use rand::RngCore;
        let tls = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::new()?);
        let (mut sender, _) = hyper::client::conn::http1::handshake(TokioIo::new(
          tls
            .connect(
              &server,
              TcpStream::connect((uri.host().ok_or(io::Error::new(ErrorKind::Other, "No host in URL"))?, uri.port_u16().unwrap_or(443))).await?,
            )
            .await?,
        ))
        .await?;

        let response = sender
          .send_request(
            hyper::Request::get(uri)
              .version(http::Version::HTTP_11)
              .header(http::header::HOST, server.as_ref())
              .header(http::header::CONNECTION, "upgrade")
              .header(http::header::SEC_WEBSOCKET_VERSION, "13")
              .header(http::header::SEC_WEBSOCKET_PROTOCOL, "spadina")
              .header(http::header::UPGRADE, "websocket")
              .header(http::header::SEC_WEBSOCKET_KEY, format!("spadina{}", &mut rand::thread_rng().next_u64()))
              .header(http::header::AUTHORIZATION, format!("Bearer {}", token))
              .body(Full::new(Bytes::new()))
              .unwrap(),
          )
          .await?;
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
          let upgraded = hyper::upgrade::on(response).await?;
          let socket = WebSocketStream::from_raw_socket(upgraded.into(), Role::Client, None).await;
          directory.register_peer(server, socket).await.map_err(|_| io::Error::new(ErrorKind::Other, "Failed registration"))?;
        } else {
          crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
          let status = response.status();
          eprintln!("Failed to connect to {}: {}", &server, status);
        }
      };

      match result {
        Ok(()) => (),
        Err(e) => {
          crate::metrics::FAILED_SERVER_CALLBACK.get_or_create(&peer_labels).inc();
          eprintln!("Failed callback to {}: {}", &peer_labels.peer.0, e);
          return;
        }
      }
    });
    Response::builder().status(StatusCode::OK).body("Will do".into())
  }
}
pub async fn initiate(server: &str, auth: &AccessManagement) -> Result<(), ()> {
  let authority = server.parse::<http::uri::Authority>().map_err(|e| {
    println!("Bad peer server name {}: {}", server, e);
  })?;
  let uri = hyper::Uri::builder().scheme(http::uri::Scheme::HTTPS).path_and_query("/api/server/v1").authority(authority).build().map_err(|e| {
    println!("Bad URL construction for server name {}: {}", server, e);
  })?;
  let token = encode_jwt(&PeerClaim { exp: crate::http_server::jwt::expiry_time(3600), name: server }, auth).map_err(|e| {
    eprintln!("Failed to encode JWT for {}: {}", server, e);
  })?;
  let request = Full::new(Bytes::from(
    serde_json::to_vec(&PeerHttpRequestBody { token: token.as_str(), server: &auth.server_name })
      .map_err(|e| eprintln!("Failed to encode request for {}: {}", server, e))?,
  ));

  let tls = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::new().map_err(|e| {
    eprintln!("Failed contact to {}: {}", &server, e);
  })?);
  let (mut sender, _) = hyper::client::conn::http1::handshake(TokioIo::new(
    tls
      .connect(
        &server,
        TcpStream::connect((uri.host().ok_or(())?, uri.port_u16().unwrap_or(443))).await.map_err(|e| {
          eprintln!("Failed contact to {}: {}", &server, e);
        })?,
      )
      .await
      .map_err(|e| {
        eprintln!("Failed contact to {}: {}", &server, e);
      })?,
  ))
  .await
  .map_err(|e| {
    eprintln!("Failed contact to {}: {}", &server, e);
  })?;
  let response = sender.send_request(hyper::Request::post(uri).version(http::Version::HTTP_11).body(request).unwrap()).await.map_err(|e| {
    eprintln!("Failed contact to {}: {}", &server, e);
  })?;
  let status = response.status();
  if status != StatusCode::OK {
    let body = response.into_body().collect().await.map(|b| b.to_bytes());
    eprintln!(
      "Failed to connect to peer server {} ({}): {}",
      &server,
      status,
      body.as_ref().map(|body| std::str::from_utf8(&*body).unwrap_or("<invalid UTF-8>")).unwrap_or("<Failed to read body>")
    );
    return Err(());
  }
  Ok(())
}
