pub mod calendar;
pub mod mixed_connection;
pub mod server;

pub const OIDC_AUTH_START_PATH: &str = "/api/auth/oidc/start";

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
