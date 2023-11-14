use crate::http_server::WebServer;
use hyper_util::rt::TokioIo;
use notify::event::CreateKind;
use notify::Watcher;
use notify::{Event, EventHandler, EventKind};
use std::error::Error;
use std::io::Read;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::spawn;
use tokio_native_tls::native_tls::Identity;
use tokio_native_tls::TlsAcceptor;

struct UpdateCertificate(Arc<RwLock<TlsAcceptor>>);

impl EventHandler for UpdateCertificate {
  fn handle_event(&mut self, event: notify::Result<Event>) {
    match event {
      Ok(event) => match event.kind {
        EventKind::Create(CreateKind::File) | EventKind::Modify(notify::event::ModifyKind::Data(_)) => match load_cert(&event.paths[0]) {
          Ok(acceptor) => {
            *self.0.write().expect("Failed to update TLS certificate") = acceptor;
          }
          Err(e) => eprintln!("Failed to load new SSL cert: {}", e),
        },
        _ => (),
      },
      Err(e) => {
        eprintln!("SSL certificate loader error: {}", e);
      }
    }
  }
}
fn load_cert<P: AsRef<std::path::Path>>(certificate_path: P) -> Result<TlsAcceptor, Box<dyn Error + Send + Sync>> {
  let mut f = std::fs::File::open(&certificate_path)?;
  let mut buffer = Vec::new();
  f.read_to_end(&mut buffer)?;
  let cert = Identity::from_pkcs12(&buffer, "")?;
  Ok(TlsAcceptor::from(native_tls::TlsAcceptor::new(cert)?))
}
pub(crate) async fn start(
  server: WebServer,
  certificate: Option<impl AsRef<std::path::Path>>,
  bind_address: Option<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
  let (tls, default_address) = match certificate {
    Some(certificate_path) => {
      let acceptor = Arc::new(RwLock::new(load_cert(&certificate_path)?));
      let mut watcher = notify::recommended_watcher(UpdateCertificate(acceptor.clone()))?;
      watcher.watch(certificate_path.as_ref(), notify::RecursiveMode::NonRecursive)?;
      (Some(acceptor), "0.0.0.0:443")
    }
    None => (None, "0.0.0.0:80"),
  };
  let addr = bind_address.as_ref().map(|a| a.as_str()).unwrap_or(default_address);
  eprintln!("Starting web server on {}", addr);
  let listener = TcpListener::bind(addr).await?;
  let mut death = server.directory.access_management.give_me_death();
  loop {
    let (stream, _) = tokio::select! {biased;
    _ = death.recv() => break,
    r = listener.accept() => r,
    }?;
    let server = server.clone();
    match tls.as_ref() {
      None => {
        spawn(async move {
          if let Err(e) = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), server).await {
            eprintln!("Failed serving connection: {:?}", e);
          }
        });
      }
      Some(acceptor) => {
        let acceptor = acceptor.read().expect("SSL acceptor is broken").clone();
        spawn(async move {
          let stream = match acceptor.accept(stream).await {
            Ok(a) => a,
            Err(e) => {
              eprintln!("Failed SSL negotiation connection: {:?}", e);
              return;
            }
          };
          if let Err(e) = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), server).await {
            eprintln!("Failed serving connection: {:?}", e);
          }
        });
      }
    }
  }
  Ok(())
}
