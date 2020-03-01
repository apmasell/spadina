use futures::{SinkExt, StreamExt};

// When adding new features, add them to this array
const CLIENT_CAPABILITIES: &[&str] = &["base"];

enum ServerParameter {
  Fixed(String),
  Default(String),
  UserDefined,
}
#[async_trait::async_trait]
trait ServerConnection: Send + Sync + std::fmt::Debug {
  async fn authenticate_password(server: String, insecure: bool, user: String, password: String) -> Result<std::sync::Arc<Self>, String>;
  async fn authentication_schemes(server: String, insecure: bool) -> Result<puzzleverse_core::AuthScheme, String>;
  async fn send(&mut self, request: Vec<u8>);
  async fn send_request(&mut self, request: puzzleverse_core::ClientRequest) {
    self.send(rmp_serde::to_vec(&request).unwrap()).await
  }
  fn subscribe(&self) -> iced::Subscription<Option<puzzleverse_core::ClientResponse>>;
}
#[cfg(not(target_arch = "wasm32"))]
type ResponseStream = std::sync::Arc<std::sync::Mutex<futures::stream::SplitStream<tokio_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>>>>;
#[cfg(not(target_arch = "wasm32"))]
struct TungsteniteConnection {
  outbound: futures::stream::SplitSink<tokio_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>, tungstenite::Message>,
  inbound: ResponseStream,
}

