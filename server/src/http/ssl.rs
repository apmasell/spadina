use futures::StreamExt;
use std::io::Read;
fn load_cert<P: AsRef<std::path::Path>>(certificate_path: P) -> Result<tokio_native_tls::TlsAcceptor, Box<dyn std::error::Error>> {
  let mut f = std::fs::File::open(&certificate_path)?;
  let mut buffer = Vec::new();
  f.read_to_end(&mut buffer)?;
  let cert = native_tls::Identity::from_pkcs12(&buffer, "")?;
  Ok(tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?))
}
pub(crate) async fn start(
  server: std::sync::Arc<super::WebServer>,
  certificate: Option<impl AsRef<std::path::Path>>,
  bind_address: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
  let addr = bind_address.unwrap_or(if certificate.is_none() { "0.0.0.0:80".to_string() } else { "0.0.0.0:443".to_string() });
  let mut death = server.authnz.give_me_death();
  let shutdown = async move {
    if let Err(e) = death.recv().await {
      eprintln!("Failed to wait on graceful shutdown handler: {}", e);
    }
  };
  match certificate {
    Some(certificate_path) => {
      let acceptor = std::sync::Arc::new(std::sync::RwLock::new(load_cert(&certificate_path)?));
      let server = server.clone();
      let (tx, rx) = std::sync::mpsc::channel();
      match notify::recommended_watcher(tx) {
        Ok(mut watcher) => {
          use notify::Watcher;
          watcher.watch(certificate_path.as_ref(), notify::RecursiveMode::NonRecursive).unwrap();
          let a = acceptor.clone();
          std::thread::spawn(move || loop {
            match rx.recv() {
              Ok(Ok(event)) => {
                if match event.kind {
                  notify::EventKind::Create(notify::event::CreateKind::File) | notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => true,
                  _ => false,
                } {
                  match load_cert(&event.paths[0]) {
                    Ok(acceptor) => {
                      *a.write().unwrap() = acceptor;
                    }
                    Err(e) => eprintln!("Failed to load new SSL cert: {}", e),
                  }
                }
              }
              Ok(Err(e)) => {
                eprintln!("SSL certificate loader died: {}", e);
                break;
              }
              Err(e) => {
                eprintln!("SSL certificate loader died: {}", e);
                break;
              }
            }
          });
        }
        Err(e) => eprintln!("Failed to set up watcher on SSL cert: {}", e),
      }

      struct TokioTcpListener(tokio::net::TcpListener);
      impl futures::Stream for TokioTcpListener {
        type Item = tokio::net::TcpStream;

        fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut futures::task::Context<'_>) -> futures::task::Poll<Option<Self::Item>> {
          self.0.poll_accept(cx).map(|result| match result {
            Ok((socket, _)) => Some(socket),
            Err(e) => {
              eprintln!("Failed to accept TCP request: {}", e);
              None
            }
          })
        }
      }

      hyper::server::Server::builder(hyper::server::accept::from_stream(TokioTcpListener(tokio::net::TcpListener::bind(&addr).await?).then(
        move |socket| {
          let acceptor = acceptor.clone();
          async move { acceptor.read().unwrap().accept(socket).await }
        },
      )))
      .serve(hyper::service::make_service_fn(move |_| {
        let server = server.clone();
        async move {
          Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req: http::Request<hyper::Body>| {
            let server = server.clone();
            server.handle_http_request(req)
          }))
        }
      }))
      .with_graceful_shutdown(shutdown)
      .await?;
    }
    None => {
      let server = server.clone();
      let addr = addr.parse().expect("Invalid bind address");
      hyper::Server::bind(&addr)
        .serve(hyper::service::make_service_fn(move |_| {
          let server = server.clone();
          async move {
            Ok::<_, std::convert::Infallible>(hyper::service::service_fn(move |req: http::Request<hyper::Body>| {
              let server = server.clone();
              server.handle_http_request(req)
            }))
          }
        }))
        .with_graceful_shutdown(shutdown)
        .await?
    }
  }
  Ok(())
}
