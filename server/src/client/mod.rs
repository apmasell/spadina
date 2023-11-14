use crate::accounts::policy::Policy;
use crate::aggregating_map::AggregatingMap;
use crate::database::location_scope::{LocationListScope, LocationScope};
use crate::database::persisted::PersistedLocal;
use crate::database::player_persistence::{PlayerAvatar, PlayerDefaultLocationAccess, PlayerMessageAccess, PlayerOnlineAccess};
use crate::database::player_reference::PlayerReference;
use crate::database::Database;
use crate::directory::player_directory::PlayerRequest;
use crate::directory::Directory;
use crate::http_server::jwt::PlayerClaim;
use crate::location_search;
use crate::metrics::{PlayerLabel, SharedString};
use crate::player_event::PlayerEvent;
use crate::socket_entity::{ConnectionState, Incoming, Outgoing, SocketEntity};
use chrono::{Duration, Utc};
use diesel::QueryResult;
use futures::StreamExt;
use futures::{FutureExt, Stream};
use spadina_core::access::{AccessSetting, BulkLocationSelector, OnlineAccess};
use spadina_core::communication::DirectMessage;
use spadina_core::location::change::{LocationChangeRequest, LocationChangeResponse};
use spadina_core::location::directory::Activity;
use spadina_core::location::target::{AbsoluteTarget, LocalTarget, UnresolvedTarget};
use spadina_core::location::DescriptorKind;
use spadina_core::net::mixed_connection::MixedConnection;
use spadina_core::net::server::administration::{AdministrationRequest, AdministrationResponse};
use spadina_core::net::server::{AssetError, ClientRequest, ClientResponse};
use spadina_core::player::{OnlineState, PlayerIdentifier};
use spadina_core::reference_converter::{AsArc, AsReference, AsShared, AsSingle, ForPacket};
use spadina_core::shared_ref::SharedRef;
use spadina_core::{communication, UpdateResult};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio_stream::wrappers::WatchStream;
use tokio_tungstenite::WebSocketStream;

mod hosting;
mod idle_timer;
mod incremental_search;
mod location;
mod online;

pub struct Client {
  avatar: PersistedLocal<PlayerAvatar>,
  calendar_id: Vec<u8>,
  current_location: location::Location,
  db_id: i32,
  default_location_acl: PersistedLocal<PlayerDefaultLocationAccess>,
  idle_timer: idle_timer::IdleTimer,
  message_acl: PersistedLocal<PlayerMessageAccess>,
  name: Arc<str>,
  online_acl: PersistedLocal<PlayerOnlineAccess>,
}

impl Stream for Client {
  type Item = location::LocationEvent;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    self.get_mut().current_location.poll_next_unpin(cx)
  }
}

impl SocketEntity for Client {
  const DIRECTORY_QUEUE_DEPTH: usize = 100;
  type Claim = PlayerClaim<String>;
  type DirectoryRequest = PlayerRequest;
  type ExternalRequest = ClientRequest<String, Vec<u8>>;

  fn establish(claim: Self::Claim, connection: WebSocketStream<MixedConnection>, directory: Directory) -> impl Future<Output = Result<(), ()>> {
    async move { directory.register_player(Arc::from(claim.name), connection).await }
  }