impl std::fmt::Debug for TungsteniteConnection {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("<Active Connection>")
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl ServerConnection for TungsteniteConnection {
  async fn authenticate_password(server: String, insecure: bool, username: String, password: String) -> Result<std::sync::Arc<Self>, String> {
    match &server.parse::<http::uri::Authority>() {
      Ok(authority) => {
        let token: String = match hyper::Uri::builder()
          .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
          .path_and_query("/api/auth/password")
          .authority(authority.clone())
          .build()
        {
          Ok(uri) => {
            let connector = hyper_tls::HttpsConnector::new();
            let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

            match client
              .request(
                hyper::Request::post(uri)
                  .body(hyper::Body::from(serde_json::to_vec(&puzzleverse_core::PasswordRequest { username, password }).map_err(|e| e.to_string())?))
                  .unwrap(),
              )
              .await
            {
              Err(e) => Err(e.to_string()),
              Ok(response) => {
                use bytes::buf::Buf;
                if response.status() == http::StatusCode::OK {
                  std::str::from_utf8(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk())
                    .map_err(|e| e.to_string())
                    .map(|s| s.to_string())
                } else {
                  let status = response.status();
                  match hyper::body::aggregate(response).await {
                    Err(e) => Err(format!("Failed to connect to {} {}: {}", &server, status, e)),
                    Ok(buf) => {
                      Err(format!("Failed to connect to {} {}: {}", &server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                    }
                  }
                }
              }
            }
          }
          Err(e) => Err(e.to_string()),
        }?;
        match hyper::Uri::builder()
          .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
          .path_and_query("/api/client/v1")
          .authority(authority.clone())
          .build()
        {
          Ok(uri) => {
            let connector = hyper_tls::HttpsConnector::new();
            let client = hyper::Client::builder().build::<_, hyper::Body>(connector);
            use rand::RngCore;

            match client
              .request(
                hyper::Request::get(uri)
                  .version(http::Version::HTTP_11)
                  .header(http::header::CONNECTION, "upgrade")
                  .header(http::header::SEC_WEBSOCKET_VERSION, "13")
                  .header(http::header::SEC_WEBSOCKET_PROTOCOL, "puzzleverse")
                  .header(http::header::UPGRADE, "websocket")
                  .header(http::header::SEC_WEBSOCKET_KEY, format!("pv{}", &mut rand::thread_rng().next_u64()))
                  .header(http::header::AUTHORIZATION, format!("Bearer {}", token))
                  .body(hyper::Body::empty())
                  .unwrap(),
              )
              .await
            {
              Err(e) => Err(e.to_string()),
              Ok(response) => {
                if response.status() == http::StatusCode::SWITCHING_PROTOCOLS {
                  match hyper::upgrade::on(response).await {
                    Ok(upgraded) => {
                      use futures::prelude::*;
                      let (writer, reader) =
                        tokio_tungstenite::WebSocketStream::from_raw_socket(upgraded, tokio_tungstenite::tungstenite::protocol::Role::Client, None)
                          .await
                          .split();
                      Ok(std::sync::Arc::new(TungsteniteConnection { outbound: writer, inbound: std::sync::Arc::new(std::sync::Mutex::new(reader)) }))
                    }
                    Err(e) => Err(e.to_string()),
                  }
                } else {
                  use bytes::buf::Buf;
                  let status = response.status();
                  match hyper::body::aggregate(response).await {
                    Err(e) => Err(format!("Failed to connect to {} {}: {}", &server, status, e)),
                    Ok(buf) => {
                      Err(format!("Failed to connect to {} {}: {}", &server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                    }
                  }
                }
              }
            }
          }
          Err(e) => Err(e.to_string()),
        }
      }
      Err(e) => Err(e.to_string()),
    }
  }

  async fn authentication_schemes(server: String, insecure: bool) -> Result<puzzleverse_core::AuthScheme, String> {
    match &server.parse::<http::uri::Authority>() {
      Ok(authority) => {
        match hyper::Uri::builder()
          .scheme(if insecure { http::uri::Scheme::HTTP } else { http::uri::Scheme::HTTPS })
          .path_and_query("/api/auth/method")
          .authority(authority.clone())
          .build()
        {
          Ok(uri) => {
            let connector = hyper_tls::HttpsConnector::new();
            let client = hyper::Client::builder().build::<_, hyper::Body>(connector);

            match client.request(hyper::Request::get(uri).body(hyper::Body::empty()).unwrap()).await {
              Err(e) => Err(e.to_string()),
              Ok(response) => {
                use bytes::buf::Buf;
                if response.status() == http::StatusCode::OK {
                  serde_json::from_slice(hyper::body::aggregate(response).await.map_err(|e| e.to_string())?.chunk()).map_err(|e| e.to_string())
                } else {
                  let status = response.status();
                  match hyper::body::aggregate(response).await {
                    Err(e) => Err(format!("Failed to connect to {} {}: {}", &server, status, e)),
                    Ok(buf) => {
                      Err(format!("Failed to connect to {} {}: {}", &server, status, std::str::from_utf8(buf.chunk()).unwrap_or("Bad UTF-8 data"),))
                    }
                  }
                }
              }
            }
          }
          Err(e) => Err(e.to_string()),
        }
      }
      Err(e) => Err(e.to_string()),
    }
  }

  async fn send(&mut self, request: Vec<u8>) {
    if let Err(e) = self.outbound.send(tungstenite::Message::Binary(request)).await {
      eprintln!("Error sending packet: {}", e);
    }
  }

  fn subscribe(&self) -> iced::Subscription<Option<puzzleverse_core::ClientResponse>> {
    fn decode_server_messages(input: Result<tungstenite::Message, tungstenite::Error>) -> Option<puzzleverse_core::ClientResponse> {
      match input {
        Ok(tungstenite::Message::Binary(value)) => match rmp_serde::from_read(std::io::Cursor::new(&value)) {
          Ok(v) => Some(v),
          Err(e) => {
            eprintln!("Failed to decode message from server. Mismatched protocols?: {}", e);
            None
          }
        },
        Ok(_) => None,
        Err(e) => {
          eprintln!("Failed to decode Web Socket packet: {}", e);
          None
        }
      }
    }
    struct ServerRecipe(ResponseStream);
    impl futures::stream::Stream for ServerRecipe {
      type Item = Result<tungstenite::Message, tungstenite::Error>;

      fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Option<Self::Item>> {
        self.0.lock().unwrap().poll_next_unpin(cx)
      }
    }
    impl<H: std::hash::Hasher, I> iced_futures::subscription::Recipe<H, I> for ServerRecipe {
      type Output = Result<tungstenite::Message, tungstenite::Error>;

      fn hash(&self, state: &mut H) {
        state.write(b"connection");
        state.write_u128(std::time::Instant::now().elapsed().as_nanos());
      }

      fn stream(self: Box<Self>, _: futures::stream::BoxStream<I>) -> futures::stream::BoxStream<Self::Output> {
        self.boxed()
      }
    }
    iced::Subscription::from_recipe(ServerRecipe(self.inbound.clone())).map(decode_server_messages)
  }
}

#[cfg(target_arch = "wasm32")]
async fn synchronous_http(url: &str, body: Option<&str>) -> Result<String, String> {
  use stdweb::web::IEventTarget;
  let request = stdweb::web::XmlHttpRequest::new();
  request
    .open(
      match body {
        None => "GET",
        Some(_) => "POST",
      },
      url,
    )
    .unwrap();

  let (mut sender, mut receiver) = futures::channel::mpsc::channel(1);
  let _on_load = {
    let xhr = request.clone();
    let mut sender = sender.clone();
    request.add_event_listener(move |event: stdweb::web::event::ProgressLoadEvent| {
      futures::executor::block_on(sender.send(Ok(xhr.response_text().unwrap().unwrap()))).unwrap();
    })
  };

  let _on_error = request.add_event_listener(move |_: stdweb::web::event::ProgressErrorEvent| {
    sender.send(Err("Failed to load URL".to_string()));
  });
  match body {
    None => request.send(),
    Some(body) => request.send_with_string(body),
  }
  .unwrap();
  match receiver.next().await {
    None => Err("Failed to do synchronous HTTP request".to_string()),
    Some(v) => v,
  }
}
#[cfg(target_arch = "wasm32")]
struct WebSocketHolder {
  socket: stdweb::web::WebSocket,
  _handler: stdweb::web::EventListenerHandle,
}
#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait]
impl ServerConnection for WebSocketHolder {
  async fn authenticate_password(
    _: String,
    _: bool,
    username: String,
    password: String,
  ) -> Result<(std::sync::Arc<Self>, Box<dyn futures::Stream<Item = puzzleverse_core::ClientResponse>>), String> {
    use stdweb::web::IEventTarget;
    let token = synchronous_http(
      "/api/auth/password",
      Some(&serde_json::to_string(&puzzleverse_core::PasswordRequest { username, password }).map_err(|e| e.to_string())?),
    )
    .await?;
    let mut port = stdweb::web::window().location().unwrap().port().map_err(|e| e.to_string())?;
    if !port.is_empty() {
      port.insert(0, ':');
    }
    let ws = stdweb::web::WebSocket::new_with_protocols(
      &format!(
        "{}://{}{}/api/client/v1?token={}",
        if &stdweb::web::window().location().unwrap().protocol().map_err(|e| e.to_string())? == "https" { "wss" } else { "ws" },
        stdweb::web::window().location().unwrap().host().map_err(|e| e.to_string())?,
        port,
        token
      ),
      &["puzzleverse"],
    )
    .map_err(|e| e.to_string())?;
    ws.set_binary_type(stdweb::web::SocketBinaryType::ArrayBuffer);
    let (mut sender, receiver) = futures::channel::mpsc::unbounded();
    let handler = ws.add_event_listener(move |message: stdweb::web::event::SocketMessageEvent| {
      use stdweb::traits::IMessageEvent;
      let result = match message.data() {
        stdweb::web::event::SocketMessageData::ArrayBuffer(buffer) => {
          match rmp_serde::from_read(std::io::Cursor::new(&stdweb::web::TypedArray::<u8>::from(buffer).to_vec())) {
            Ok(m) => {
              if let Err(e) = futures::executor::block_on(sender.send(m)) {
                eprintln!("Failed to hand-off HTTP value: {}", e);
              }
            }
            Err(e) => {
              eprintln!("Bad message from server: {}", e);
            }
          }
        }
        _ => {
          eprintln!("Unexpected data type received on web socket.");
        }
      };
    });
    Ok((Box::new(WebSocketHolder { socket: ws, _handler: handler }), Box::new(receiver)))
  }

  async fn authentication_schemes(_: String, _: bool) -> Result<puzzleverse_core::AuthScheme, String> {
    serde_json::from_str(&synchronous_http("/api/auth/method", None).await?).map_err(|e| e.to_string())
  }

  async fn send(&mut self, request: Vec<u8>) {
    self.socket.send_bytes(&request).unwrap();
  }
}

enum ConnectionState<C: ServerConnection> {
  Idle,
  Active(std::sync::Arc<C>),
  Dead(Vec<puzzleverse_core::ClientRequest>),
}

impl<C: 'static + ServerConnection> ConnectionState<C> {
  fn subscribe(&self) -> impl Iterator<Item = iced::Subscription<UpdateMessage<C>>> {
    match self {
      ConnectionState::Active(c) => Some(c.subscribe().map(|v| match v {
        Some(m) => UpdateMessage::Response(m),
        None => UpdateMessage::NoOp,
      })),
      _ => None,
    }
    .into_iter()
  }
}

struct Client<C: 'static + ServerConnection, S: puzzleverse_core::asset_store::AssetStore> {
  connection: ConnectionState<C>,
  insecure: bool,
  mode: iced::window::Mode,
  password: String,
  player_name: String,
  player: ServerParameter,
  progress_current: f32,
  progress_range: std::ops::RangeInclusive<f32>,
  screen: Vec<ScreenState>,
  server: String,
  store: S,
}
struct ClientConfiguration<S: puzzleverse_core::asset_store::AssetStore> {
  insecure: bool,
  server: ServerParameter,
  user: ServerParameter,
  store: S,
}
impl<C: 'static + ServerConnection, S: puzzleverse_core::asset_store::AssetStore> iced::Application for Client<C, S> {
  type Executor = iced_futures::executor::Tokio;
  type Message = UpdateMessage<C>;
  type Flags = ClientConfiguration<S>;

  fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
    match flags.server {
      ServerParameter::Fixed(server) => (
        Client {
          connection: ConnectionState::Idle,
          insecure: flags.insecure,
          mode: iced::window::Mode::Fullscreen,
          password: String::new(),
          player_name: String::new(),
          player: flags.user,
          progress_current: 0.0,
          progress_range: 0.0..=1.0,
          screen: vec![ScreenState::Waiting],
          server: server.clone(),
          store: flags.store,
        },
        iced::Command::perform(C::authentication_schemes(server, flags.insecure), UpdateMessage::from),
      ),
      ServerParameter::Default(server) => (
        Client {
          connection: ConnectionState::Idle,
          insecure: flags.insecure,
          mode: iced::window::Mode::Fullscreen,
          password: String::new(),
          player: flags.user,
          player_name: String::new(),
          progress_current: 0.0,
          progress_range: 0.0..=1.0,
          screen: vec![ScreenState::ServerSelection {
            connect: iced::button::State::new(),
            quit: iced::button::State::new(),
            server: iced::text_input::State::focused(),
          }],
          server: server.clone(),
          store: flags.store,
        },
        iced::Command::none(),
      ),
      ServerParameter::UserDefined => (
        Client {
          connection: ConnectionState::Idle,
          insecure: flags.insecure,
          mode: iced::window::Mode::Fullscreen,
          password: String::new(),
          player: flags.user,
          player_name: String::new(),
          progress_range: 0.0..=1.0,
          progress_current: 0.0,
          screen: vec![ScreenState::ServerSelection {
            connect: iced::button::State::new(),
            quit: iced::button::State::new(),
            server: iced::text_input::State::focused(),
          }],
          server: String::new(),
          store: flags.store,
        },
        iced::Command::none(),
      ),
    }
  }

  fn title(&self) -> String {
    match &self.screen.last() {
      Some(ScreenState::Realm { realm_name, .. }) => realm_name.clone(),
      _ => "Puzzleverse".to_string(),
    }
  }

  fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
    match message {
      UpdateMessage::NoOp => iced::Command::none(),
      UpdateMessage::Quit => todo!(),
      UpdateMessage::UpdateServer(server) => {
        self.server = server;
        iced::Command::none()
      }
      UpdateMessage::UpdatePlayer(name) => {
        self.player_name = name;
        iced::Command::none()
      }
      UpdateMessage::UpdatePassword(password) => {
        self.password = password;
        iced::Command::none()
      }
      UpdateMessage::ConnectToServer => iced::Command::perform(C::authentication_schemes(self.server.clone(), self.insecure), UpdateMessage::from),
      UpdateMessage::ShowLoginPassword => {
        self.player_name.clear();
        match &self.player {
          ServerParameter::Fixed(name) => self.player_name.push_str(&name),
          ServerParameter::Default(name) => self.player_name.push_str(&name),
          ServerParameter::UserDefined => {}
        }
        self.screen.push(ScreenState::PasswordLogin {
          login: iced::button::State::new(),
          player: iced::text_input::State::new(),
          password: iced::text_input::State::new(),
        });
        iced::Command::none()
      }
      UpdateMessage::Connect(connection) => {
        self.connection = ConnectionState::Active(connection);
        self.screen.clear();
        self.screen.push(ScreenState::InTransit); // TODO should probably make a home request
        iced::Command::none()
      }
      UpdateMessage::AuthenticateWithPassword => {
        self.screen.push(ScreenState::Busy("Authenticating...".to_string()));
        iced::Command::perform(
          C::authenticate_password(self.server.clone(), self.insecure, self.player_name.clone(), self.password.clone()),
          |result| match result {
            Ok(connection) => UpdateMessage::Connect(connection),
            Err(e) => UpdateMessage::ShowConnectionError(e.to_string(), true),
          },
        )
      }
      UpdateMessage::ShowConnectionError(error, pop) => {
        if pop {
          self.screen.pop();
        }
        self.screen.push(ScreenState::Error(error));
        iced::Command::none()
      }
      UpdateMessage::Tick(_) => {
        // TODO
        iced::Command::none()
      }
      UpdateMessage::Response(_) => todo!(),
    }
  }

