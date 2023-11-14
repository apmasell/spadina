use crate::database::location_scope::{LocationListScope, LocationScope};
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use crate::directory::Directory;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::http;
use icalendar::Component;
use icalendar::EventLike;
use spadina_core::location::directory::Visibility;
use spadina_core::location::target::{AbsoluteTarget, LocalTarget, UnresolvedTarget};
use spadina_core::net::calendar::CalendarQuery;
use spadina_core::reference_converter::AsReference;
use spadina_core::resource::Resource;

pub fn build_calendar(query: Option<&str>, database: &Database, directory: &Directory) -> http::Result<http::Response<Full<Bytes>>> {
  let (filters, calendar_id) = match query {
    Some(query) => match serde_urlencoded::from_str::<CalendarQuery<String>>(query) {
      Ok(query) => {
        let mut filters: Vec<_> = query
          .locations
          .into_iter()
          .map(|LocalTarget { owner, descriptor }| LocationListScope::Exact(LocationScope { owner: PlayerReference::Name(owner), descriptor }))
          .collect();
        if query.in_directory {
          filters.push(LocationListScope::Visibility(vec![Visibility::Public]));
        }

        (filters, if query.id.is_empty() { None } else { Some(query.id) })
      }
      Err(e) => return http::Response::builder().status(http::StatusCode::BAD_REQUEST).body(e.to_string().into()),
    },
    None => (Vec::new(), None),
  };

  let mut calendar = icalendar::Calendar::new().name(&format!("Events for {} Spadina", &directory.access_management.server_name)).done();

  let logged_in = match calendar_id.as_ref() {
    None => false,
    Some(calendar_id) => database.calendar_check(calendar_id.as_slice()).unwrap_or_else(|e| {
      eprintln!("Failed to check calendar ID: {}", e);
      false
    }),
  };

  for announcement in directory.access_management.announcements.read() {
    if announcement.public || logged_in {
      let mut event = icalendar::Event::new();
      event.summary(&announcement.title);
      event.description(&announcement.body);
      add_time(&announcement.when, &mut event);
      event.url(&match &announcement.location {
        UnresolvedTarget::Absolute(AbsoluteTarget { descriptor, owner, server }) => Resource::Location(UnresolvedTarget::Absolute(AbsoluteTarget {
          descriptor: descriptor.reference(AsReference::<str>::default()),
          owner: owner.as_ref(),
          server: server.as_ref(),
        }))
        .to_string(),
        UnresolvedTarget::NoWhere => Resource::Server(&directory.access_management.server_name).to_string(),
        UnresolvedTarget::Personal { asset } => Resource::Location(UnresolvedTarget::Personal { asset: asset.as_ref() }).to_string(),
      });
      calendar.push(event.done());
    }
  }
  if !filters.is_empty() || calendar_id.is_some() {
    match database.location_announcements_fetch_all(LocationListScope::Or(filters), calendar_id, &directory.access_management.server_name) {
      Ok(announcements) => {
        for (target, announcement) in announcements {
          let mut event = icalendar::Event::new();
          event.summary(&announcement.title);
          event.description(&announcement.body);
          add_time(&announcement.when, &mut event);
          event.url(&Resource::Location(target.into()).to_string());
          calendar.push(event.done());
        }
      }
      Err(e) => {
        eprintln!("Failed to get realm announcements for calendar: {}", e);
      }
    }
  }

  http::Response::builder().header("Content-Type", "text/calendar").body(calendar.to_string().into())
}
fn add_time(start: &spadina_core::communication::AnnouncementTime, event: &mut icalendar::Event) {
  match start {
    spadina_core::communication::AnnouncementTime::Until(date) => {
      event.starts(*date);
    }
    spadina_core::communication::AnnouncementTime::Starts(start, minutes) => {
      event.starts(*start).ends(*start + chrono::Duration::minutes(*minutes as i64));
    }
  }
}
