pub const ACCESS_PATH: &str = "/api/access";
pub const AUTH_METHOD_PATH: &str = "/api/auth/method";
pub const CALENDAR_PATH: &str = "/api/calendar";
pub const CAPABILITY_HEADER: &str = "X-Spadina-Capability";
pub const CLIENT_KEY_PATH: &str = "/api/client/key";
pub const CLIENT_NONCE_PATH: &str = "/api/client/nonce";
pub const CLIENT_V1_PATH: &str = "/api/client/v1";
pub const KERBEROS_AUTH_PATH: &str = "/api/auth/kerberos";
pub const KERBEROS_PRINCIPAL_PATH: &str = "/api/auth/kerberos-principal";
pub const OIDC_AUTH_FINISH_PATH: &str = "/api/auth/oidc/finish";
pub const OIDC_AUTH_START_PATH: &str = "/api/auth/oidc/start";
pub const PASSWORD_AUTH_PATH: &str = "/api/auth/password";

///A format for the link to subscribe to a calendar
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarLink {
  /// A link to a `webcal://` URL, which is the standard for desktop applications
  WebCal,
  /// A link to subsribe using Outlook Live
  Outlook,
  /// A link to subsribe using Google Calendar
  Google,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde_with::serde_as]
pub struct CalendarQuery<S: AsRef<str>> {
  #[serde(default)]
  pub in_directory: bool,
  #[serde_as(as = "serde_with::json::JsonString")]
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub realms: Vec<crate::realm::LocalRealmTarget<S>>,
  #[serde_as(as = "serde_with::base64::Base64")]
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub id: Vec<u8>,
}

#[derive(Debug)]
#[pin_project::pin_project(project = IncomingConnectionProjection)]
pub enum IncomingConnection {
  Upgraded(#[pin] hyper::upgrade::Upgraded),
  Unix(#[pin] tokio::net::UnixStream),
}

pub trait ToWebMessage: serde::Serialize {
  fn as_wsm(&self) -> tokio_tungstenite::tungstenite::Message {
    tokio_tungstenite::tungstenite::Message::Binary(rmp_serde::to_vec(self).unwrap())
  }
}

pub fn has_domain_suffix(domain: &str, server: &str) -> bool {
  server.ends_with(domain) && (server.len() == domain.len() || &server[(server.len() - domain.len() - 1)..(server.len() - domain.len())] == ".")
}

/// Parse and normalize a server name
pub fn parse_server_name(server_name: &str) -> Option<String> {
  match idna::domain_to_unicode(server_name) {
    (name, Ok(())) => Some(name),
    _ => None,
  }
}
impl CalendarLink {
  /// Create a calendar link for the public calendar of a server and any desired realms
  pub fn create_link(&self, server: &str, query: &CalendarQuery<impl AsRef<str> + serde::Serialize>) -> String {
    let realm_query = serde_urlencoded::to_string(query).expect("Failed to URL encode calendar");
    match self {
      CalendarLink::WebCal => {
        format!("webcal://{}/{}{}", server, CALENDAR_PATH, realm_query)
      }
      CalendarLink::Google => format!(
        "https://calendar.google.com/calendar/r?pli=1&cid=https://{}/{}{}",
        server,
        CALENDAR_PATH,
        percent_encoding::percent_encode(realm_query.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
      ),
      CalendarLink::Outlook => {
        format!("https://outlook.live.com/owa/?path=/calendar/action/compose&rru=addsubscription&url=https://{}/{}{}&name=Spadina%20Events%20for%20{}&mkt=en-001", server,  CALENDAR_PATH, percent_encoding::percent_encode(realm_query.as_bytes(), percent_encoding::NON_ALPHANUMERIC),  server)
      }
    }
  }
}

impl tokio::io::AsyncRead for IncomingConnection {
  fn poll_read(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_read(cx, buf),
      IncomingConnectionProjection::Unix(u) => u.poll_read(cx, buf),
    }
  }
}
impl tokio::io::AsyncWrite for IncomingConnection {
  fn poll_write(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_write(cx, buf),
      IncomingConnectionProjection::Unix(u) => u.poll_write(cx, buf),
    }
  }

  fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_flush(cx),
      IncomingConnectionProjection::Unix(u) => u.poll_flush(cx),
    }
  }

  fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
    match self.project() {
      IncomingConnectionProjection::Upgraded(u) => u.poll_shutdown(cx),
      IncomingConnectionProjection::Unix(u) => u.poll_shutdown(cx),
    }
  }
}
impl From<hyper::upgrade::Upgraded> for IncomingConnection {
  fn from(value: hyper::upgrade::Upgraded) -> Self {
    IncomingConnection::Upgraded(value)
  }
}
impl From<tokio::net::UnixStream> for IncomingConnection {
  fn from(value: tokio::net::UnixStream) -> Self {
    IncomingConnection::Unix(value)
  }
}