  fn subscription(&self) -> iced::Subscription<Self::Message> {
    iced::Subscription::batch(
      std::iter::once(iced::time::every(std::time::Duration::from_millis(10)).map(UpdateMessage::Tick)).chain(self.connection.subscribe()),
    )
  }

  fn view(&mut self) -> iced::Element<Self::Message> {
    match self.screen.last_mut() {
      Some(ScreenState::InTransit) => iced::Element::new(
        iced::Text::new("Finding Realm...")
          .horizontal_alignment(iced::HorizontalAlignment::Center)
          .vertical_alignment(iced::VerticalAlignment::Center),
      ),
      Some(ScreenState::Loading) => iced::Element::new(
        iced::Column::new()
          .align_items(iced::Align::Center)
          .push(
            iced::Text::new("Loading Assets for Realm...")
              .horizontal_alignment(iced::HorizontalAlignment::Center)
              .vertical_alignment(iced::VerticalAlignment::Center),
          )
          .push(iced::ProgressBar::new(self.progress_range.clone(), self.progress_current)),
      ),
      Some(ScreenState::Lost(_)) => todo!(),
      Some(ScreenState::PasswordLogin { login, player, password }) => iced::Element::new(
        iced::Column::new()
          .push(iced::Row::new().push(iced::Text::new("Player: ")).push(iced::TextInput::new(
            player,
            "Name",
            &self.player_name.clone(),
            UpdateMessage::UpdatePlayer,
          )))
          .push(
            iced::Row::new()
              .push(iced::Text::new("Password: "))
              .push(iced::TextInput::new(password, "Password", &self.password, UpdateMessage::UpdatePassword).password()),
          )
          .push(iced::Button::new(login, iced::Text::new("Login")).on_press(UpdateMessage::AuthenticateWithPassword)),
      ),
      Some(ScreenState::Realm { pane, .. }) => {
        iced::Element::new(iced::PaneGrid::new(pane, |pane, content| iced::pane_grid::Content::new(content.view())))
      }
      Some(ScreenState::ServerSelection { connect, server, quit }) => iced::Element::new(
        iced::Column::new()
          .push(
            iced::Row::new().push(iced::Text::new("Server: ")).push(
              iced::TextInput::new(server, "example.com", &self.server, UpdateMessage::UpdateServer)
                .size(255)
                .on_submit(UpdateMessage::ConnectToServer),
            ),
          )
          .push(
            iced::Row::new()
              .push(iced::Button::new(connect, iced::Text::new("Connect")).on_press(UpdateMessage::ConnectToServer))
              .push(iced::Button::new(quit, iced::Text::new("Quit")).on_press(UpdateMessage::Quit)),
          ),
      ),
      Some(ScreenState::Busy(message)) => iced::Element::new(
        iced::Text::new(message.clone()).horizontal_alignment(iced::HorizontalAlignment::Center).vertical_alignment(iced::VerticalAlignment::Center),
      ),
      Some(ScreenState::Waiting) => iced::Element::new(
        iced::Text::new("Connecting...").horizontal_alignment(iced::HorizontalAlignment::Center).vertical_alignment(iced::VerticalAlignment::Center),
      ),
      None => iced::Element::new(
        iced::Text::new("Nothing happening")
          .horizontal_alignment(iced::HorizontalAlignment::Center)
          .vertical_alignment(iced::VerticalAlignment::Center),
      ),
      Some(ScreenState::Error(error)) => iced::Element::new(
        iced::Text::new(error.clone()).horizontal_alignment(iced::HorizontalAlignment::Center).vertical_alignment(iced::VerticalAlignment::Center),
      ),
    }
  }

