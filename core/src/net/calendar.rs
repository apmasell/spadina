use crate::net::server::CALENDAR_PATH;

///A format for the link to subscribe to a calendar
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarLink {
  /// A link to a `webcal://` URL, which is the standard for desktop applications
  WebCal,
  /// A link to subscribe using Outlook Live
  Outlook,
  /// A link to subscribe using Google Calendar
  Google,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CalendarQuery<S: AsRef<str> + serde::Serialize + serde::de::DeserializeOwned> {
  #[serde(default)]
  pub in_directory: bool,
  #[serde_as(as = "serde_with::json::JsonString")]
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub locations: Vec<crate::location::target::LocalTarget<S>>,
  #[serde_as(as = "serde_with::base64::Base64")]
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub id: Vec<u8>,
}

impl CalendarLink {
  /// Create a calendar link for the public calendar of a server and any desired realms
  pub fn create_link(&self, server: &str, query: &CalendarQuery<impl AsRef<str> + serde::Serialize + serde::de::DeserializeOwned>) -> String {
    let encoded_query = serde_urlencoded::to_string(query).expect("Failed to URL encode calendar");
    match self {
      CalendarLink::WebCal => {
        format!("webcal://{}/{}{}", server, CALENDAR_PATH, encoded_query)
      }
      CalendarLink::Google => format!(
        "https://calendar.google.com/calendar/r?pli=1&cid=https://{}/{}{}",
        server,
        CALENDAR_PATH,
        percent_encoding::percent_encode(encoded_query.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
      ),
      CalendarLink::Outlook => {
        format!("https://outlook.live.com/owa/?path=/calendar/action/compose&rru=addsubscription&url=https://{}/{}{}&name=Spadina%20Events%20for%20{}&mkt=en-001", server, CALENDAR_PATH, percent_encoding::percent_encode(encoded_query.as_bytes(), percent_encoding::NON_ALPHANUMERIC), server)
      }
    }
  }
}
