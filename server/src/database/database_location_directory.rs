use crate::access::AccessManagement;
use crate::database::location_scope::LocationScope;
use crate::database::player_reference::PlayerReference;
use crate::database::{database_location, Database};
use crate::directory::location_endpoint::LocationEndpoint;
use crate::directory::Directory;
use crate::join_request::JoinRequest;
use crate::player_location_update::PlayerLocationUpdate;
use crate::server_controller_template::ServerControllerTemplate;
use rand::{thread_rng, RngCore};
use spadina_core::location::change::LocationChangeResponse;
use spadina_core::location::directory::Activity;
use spadina_core::location::target::LocalTarget;
use spadina_core::location::{Descriptor, DescriptorKind};
use spadina_core::player::PlayerIdentifier;
use spadina_core::reference_converter::{AsReference, ToClone};
use spadina_core::shared_ref::SharedRef;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

pub enum DatabaseLocationRequest {
  Activity(LocalTarget<SharedRef<str>>, oneshot::Sender<Activity>),
  Create(DescriptorKind<SharedRef<str>>, JoinRequest),
  Join(LocalTarget<SharedRef<str>>, JoinRequest),
}

enum Event {
  Resolve(DatabaseLocationRequest),
  Quit,
}

pub type DatabaseLocationDirectory = mpsc::Sender<DatabaseLocationRequest>;

pub fn start(auth: &AccessManagement, database: Database, directory: Directory, mut rx: mpsc::Receiver<DatabaseLocationRequest>) {
  let mut death = auth.give_me_death();
  tokio::spawn(async move {
    let mut active = HashMap::<LocalTarget<SharedRef<str>>, LocationEndpoint>::new();
    'main: loop {
      let message: Event = tokio::select! {
          _ = death.recv() => Event::Quit,
          r = rx.recv() => r.map(Event::Resolve).unwrap_or(Event::Quit),
      };
      let Some((target, join_request)) = (match message {
        Event::Resolve(DatabaseLocationRequest::Activity(target, output)) => {
          let _ = output.send(match active.get(&target).map(|endpoint| endpoint.activity()) {
            Some(a) => a,
            None => match database.location_find(LocationScope { owner: PlayerReference::Name(target.owner), descriptor: target.descriptor }) {
              Ok(None) => Activity::Unknown,
              Ok(Some(_)) => Activity::Deserted,
              Err(e) => {
                eprintln!("Failed to find location: {}", e);
                Activity::Unknown
              }
            },
          });
          None
        }
        Event::Resolve(DatabaseLocationRequest::Create(target, join_request)) => {
          let PlayerIdentifier::Local(player) = &join_request.name else {
            let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::PermissionError));
            continue;
          };
          let player = player.clone();
          match target {
            DescriptorKind::Asset(asset) => {
              let asset = asset.into_arc();
              if !active.contains_key(&LocalTarget {
                descriptor: Descriptor::Asset(SharedRef::Shared(asset.clone())),
                owner: SharedRef::Shared(player.clone()),
              }) && {
                match database.location_find(LocationScope { owner: PlayerReference::Name(&*player), descriptor: Descriptor::Asset(&*asset) }) {
                  Ok(None) => true,
                  Ok(Some(_)) => false,
                  Err(e) => {
                    let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
                    eprintln!("Failed to find location: {}", e);
                    continue;
                  }
                }
              } {
                let endpoint = database_location::find_and_create_realm(player.clone(), asset.clone(), database.clone(), directory.clone()).await;
                active.insert(
                  LocalTarget { owner: SharedRef::Shared(player.clone()), descriptor: Descriptor::Asset(SharedRef::Shared(asset.clone())) },
                  endpoint,
                );
              }
              Some((LocalTarget { owner: SharedRef::Shared(player), descriptor: Descriptor::Asset(SharedRef::Shared(asset)) }, join_request))
            }
            DescriptorKind::Application(application) => {
              let mut attempts = 0;
              let id = loop {
                attempts += 1;
                if attempts > 20 {
                  eprintln!("Too many attempts to find available ID for {:?} for {}", application, &player);
                  let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
                  break 'main;
                }
                let id = thread_rng().next_u32();
                match database
                  .location_find(LocationScope { owner: PlayerReference::Name(&*player), descriptor: Descriptor::Application(application, id) })
                {
                  Ok(None) => {
                    break id;
                  }
                  Ok(Some(_)) => continue,
                  Err(e) => {
                    eprintln!("Failed to find available ID: {}", e);
                    let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
                    break 'main;
                  }
                }
              };
              let endpoint = database_location::create_location(
                ServerControllerTemplate::Application(application),
                player.clone(),
                Descriptor::Application(application, id),
                &database,
                &directory,
              )
              .await;
              active.insert(LocalTarget { owner: SharedRef::Shared(player.clone()), descriptor: Descriptor::Application(application, id) }, endpoint);
              Some((LocalTarget { owner: SharedRef::Shared(player.clone()), descriptor: Descriptor::Application(application, id) }, join_request))
            }
            DescriptorKind::Unsupported(_) => {
              let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::UnsupportedError));
              None
            }
          }
        }
        Event::Resolve(DatabaseLocationRequest::Join(target, join_request)) => Some((target, join_request)),
        Event::Quit => break,
      }) else {
        continue;
      };
      match active.entry(target) {
        Entry::Occupied(o) => match o.get().join(join_request) {
          Ok(()) => (),
          Err(join_request) => {
            let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
            o.remove();
          }
        },
        Entry::Vacant(entry) => {
          match database.location_find(LocationScope {
            owner: PlayerReference::Name(entry.key().owner.as_ref()),
            descriptor: entry.key().descriptor.reference(AsReference::<str>::default()),
          }) {
            Ok(None) => {
              let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::ResolutionError));
            }
            Ok(Some(id)) => {
              let owner = {
                let SharedRef::Shared(owner) = &entry.key().owner else {
                  continue;
                };
                owner.clone()
              };
              let descriptor = entry.key().descriptor.reference(ToClone::<str>::default());
              if let Err(join_request) =
                entry.insert(database_location::load(id, owner, descriptor, database.clone(), directory.clone())).join(join_request)
              {
                let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
              }
            }
            Err(e) => {
              eprintln!("Failed to find location: {}", e);
              let _ = join_request.tx.try_send(PlayerLocationUpdate::ResolveUpdate(LocationChangeResponse::InternalError));
            }
          }
        }
      }
    }
  });
}