  fn mode(&self) -> iced::window::Mode {
    self.mode
  }

  fn background_color(&self) -> iced::Color {
    iced::Color::BLACK
  }

  fn scale_factor(&self) -> f64 {
    1.0
  }
}

enum RealmSelector {
  Player,
  Local,
  Bookmarks,
  Remote(String),
}

struct RealmSelectorState {}

enum ScreenState {
  Error(String),
  Busy(String),
  InTransit,
  Loading,
  Lost(RealmSelectorState),
  PasswordLogin { login: iced::button::State, player: iced::text_input::State, password: iced::text_input::State },
  Realm { realm_name: String, pane: iced::pane_grid::State<RealmPaneState> },
  ServerSelection { quit: iced::button::State, connect: iced::button::State, server: iced::text_input::State },
  Waiting,
}

enum RealmPaneState {
  Game,
  Tabs,
}

impl RealmPaneState {
  fn view<C: ServerConnection>(&mut self) -> iced::Element<'_, UpdateMessage<C>> {
    todo!()
  }
}

#[derive(Debug, derivative::Derivative)]
#[derivative(Clone(bound = ""))]
enum UpdateMessage<C: ServerConnection> {
  Response(puzzleverse_core::ClientResponse),
  Tick(std::time::Instant),
  ConnectToServer,
  Quit,
  UpdatePassword(String),
  UpdatePlayer(String),
  AuthenticateWithPassword,
  Connect(std::sync::Arc<C>),
  ShowConnectionError(String, bool),
  UpdateServer(String),
  ShowLoginPassword,
  NoOp,
}
impl<C: ServerConnection> From<Result<puzzleverse_core::AuthScheme, String>> for UpdateMessage<C> {
  fn from(result: Result<puzzleverse_core::AuthScheme, String>) -> Self {
    match result {
      Err(err) => UpdateMessage::ShowConnectionError(err, false),
      Ok(auth) => match auth {
        puzzleverse_core::AuthScheme::Password => UpdateMessage::ShowLoginPassword,
      },
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Configuration {
  server: Option<String>,
  user: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  let dirs = directories::ProjectDirs::from("", "", "puzzleverse").unwrap();
  let mut server = ServerParameter::UserDefined;
  let mut insecure = true;
  let mut user = ServerParameter::UserDefined;
  let mut login_file = std::path::PathBuf::new();
  login_file.extend(dirs.config_dir());
  login_file.push("login.json");
  if let Ok(login_handle) = std::fs::File::open(&login_file) {
    if let Ok(config) = serde_json::from_reader::<_, Configuration>(login_handle) {
      if let Some(last_server) = config.server {
        server = ServerParameter::Default(last_server);
      }
      if let Some(last_user) = config.user {
        user = ServerParameter::Default(last_user);
      }
    }
  }
  {
    struct StoreDefault;
    impl std::str::FromStr for ServerParameter {
      type Err = String;
      fn from_str(s: &str) -> Result<Self, String> {
        Ok(ServerParameter::Default(s.to_string()))
      }
    }

    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("Puzzleverse Client");

    ap.refer(&mut server).add_option(&["-s", "--server"], argparse::Store, "Set the server to a fixed value");
    ap.refer(&mut user).add_option(&["-u", "--user"], argparse::Store, "Set the username to a fixed value");
    ap.refer(&mut insecure).add_option(&["-i", "--insecure"], argparse::StoreTrue, "Use HTTP instead HTTPS");
    ap.parse_args_or_exit();
  }
  use iced::Application;
  Client::<TungsteniteConnection, _>::run(iced::Settings {
    window: iced::window::Settings {
      size: (1024, 768),
      min_size: None,
      max_size: None,
      resizable: true,
      decorations: false,
      transparent: false,
      always_on_top: false,
      icon: None,
    },
    flags: ClientConfiguration { server, user, insecure, store: puzzleverse_core::asset_store::FileSystemStore::new(dirs.cache_dir(), &[4, 4, 4]) },
    default_font: None,
    default_text_size: 14,
    antialiasing: true,
  })
  .unwrap();
}
#[cfg(target_arch = "wasm32")]
fn main() {
  unimplemented!()
}
