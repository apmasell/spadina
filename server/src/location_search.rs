use crate::database::location_scope::{LocationListScope, LocationScope};
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use crate::directory::Directory;
use crate::peer::message::PeerLocationSearch;
use crate::socket_entity::{Outgoing, SocketEntity};
use diesel::QueryResult;
use futures::{stream, FutureExt, Stream, StreamExt};
use futures_batch::ChunksTimeoutStreamExt;
use serde::Serialize;
use spadina_core::location::directory::{Activity, DirectoryEntry};
use spadina_core::location::target::{AbsoluteTarget, LocalTarget, UnresolvedTarget};
use spadina_core::reference_converter::AsShared;
use spadina_core::resource::Resource;
use spadina_core::shared_ref::SharedRef;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::WatchStream;
use tokio_tungstenite::tungstenite::Message;

pub fn resource_to_target(resource: Resource<String>) -> Option<(String, LocalTarget<String>)> {
  match resource {
    Resource::Location(location) => match location {
      UnresolvedTarget::Absolute(AbsoluteTarget { descriptor, owner, server }) => Some((server, LocalTarget { descriptor, owner })),
      UnresolvedTarget::NoWhere => None,
      UnresolvedTarget::Personal { .. } => None,
    },
    _ => None,
  }
}

pub fn local_locations<LR: LocationRecipient>(
  recipient: LR,
  targets: Vec<LocalTarget<impl AsRef<str> + Debug>>,
  database: &Database,
  directory: &Directory,
) -> Vec<Outgoing<LR::Receiver>>
where
  <<LR as LocationRecipient>::Receiver as Stream>::Item: Send + 'static,
{
  local_query(
    recipient,
    LocationListScope::Or(
      targets
        .into_iter()
        .map(|LocalTarget { descriptor, owner }| LocationListScope::Exact(LocationScope { owner: PlayerReference::Name(owner), descriptor }))
        .collect(),
    ),
    database,
    directory,
  )
}

pub trait LocationRecipient: Clone + Copy + Send + Sync + 'static
where
  <Self::Receiver as Stream>::Item: Send + 'static,
{
  type Receiver: SocketEntity + ?Sized;
  fn encode(&self, locations: Vec<DirectoryEntry<impl AsRef<str> + Eq + Hash + Ord + Serialize>>) -> Message;
  fn fail(&self) -> Message;
}

pub fn remote_locations<LR: LocationRecipient>(
  recipient: LR,
  server: String,
  query: PeerLocationSearch<String>,
  directory: &Directory,
  timeout: chrono::Duration,
) -> Vec<Outgoing<LR::Receiver>>
where
  <<LR as LocationRecipient>::Receiver as Stream>::Item: Send + 'static,
{
  let directory = directory.clone();
  let task = Outgoing::SideTask(
    async move {
      match directory.search_on_peer(server, timeout, query).await {
        Ok(watch) => {
          let task = Outgoing::SideTask(
            WatchStream::new(watch)
              .map(move |locations| {
                if locations.is_empty() {
                  vec![]
                } else {
                  let message = Outgoing::Send(recipient.encode(locations));
                  vec![message]
                }
              })
              .boxed(),
          );
          vec![task]
        }
        Err(()) => vec![],
      }
    }
    .into_stream()
    .boxed(),
  );
  vec![task]
}

pub fn local_query<LR: LocationRecipient>(
  recipient: LR,
  scope: LocationListScope<impl AsRef<str> + Debug>,
  database: &Database,
  directory: &Directory,
) -> Vec<Outgoing<LR::Receiver>>
where
  <<LR as LocationRecipient>::Receiver as Stream>::Item: Send + 'static,
{
  local_results(recipient, database.location_list(&directory.access_management.server_name, scope), directory)
}

pub fn local_results<LR: LocationRecipient>(
  recipient: LR,
  result: QueryResult<Vec<DirectoryEntry<Arc<str>>>>,
  directory: &Directory,
) -> Vec<Outgoing<LR::Receiver>>
where
  <<LR as LocationRecipient>::Receiver as Stream>::Item: Send + 'static,
{
  match result {
    Ok(locations) => {
      let directory = directory.clone();
      let task = Outgoing::SideTask(
        stream::iter(locations)
          .map(move |mut entry| {
            let directory = directory.clone();
            async move {
              entry.activity = match directory
                .check_activity(LocalTarget {
                  descriptor: entry.descriptor.clone().convert(AsShared::<str>::default()),
                  owner: SharedRef::Shared(entry.owner.clone()),
                })
                .await
              {
                Ok(activity) => activity,
                Err(rx) => rx.await.unwrap_or(Activity::Unknown),
              };
              entry
            }
          })
          .buffer_unordered(10)
          .chunks_timeout(20, Duration::from_secs(1))
          .map(move |locations| {
            let message = Outgoing::Send(recipient.encode(locations));
            vec![message]
          })
          .boxed(),
      );
      vec![task]
    }
    Err(e) => {
      eprintln!("Failed to search locally: {}", e);
      let message = Outgoing::Send(recipient.fail());
      vec![message]
    }
  }
}

pub async fn combined_locations<LR: LocationRecipient>(
  recipient: LR,
  locations: QueryResult<BTreeMap<String, Vec<LocalTarget<String>>>>,
  name: &str,
  database: &Database,
  directory: &Directory,
  timeout: chrono::Duration,
) -> Vec<Outgoing<LR::Receiver>>
where
  <<LR as LocationRecipient>::Receiver as Stream>::Item: Send + 'static,
{
  match locations {
    Err(e) => {
      eprintln!("Failed to fetch bookmarks for {}: {}", name, e);
      let message = Outgoing::Send(recipient.fail());
      vec![message]
    }
    Ok(mut targets) => {
      let mut output = Vec::new();
      if let Some(targets) = targets.remove(directory.access_management.server_name.as_ref()) {
        output.extend(local_locations(recipient.clone(), targets, &database, &directory));
      }
      output.extend(
        targets.into_iter().flat_map(|(server, locations)| {
          remote_locations(recipient.clone(), server, PeerLocationSearch::Specific { locations }, directory, timeout)
        }),
      );
      output
    }
  }
}