  fn new(name: Arc<str>, database: &Database) -> QueryResult<Self> {
    let (db_id, calendar_id) = database.player_load(&name)?;
    Ok(Client {
      avatar: PersistedLocal::new(database.clone(), PlayerAvatar(db_id))?,
      calendar_id,
      current_location: location::Location::NoWhere,
      db_id,
      default_location_acl: PersistedLocal::new(database.clone(), PlayerDefaultLocationAccess(db_id))?,
      idle_timer: Default::default(),
      message_acl: PersistedLocal::new(database.clone(), PlayerMessageAccess(db_id))?,
      name,
      online_acl: PersistedLocal::new(database.clone(), PlayerOnlineAccess(db_id))?,
    })
  }
  async fn process(
    &mut self,
    incoming: Incoming<Self>,
    directory: &Directory,
    database: &Database,
    connection_state: ConnectionState,
  ) -> Vec<Outgoing<Self>> {
    let is_superuser = connection_state == ConnectionState::ConnectedUnix;
    self.idle_timer.active(connection_state != ConnectionState::Disconnected);
    match incoming {
      Incoming::Delayed(location::LocationEvent::IdleTimeout) => vec![Outgoing::Break],
      Incoming::Delayed(location::LocationEvent::Message(message)) => vec![Outgoing::Send(message)],
      Incoming::Delayed(location::LocationEvent::Redirect(redirect)) => {
        let location = match redirect {
          UnresolvedTarget::NoWhere => {
            self.current_location = location::Location::NoWhere;
            LocationChangeResponse::NoWhere
          }
          UnresolvedTarget::Absolute(target) => match target.localize(&directory.access_management.server_name) {
            Some((target, server)) => {
              let (location, join_request) = self.current_location.start_join(is_superuser, self.name.clone(), self.avatar.read().clone());
              match server {
                None => directory.join_location(target, join_request).await,
                Some(server) => directory.join_location_on_peer(target, SharedRef::Single(server), join_request).await,
              }

              location
            }
            None => {
              self.current_location = location::Location::NoWhere;
              LocationChangeResponse::NoWhere
            }
          },
          UnresolvedTarget::Personal { asset } => {
            let (location, join_request) = self.current_location.start_join(is_superuser, self.name.clone(), self.avatar.read().clone());
            directory.create_location(DescriptorKind::Asset(asset), join_request).await;
            location
          }
        };
        vec![Outgoing::Send(ClientResponse::<_, &[u8]>::LocationChange { location }.into())]
      }
      Incoming::Directory(PlayerRequest::Check(requester, output)) => {
        let _ = output.send(match self.online_acl.read().check(&requester, &directory.access_management.server_name) {
          OnlineAccess::Location => self.current_location.get_state().convert(AsShared::default()),
          OnlineAccess::OnlineOnly => OnlineState::Online,
          OnlineAccess::Deny => OnlineState::Unknown,
        });
        vec![]
      }
      Incoming::Directory(PlayerRequest::Connect(connection)) => vec![Outgoing::Connect(connection)],
      Incoming::Directory(PlayerRequest::DirectMessage(player, body, timestamp)) => vec![Outgoing::Send(
        ClientResponse::<_, &[u8]>::DirectMessage {
          player: player.reference(AsReference::<str>::default()),
          message: DirectMessage { inbound: false, body: body.reference(AsReference::<str>::default()), timestamp },
        }
        .into(),
      )],
      Incoming::External(ClientRequest::Activity { id, player }) => {
        let result = match player.localize(&directory.access_management.server_name) {
          PlayerIdentifier::Local(player) => directory.check_host_activity(SharedRef::Single(player)).await,
          PlayerIdentifier::Remote { player, server } => {
            directory.check_host_activity_on_peer(SharedRef::Single(server), SharedRef::Single(player)).await
          }
        };
        let output = match result {
          Ok(activity) => Outgoing::Send(ClientResponse::<String, &[u8]>::Activity { id, activity }.into()),
          Err(rx) => Outgoing::SideTask(
            async move {
              let message = Outgoing::Send(ClientResponse::<String, &[u8]>::Activity { id, activity: rx.await.unwrap_or(Activity::Unknown) }.into());
              vec![message]
            }
            .into_stream()
            .boxed(),
          ),
        };
        vec![output]
      }
      Incoming::External(ClientRequest::Administration { id, request }) => {
        let directory = directory.clone();
        let player_name = self.name.clone();
        let task = Outgoing::SideTask(
          async move {
            let message = Outgoing::Send(
              ClientResponse::<_, &[u8]>::Administration {
                id,
                response: if is_superuser || directory.access_management.accounts.is_administrator(&player_name).await {
                  match request {
                    AdministrationRequest::AccountLockChange { .. } => todo!(),
                    AdministrationRequest::AccountLockStatus { .. } => todo!(),
                    AdministrationRequest::Invite => todo!(),
                  }
                } else {
                  AdministrationResponse::<String>::NotAdministrator
                },
              }
              .into(),
            );
            vec![message]
          }
          .into_stream()
          .boxed(),
        );
        vec![task]
      }
      Incoming::External(ClientRequest::AccessGetDefault) => {
        let AccessSetting { rules, default } = self.default_location_acl.read();
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CurrentAccessDefault { rules: rules.clone(), default: *default }.into())]
      }
      Incoming::External(ClientRequest::AccessGetDirectMessage) => {
        let AccessSetting { rules, default } = self.message_acl.read();
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CurrentAccessDirectMessage { rules: rules.clone(), default: *default }.into())]
      }
      Incoming::External(ClientRequest::AccessGetOnline) => {
        let AccessSetting { rules, default } = self.online_acl.read();
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CurrentAccessOnline { rules: rules.clone(), default: *default }.into())]
      }
      Incoming::External(ClientRequest::AccessSetDirectMessage { rules, default, id }) => {
        let rules = rules.into_iter().filter_map(|rule| rule.into_local(&directory.access_management.server_name)).collect();
        let response = Outgoing::Send(
          ClientResponse::<String, Vec<u8>>::AccessChange {
            id,
            result: self.message_acl.mutate(|settings| {
              settings.default = default;
              settings.rules = rules;
              UpdateResult::Success
            }),
          }
          .into(),
        );
        vec![response]
      }
      Incoming::External(ClientRequest::AccessSetDefault { id, rules, default }) => {
        vec![Outgoing::Send(
          ClientResponse::<String, Vec<u8>>::AccessChange {
            id,
            result: self.default_location_acl.mutate(|settings| {
              settings.default = default;
              settings.rules = rules;
              UpdateResult::Success
            }),
          }
          .into(),
        )]
      }
      Incoming::External(ClientRequest::AccessSetLocationBulk { id, selection, rules, default }) => {
        let filter = match selection {
          BulkLocationSelector::AllMine => Some(LocationListScope::Owner(PlayerReference::Id(self.db_id))),
          BulkLocationSelector::AllForOther { player } => {
            if player.as_str() == &*self.name {
              Some(LocationListScope::Owner(PlayerReference::Id(self.db_id)))
            } else if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::Owner(PlayerReference::Name(SharedRef::Single(player))))
            } else {
              None
            }
          }
          BulkLocationSelector::AllServer => {
            if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::All)
            } else {
              None
            }
          }
          BulkLocationSelector::MineByDescriptor { descriptors } => Some(LocationListScope::Or(
            descriptors
              .into_iter()
              .map(|descriptor| {
                LocationListScope::Exact(LocationScope {
                  owner: PlayerReference::Id(self.db_id),
                  descriptor: descriptor.convert(AsSingle::<str>::default()),
                })
              })
              .collect(),
          )),
          BulkLocationSelector::OtherPlayerByDescriptor { descriptors, player } => {
            if player.as_str() == &*self.name {
              Some(LocationListScope::Or(
                descriptors
                  .into_iter()
                  .map(|descriptor| {
                    LocationListScope::Exact(LocationScope {
                      owner: PlayerReference::Id(self.db_id),
                      descriptor: descriptor.convert(AsSingle::<str>::default()),
                    })
                  })
                  .collect(),
              ))
            } else if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              let player: Arc<str> = player.into();
              Some(LocationListScope::Or(
                descriptors
                  .into_iter()
                  .map(|descriptor| {
                    LocationListScope::Exact(LocationScope {
                      owner: PlayerReference::Name(SharedRef::Shared(player.clone())),
                      descriptor: descriptor.convert(AsSingle::<str>::default()),
                    })
                  })
                  .collect(),
              ))
            } else {
              None
            }
          }
          BulkLocationSelector::MineByKind { kind } => Some(LocationListScope::And(vec![
            LocationListScope::Owner(PlayerReference::Id(self.db_id)),
            LocationListScope::Kind(kind.convert(AsSingle::<str>::default())),
          ])),
          BulkLocationSelector::OtherPlayerByKind { kind, player } => {
            if player.as_str() == &*self.name {
              Some(LocationListScope::And(vec![
                LocationListScope::Owner(PlayerReference::Id(self.db_id)),
                LocationListScope::Kind(kind.convert(AsSingle::<str>::default())),
              ]))
            } else if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::And(vec![
                LocationListScope::Owner(PlayerReference::Name(SharedRef::Single(player))),
                LocationListScope::Kind(kind.convert(AsSingle::<str>::default())),
              ]))
            } else {
              None
            }
          }
        };
        let result = {
          match filter {
            Some(filter) => match database.location_acl_write_bulk(filter, &AccessSetting { rules, default }) {
              Ok(()) => UpdateResult::Success,
              Err(e) => {
                eprintln!("Failed to bulk update realm ACLs: {}", e);
                UpdateResult::InternalError
              }
            },
            None => UpdateResult::NotAllowed,
          }
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::AccessChange { id, result }.into())]
      }
      Incoming::External(ClientRequest::AccessSetOnline { id, rules, default }) => {
        vec![Outgoing::Send(
          ClientResponse::<String, Vec<u8>>::AccessChange {
            id,
            result: self.online_acl.mutate(|settings| {
              settings.default = default;
              settings.rules = rules;
              UpdateResult::Success
            }),
          }
          .into(),
        )]
      }

      Incoming::External(ClientRequest::AssetPull { id, principal }) => {
        let result = match directory.pull_asset(principal.into(), true).await {
          Ok(rx) => Outgoing::SideTask(
            async move {
              let message = Outgoing::Send(match rx.await {
                Ok(asset) => ClientResponse::Asset { id, asset: asset.reference(ForPacket) }.into(),
                Err(_) => ClientResponse::<&str, &[u8]>::AssetUnavailable { id }.into(),
              });
              vec![message]
            }
            .into_stream()
            .boxed(),
          ),
          Err(()) => Outgoing::Send(ClientResponse::<&str, &[u8]>::AssetUnavailable { id }.into()),
        };
        vec![result]
      }
      Incoming::External(ClientRequest::AnnouncementAdd { id, announcement }) => {
        let now = Utc::now();
        let result = if announcement.when.expires() < now {
          UpdateResult::NotAllowed
        } else if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          let communication::Announcement { title, body, when, location, public } = announcement;
          directory.access_management.announcements.write(|announcements| {
            announcements.push(communication::Announcement {
              title: Arc::from(title),
              body: Arc::from(body),
              when,
              location: location.convert(AsArc::<str>::default()),
              public,
            });
            announcements.retain(|a| a.when.expires() > now);
          })
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::AnnouncementUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::AnnouncementClear { id }) => {
        let result = if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          directory.access_management.announcements.write(|announcements| announcements.clear())
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::AnnouncementUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::AnnouncementList) => {
        vec![Outgoing::Send(ClientResponse::<_, &[u8]>::Announcements { announcements: directory.access_management.announcements.read() }.into())]
      }
      Incoming::External(ClientRequest::AssetUpload { id, asset }) => {
        let directory = directory.clone();
        let player_name = self.name.clone();
        let task = Outgoing::SideTask(
          async move {
            if directory.access_management.accounts.can_create(&player_name).await {
              let result = Outgoing::Send(match directory.push_asset(asset).await {
                Ok(()) => ClientResponse::<String, Vec<u8>>::AssetCreationSucceeded { id }.into(),
                Err(error) => ClientResponse::<String, Vec<u8>>::AssetCreationFailed { id, error }.into(),
              });
              vec![result]
            } else {
              vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::AssetCreationFailed { id, error: AssetError::PermissionError }.into())]
            }
          }
          .into_stream()
          .boxed(),
        );
        vec![task]
      }
      Incoming::External(ClientRequest::AvatarGet) => {
        let message = Outgoing::Send(ClientResponse::<String, Vec<u8>>::AvatarCurrent { avatar: self.avatar.read().clone() }.into());
        vec![message]
      }
      Incoming::External(ClientRequest::AvatarSet { id, avatar }) => {
        let result = self.avatar.mutate(|a| {
          *a = avatar.clone();
          UpdateResult::Success
        });
        let mut outgoing = vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::AvatarUpdate { id, result }.into())];
        if result == UpdateResult::Success {
          if let Err(message) = self.current_location.send(PlayerEvent::Avatar(avatar.clone())).await {
            outgoing.push(Outgoing::Send(message));
          }
        }
        outgoing
      }
      Incoming::External(ClientRequest::BookmarkAdd { id, bookmark }) => {
        let success = match database.bookmark_add(self.db_id, &bookmark.localize(&directory.access_management.server_name)) {
          Err(e) => {
            eprintln!("Failed to write bookmark to database for {}: {}", &self.name, e);
            false
          }
          Ok(_) => true,
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::BookmarkUpdate { id, success }.into())]
      }
      Incoming::External(ClientRequest::BookmarkRemove { id, bookmark }) => {
        let success = match database.bookmark_rm(self.db_id, &bookmark.localize(&directory.access_management.server_name)) {
          Err(e) => {
            eprintln!("Failed to delete bookmark from database for {}: {}", &self.name, e);
            false
          }
          Ok(_) => true,
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::BookmarkUpdate { id, success }.into())]
      }
      Incoming::External(ClientRequest::BookmarksList) => match database.bookmark_get(self.db_id, |b| Some(b)) {
        Err(e) => {
          eprintln!("Failed to get bookmarks for {}: {}", &self.name, e);
          vec![Outgoing::Send(ClientResponse::<String, &[u8]>::Bookmarks { bookmarks: Default::default() }.into())]
        }
        Ok(bookmarks) => {
          vec![Outgoing::Send(ClientResponse::<_, &[u8]>::Bookmarks { bookmarks }.into())]
        }
      },
      Incoming::External(ClientRequest::CalendarIdentifier) => {
        vec![Outgoing::Send(ClientResponse::<String, &[u8]>::Calendar { id: self.calendar_id.as_slice() }.into())]
      }
      Incoming::External(ClientRequest::CalendarReset) => match database.calendar_reset(PlayerReference::<String>::Id(self.db_id)) {
        Ok(id) => {
          let message = Outgoing::Send(ClientResponse::<String, _>::Calendar { id: id.as_slice() }.into());
          self.calendar_id = id;
          vec![message]
        }
        Err(e) => {
          eprintln!("Failed to reset calendar link for {}: {}", &self.name, e);
          vec![]
        }
      },
      Incoming::External(ClientRequest::CalendarLocationAdd { id, location }) => {
        let success = match location.localize(&directory.access_management.server_name) {
          None => false,
          Some((location, None)) => match database.calendar_add(self.db_id, &location) {
            Ok(_) => true,
            Err(e) => {
              eprintln!("Failed to add subscription for {} to {:?}: {}", &self.name, &location, e);
              false
            }
          },
          Some((LocalTarget { descriptor, owner }, Some(server))) => {
            let location = AbsoluteTarget { descriptor, owner, server };
            match database.calendar_add_remote(self.db_id, &location) {
              Ok(_) => true,
              Err(e) => {
                eprintln!("Failed to add subscription for {} to {:?}: {}", &self.name, &location, e);
                false
              }
            }
          }
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CalendarLocationChange { id, success }.into())]
      }
      Incoming::External(ClientRequest::CalendarLocationClear { id }) => {
        let success = match database.calendar_rm_all(self.db_id) {
          Ok(_) => true,
          Err(e) => {
            eprintln!("Failed to remove all subscriptions for {}: {}", &self.name, e);
            false
          }
        };

        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CalendarLocationChange { id, success }.into())]
      }
      Incoming::External(ClientRequest::CalendarLocationList) => {
        let message = Outgoing::Send(
          ClientResponse::<_, &[u8]>::CalendarLocations {
            locations: match database.calendar_list(self.db_id) {
              Ok(locations) => locations,
              Err(e) => {
                eprintln!("Failed to list subscriptions for {}: {}", &self.name, e);
                Vec::new()
              }
            },
          }
          .into(),
        );
        vec![message]
      }
      Incoming::External(ClientRequest::CalendarLocationRemove { id, location }) => {
        let success = match location.localize(&directory.access_management.server_name) {
          None => false,
          Some((location, None)) => match database.calendar_rm(self.db_id, &location) {
            Ok(_) => true,
            Err(e) => {
              eprintln!("Failed to remove subscription for {} to {:?}: {}", &self.name, &location, e);
              false
            }
          },
          Some((LocalTarget { descriptor, owner }, Some(server))) => {
            let location = AbsoluteTarget { descriptor, owner, server };
            match database.calendar_rm_remote(self.db_id, &location) {
              Ok(_) => true,
              Err(e) => {
                eprintln!("Failed to remote subscription for {} to {:?}: {}", &self.name, &location, e);
                false
              }
            }
          }
        };

        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::CalendarLocationChange { id, success }.into())]
      }
      Incoming::External(ClientRequest::DirectMessageGet { player, from, to }) => {
        let player = player.localize(&directory.access_management.server_name);
        let messages = match &player {
          PlayerIdentifier::Local(name) => database.direct_message_get(self.db_id, name, &from, &to),
          PlayerIdentifier::Remote { player: player_name, server: peer_server } => {
            database.remote_direct_message_get(self.db_id, player_name, peer_server)
          }
        }
        .unwrap_or_else(|e| {
          eprintln!("Failed to fetch messages between {} and {}: {}", &self.name, &player, e);
          Vec::new()
        });
        vec![Outgoing::Send(ClientResponse::<_, &[u8]>::DirectMessages { player, from, to, messages }.into())]
      }
      Incoming::External(ClientRequest::DirectMessageSend { id, recipient, body }) => {
        match directory
          .send_dm(
            recipient.localize(&directory.access_management.server_name).convert(AsSingle::<str>::default()),
            PlayerIdentifier::Local(SharedRef::Shared(self.name.clone())),
            body,
          )
          .await
        {
          Err(status) => vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::DirectMessageReceipt { id, status }.into())],
          Ok(rx) => vec![Outgoing::SideTask(
            WatchStream::new(rx)
              .map(move |status| vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::DirectMessageReceipt { id, status }.into())])
              .boxed(),
          )],
        }
      }
      Incoming::External(ClientRequest::DirectMessageStats) => {
        let (stats, last_login) = database.direct_message_stats(self.db_id).unwrap_or_else(|e| {
          eprintln!("Failed to fetch messages stats for {}: {}", &self.name, e);
          (Default::default(), Utc::now())
        });

        vec![Outgoing::Send(ClientResponse::<_, &[u8]>::DirectMessageStats { stats, last_login }.into())]
      }
      Incoming::External(ClientRequest::InLocation { request }) => match self.current_location.send(PlayerEvent::Request(request)).await {
        Ok(()) => Vec::new(),
        Err(message) => vec![Outgoing::Send(message)],
      },
      Incoming::External(ClientRequest::LocationChange { location }) => {
        let location = match location {
          LocationChangeRequest::Location(target) => {
            let (location, join_request) = self.current_location.start_join(is_superuser, self.name.clone(), self.avatar.read().clone());
            let (target, server) = target.into_local();
            if *server == *directory.access_management.server_name {
              directory.join_location(target.convert(AsSingle::<str>::default()), join_request).await
            } else {
              directory.join_location_on_peer(target.convert(AsSingle::<str>::default()), SharedRef::Single(server), join_request).await
            }
            location
          }
          LocationChangeRequest::New(descriptor) => {
            let (location, join_request) = self.current_location.start_join(is_superuser, self.name.clone(), self.avatar.read().clone());
            directory.create_location(descriptor.convert(AsSingle::<str>::default()), join_request).await;
            location
          }
          LocationChangeRequest::Guest(host) => {
            let (location, join_request) = self.current_location.start_join(is_superuser, self.name.clone(), self.avatar.read().clone());
            match host.localize(&directory.access_management.server_name) {
              PlayerIdentifier::Local(host) => directory.join_host(SharedRef::Single(host), join_request).await,
              PlayerIdentifier::Remote { player, server } => directory.join_host_on_peer(player, server, join_request).await,
            }
            location
          }
          LocationChangeRequest::Host { descriptor, rules, default } => {
            if is_superuser || directory.access_management.accounts.can_create(&self.name).await {
              self
                .current_location
                .start_hosting(
                  self.name.clone(),
                  descriptor.convert(AsArc::<str>::default()),
                  self.avatar.read().clone(),
                  AccessSetting { rules, default },
                  &directory,
                )
                .await
            } else {
              self.current_location = location::Location::NoWhere;
              LocationChangeResponse::PermissionError
            }
          }
          LocationChangeRequest::NoWhere => {
            self.current_location = location::Location::NoWhere;
            LocationChangeResponse::NoWhere
          }
        };
        let response = Outgoing::Send(ClientResponse::<_, &[u8]>::LocationChange { location }.into());
        vec![response]
      }
      Incoming::External(ClientRequest::LocationChangeVisibility { id, visibility, selection }) => {
        let filter = match selection {
          BulkLocationSelector::AllMine => Some(LocationListScope::Owner(PlayerReference::Id(self.db_id))),
          BulkLocationSelector::AllForOther { player } => {
            if &*player == self.name.as_ref() || is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::Owner(PlayerReference::Name(SharedRef::Single(player))))
            } else {
              None
            }
          }
          BulkLocationSelector::AllServer => {
            if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::All)
            } else {
              None
            }
          }
          BulkLocationSelector::MineByDescriptor { descriptors } => Some(LocationListScope::Or(
            descriptors
              .into_iter()
              .map(|descriptor| {
                LocationListScope::Exact(LocationScope {
                  owner: PlayerReference::Id(self.db_id),
                  descriptor: descriptor.convert(AsSingle::<str>::default()),
                })
              })
              .collect(),
          )),
          BulkLocationSelector::OtherPlayerByDescriptor { descriptors, player } => {
            if &*player == self.name.as_ref() || is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              let player: Arc<str> = player.into();
              Some(LocationListScope::Or(
                descriptors
                  .into_iter()
                  .map(|descriptor| {
                    LocationListScope::Exact(LocationScope {
                      owner: PlayerReference::Name(SharedRef::Shared(player.clone())),
                      descriptor: descriptor.convert(AsSingle::<str>::default()),
                    })
                  })
                  .collect(),
              ))
            } else {
              None
            }
          }
          BulkLocationSelector::MineByKind { kind } => Some(LocationListScope::And(vec![
            LocationListScope::Owner(PlayerReference::Id(self.db_id)),
            LocationListScope::Kind(kind.convert(AsSingle::<str>::default())),
          ])),
          BulkLocationSelector::OtherPlayerByKind { kind, player } => {
            if &*player == self.name.as_ref() || is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
              Some(LocationListScope::And(vec![
                LocationListScope::Owner(PlayerReference::Name(SharedRef::Single(player))),
                LocationListScope::Kind(kind.convert(AsSingle::<str>::default())),
              ]))
            } else {
              None
            }
          }
        };
        let result = match filter {
          Some(filter) => match database.location_change_visibility(visibility, filter) {
            Ok(()) => UpdateResult::Success,
            Err(e) => {
              eprintln!("Failed to trash locations for {}: {}", &self.name, e);
              UpdateResult::InternalError
            }
          },
          None => UpdateResult::NotAllowed,
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::LocationVisibility { id, result }.into())]
      }
      Incoming::External(ClientRequest::PlayerOnlineCheck { id, player }) => match player.localize(&directory.access_management.server_name) {
        PlayerIdentifier::Local(name) => {
          if name.as_str() == &*self.name {
            let result = Outgoing::Send(ClientResponse::<_, &[u8]>::PlayerOnlineState { id, state: self.current_location.get_state() }.into());
            vec![result]
          } else {
            online::watch_online(
              id,
              directory.check_online(PlayerIdentifier::Local(SharedRef::Shared(self.name.clone())), SharedRef::Single(name)).await,
            )
          }
        }
        PlayerIdentifier::Remote { server, player } => {
          online::watch_online(id, directory.check_online_on_peer(self.name.clone(), server, player).await)
        }
      },
      Incoming::External(ClientRequest::PlayerReset { id, player }) => {
        let result = if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          match database.player_reset(&player) {
            Err(e) => {
              eprintln!("Failed to delete player {}: {}", &player, e);
              UpdateResult::InternalError
            }
            Ok(_) => UpdateResult::Success,
          }
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PlayerReset { id, result }.into())]
      }
      Incoming::External(ClientRequest::PublicKeyAdd { id, der }) => {
        let result = match openssl::pkey::PKey::public_key_from_der(&der) {
          Err(_) => UpdateResult::NotAllowed,
          Ok(_) => match database.public_key_add(self.db_id, &der) {
            Ok(_) => UpdateResult::Success,
            Err(e) => {
              eprintln!("Failed to add public key: {}", e);
              UpdateResult::InternalError
            }
          },
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PublicKeyUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::PublicKeyDelete { id, name }) => {
        let result = match database.public_key_rm(self.db_id, &name) {
          Err(e) => {
            eprintln!("Failed to delete public key: {}", e);
            UpdateResult::InternalError
          }
          Ok(_) => UpdateResult::Success,
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PublicKeyUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::PublicKeyDeleteAll { id }) => {
        let result = match database.public_key_rm_all(self.db_id) {
          Err(e) => {
            eprintln!("Failed to delete all public keys: {}", e);
            UpdateResult::InternalError
          }
          Ok(_) => UpdateResult::Success,
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PublicKeyUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::PublicKeyList) => vec![Outgoing::Send(
        ClientResponse::<_, &[u8]>::PublicKeys {
          keys: database.public_key_list(self.db_id).unwrap_or_else(|e| {
            eprintln!("Failed to send data to player {}: {}", &self.name, e);
            BTreeMap::new()
          }),
        }
        .into(),
      )],
      Incoming::External(ClientRequest::LocationsList { id, source, timeout }) => {
        let timeout = Duration::seconds(timeout.clamp(5, 60) as i64);
        let recipient = incremental_search::SearchRequest(id);
        match incremental_search::ReifiedSearch::convert(source, &self.name, self.db_id, &directory.access_management.server_name) {
          incremental_search::ReifiedSearch::Bookmarks => {
            location_search::combined_locations(
              recipient,
              database.bookmark_get::<_, AggregatingMap<String, Vec<_>>>(self.db_id, location_search::resource_to_target).map(|m| m.0),
              &self.name,
              database,
              directory,
              timeout,
            )
            .await
          }
          incremental_search::ReifiedSearch::Calendar => {
            location_search::local_results(recipient, database.calendar_list_entries(self.db_id, &directory.access_management.server_name), directory)
          }
          incremental_search::ReifiedSearch::Database(scopes, requires_admin) => {
            if requires_admin && !(is_superuser || directory.access_management.accounts.is_administrator(&self.name).await) {
              let result = Outgoing::Send(ClientResponse::<String, &[u8]>::LocationsUnavailable { id, server: None }.into());
              vec![result]
            } else {
              location_search::local_query(recipient, scopes, database, directory)
            }
          }
          incremental_search::ReifiedSearch::Remote(server, query) => location_search::remote_locations(recipient, server, query, directory, timeout),
        }
      }
      Incoming::External(ClientRequest::PeerBanAdd { id, ban }) => {
        let result = if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          directory.access_management.banned_peers.write("client_add_ban", |banned| Some(!banned.insert(ban))).await
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PeersBannedUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::PeerBanClear { id }) => {
        let result = if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          directory
            .access_management
            .banned_peers
            .write("client_clear_ban", |banned| {
              if banned.is_empty() {
                Some(false)
              } else {
                banned.clear();
                Some(true)
              }
            })
            .await
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PeersBannedUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::PeerBanList) => {
        vec![Outgoing::Send(
          ClientResponse::<_, &[u8]>::PeersBanned {
            bans: directory.access_management.banned_peers.read("client_list_ban", |bans| bans.clone()).await,
          }
          .into(),
        )]
      }
      Incoming::External(ClientRequest::PeerBanRemove { id, ban }) => {
        let result = if is_superuser || directory.access_management.accounts.is_administrator(&self.name).await {
          directory.access_management.banned_peers.write("client_remove_ban", |banned| Some(banned.remove(&ban))).await
        } else {
          UpdateResult::NotAllowed
        };
        vec![Outgoing::Send(ClientResponse::<String, Vec<u8>>::PeersBannedUpdate { id, result }.into())]
      }
      Incoming::External(ClientRequest::Peers) => {
        let response = match directory.peers().await {
          Err(()) => Outgoing::Send(ClientResponse::<String, &[u8]>::Peers { peers: Vec::new() }.into()),
          Ok(rx) => Outgoing::SideTask(
            async move {
              let message = Outgoing::Send(ClientResponse::<_, &[u8]>::Peers { peers: rx.await.unwrap_or_default() }.into());
              vec![message]
            }
            .into_stream()
            .boxed(),
          ),
        };
        vec![response]
      }
      Incoming::External(ClientRequest::FromHost { request }) => {
        self.current_location.send_host_command(request).await;
        vec![]
      }
      Incoming::StateChange => vec![],
    }
  }

  fn show_decode_error(&self, error: rmp_serde::decode::Error) {
    eprintln!("Decode error for {}: {}", &self.name, error);
    crate::metrics::BAD_CLIENT_REQUESTS.get_or_create(&PlayerLabel { player: SharedString(self.name.clone()) }).inc();
  }

  fn show_socket_error(&self, error: tokio_tungstenite::tungstenite::Error) {
    eprintln!("Socket error for {}: {:?}", &self.name, error);
    crate::metrics::BAD_CLIENT_REQUESTS.get_or_create(&PlayerLabel { player: SharedString(self.name.clone()) }).inc();
  }
}
