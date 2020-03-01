use crate::AssetManager;
use crate::ServerConnection;
use futures::sink::SinkExt;
use gfx_hal::Instance;
use serde::Deserialize;
use serde::Serialize;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
struct ServerConfiguration {
  url: String,
  user_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Configuration {
  servers: Vec<ServerConfiguration>,
}

enum ConnectionState {
  Dead,
  GettingAuthMethod(http::uri::Uri, String),
  AttemptingPasswordAuth(http::uri::Uri, String),
  Active {
    sink: futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, tokio_tungstenite::tungstenite::protocol::Message>,
    active: std::sync::atomic::AtomicBool,
  },
}

struct TungsteniteServerConnection {
  state: std::sync::Arc<std::sync::Mutex<ConnectionState>>,
}
impl TungsteniteServerConnection {
  fn new(project_dirs: &directories::ProjectDirs) -> Self {
    TungsteniteServerConnection { state: std::sync::Arc::new(std::sync::Mutex::new(ConnectionState::Dead)) }
  }
}
impl ServerConnection for TungsteniteServerConnection {
  fn send(&mut self, request: Vec<u8>) {
    let ss = self.state.clone();
    let sl = ss.lock().unwrap();
    match *sl {
      ConnectionState::Active { sink, active } => {
        sink.send(tungstenite::Message::Binary(request));
      }
      state => panic!("Connection state is inactive, but trying to send a message",),
    }
  }
}
mod conrod_winit_conv {
  conrod_winit::v021_conversion_fns!();
}
pub fn main() {
  std::panic::set_hook(Box::new(|msg| alert::alert("Puzzleverse Error", &format!("{}", msg))));
  let project_dirs = directories::ProjectDirs::from("org.puzzleverse", "Puzzleverse", "Puzzleverse Client").unwrap();

  let config_file = project_dirs.config_dir().to_path_buf();
  config_file.push("servers.json");
  let configuration = match std::fs::File::open(&config_file) {
    Ok(reader) => serde_json::from_reader::<_, Configuration>(reader).unwrap(),
    Err(_) => Configuration { servers: vec![] },
  };

  let events_loop = winit::event_loop::EventLoop::new();
  let window = winit::window::WindowBuilder::new().with_title("Puzzleverse".to_owned()).with_fullscreen(None).build(&events_loop).unwrap();
  let proxy = events_loop.create_proxy();
  let client = super::Client::new(TungsteniteServerConnection::new(&project_dirs), DiskAssetManager::new(&project_dirs));
  events_loop.run(|event, w, control_flow| {
    *control_flow = winit::event_loop::ControlFlow::Wait;

    match event {
      winit::event::Event::WindowEvent { event: window_event, window_id } => {
        if let Some(conrod_event) = conrod_winit_conv::convert_window_event(&window_event, &window) {
          ui.handle_event(&conrod_event);
        }
      }

      _ => (),
    }
  });
}
