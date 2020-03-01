use spadina_core::net::ToWebMessage;
pub(crate) struct SelfHosted {
  capabilities: std::collections::BTreeSet<&'static str>,
  acl: crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
  identifier: crate::destination::SharedPlayerId,
  local_server: std::sync::Arc<str>,
  messages: std::collections::VecDeque<spadina_core::location::LocationMessage<String>>,
  player: std::collections::HashMap<spadina_core::player::PlayerIdentifier<crate::shstr::ShStr>, crate::realm::puzzle::PlayerKey>,
  rx: tokio::sync::mpsc::Receiver<<Self as futures::Stream>::Item>,
  tx: tokio::sync::mpsc::Sender<crate::client::InternalClientRequest>,
}

impl crate::destination::Owner for crate::destination::SharedPlayerId {
  fn owner(&self) -> &std::sync::Arc<str> {
    self.get_player()
  }
}

impl SelfHosted {
  pub(crate) fn new(
    acl: crate::access::AccessSetting<spadina_core::access::SimpleAccess>,
    name: std::sync::Arc<str>,
    local_server: std::sync::Arc<str>,
    capabilities: std::collections::BTreeSet<&'static str>,
    rx: tokio::sync::mpsc::Receiver<<Self as futures::Stream>::Item>,
    tx: tokio::sync::mpsc::Sender<crate::client::InternalClientRequest>,
  ) -> Self {
    SelfHosted {
      acl,
      capabilities,
      identifier: spadina_core::player::PlayerIdentifier::Local(name),
      local_server: local_server,
      messages: Default::default(),
      player: Default::default(),
      rx,
      tx,
    }
  }
  async fn send_event(
    &mut self,
    event: spadina_core::self_hosted::HostEvent<impl AsRef<str> + std::hash::Hash + std::cmp::Ord + std::cmp::Eq + serde::Serialize>,
  ) -> Vec<crate::destination::DestinationControl<<Self as crate::destination::Destination>::Response>> {
    match self.tx.send(crate::client::InternalClientRequest::Message(spadina_core::ClientResponse::ToHost { event }.as_wsm())).await {
      Ok(()) => Vec::new(),
      Err(_) => vec![crate::destination::DestinationControl::Quit],
    }
  }
}

#[async_trait::async_trait]
impl crate::destination::Destination for SelfHosted {
  type Identifier = crate::destination::SharedPlayerId;

  type Request = spadina_core::self_hosted::GuestRequest<crate::shstr::ShStr>;

  type Response = spadina_core::self_hosted::GuestResponse<crate::shstr::ShStr>;
  fn capabilities(&self) -> &std::collections::BTreeSet<&'static str> {
    &self.capabilities
  }

  async fn consensual_emote(
    &mut self,
    _: &crate::realm::puzzle::PlayerKey,
    requester: &super::SharedPlayerId,
    _: &crate::realm::puzzle::PlayerKey,
    target: &super::SharedPlayerId,
    emote: std::sync::Arc<str>,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    self
      .send_event(spadina_core::self_hosted::HostEvent::ConsensualEmote {
        initiator: requester.as_ref(),
        recipient: target.as_ref(),
        emote: emote.as_ref(),
      })
      .await
  }
  fn delete(&mut self, _: Option<super::SharedPlayerId>) -> spadina_core::UpdateResult {
    spadina_core::UpdateResult::NotAllowed
  }

  async fn follow(
    &mut self,
    _: &crate::realm::puzzle::PlayerKey,
    requester: &super::SharedPlayerId,
    _: &crate::realm::puzzle::PlayerKey,
    target: &super::SharedPlayerId,
  ) -> Vec<crate::destination::DestinationControl<Self::Response>> {
    self.send_event(spadina_core::self_hosted::HostEvent::Follow { requester: requester.as_ref(), target: target.as_ref() }).await
  }

  fn get_messages(
    &self,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
  ) -> Vec<spadina_core::location::LocationMessage<String>> {
    self.messages.iter().filter(|m| m.timestamp >= from && m.timestamp <= to).cloned().collect()
  }

  async fn handle(
    &mut self,
    _: &crate::realm::puzzle::PlayerKey,
    player: &super::SharedPlayerId,
    _: bool,
    request: Self::Request,
  ) -> Vec<super::DestinationControl<Self::Response>> {
    self.send_event(spadina_core::self_hosted::HostEvent::PlayerRequest { player: player.clone().convert_str(), request }).await
  }

  async fn process_events(&mut self, events: <Self as futures::Stream>::Item) -> Vec<super::DestinationControl<Self::Response>> {
    match events {
      spadina_core::self_hosted::HostCommand::Broadcast { response } => Some(super::DestinationControl::Broadcast(response)),
      spadina_core::self_hosted::HostCommand::Move { player, target } => {
        self.player.get(&player).map(|key| super::DestinationControl::Move(key.clone(), target.map(|t| t.convert_str())))
      }
      spadina_core::self_hosted::HostCommand::MoveTrain { player, owner, train } => {
        self.player.get(&player).map(|key| super::DestinationControl::MoveTrain(key.clone(), owner.to_arc(), train))
      }
      spadina_core::self_hosted::HostCommand::Quit => Some(super::DestinationControl::Quit),
      spadina_core::self_hosted::HostCommand::Response { player, response } => {
        self.player.get(&player).map(|key| super::DestinationControl::Response(key.clone(), response))
      }
      spadina_core::self_hosted::HostCommand::SendMessage { body } => {
        self.send_message(None, &self.identifier.clone(), &body).await.map(|timestamp| {
          super::DestinationControl::SendMessage(spadina_core::location::LocationMessage {
            sender: self.identifier.clone(),
            body: body.convert_str(),
            timestamp,
          })
        })
      }
      spadina_core::self_hosted::HostCommand::UpdateAccess { default, rules } => {
        self.acl.default = default;
        self.acl.rules = rules;
        None
      }
    }
    .into_iter()
    .collect()
  }

  fn quit(&mut self) {}

  async fn remove_player(
    &mut self,
    _: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
  ) -> Vec<super::DestinationControl<Self::Response>> {
    self.player.remove(&player.clone().convert_str());
    self.send_event(spadina_core::self_hosted::HostEvent::PlayerLeft { player: player.as_ref() }).await
  }

  async fn send_message(
    &mut self,
    _: Option<&crate::realm::puzzle::PlayerKey>,
    player: &super::SharedPlayerId,
    body: &spadina_core::communication::MessageBody<
      impl AsRef<str> + serde::Serialize + std::fmt::Debug + std::cmp::PartialEq + std::cmp::Eq + Sync + Into<std::sync::Arc<str>>,
    >,
  ) -> Option<chrono::DateTime<chrono::Utc>> {
    let timestamp = chrono::Utc::now();
    if !body.is_transient() {
      self.messages.push_back(spadina_core::location::LocationMessage { body: body.as_owned_str(), sender: player.as_owned_str(), timestamp });
    }
    if let Err(_) = self
      .tx
      .send(crate::client::InternalClientRequest::Message(
        spadina_core::ClientResponse::LocationMessagePosted { sender: player.as_ref(), body: body.as_ref(), timestamp }.as_wsm(),
      ))
      .await
    {
      eprintln!("Failed to send message to host");
    }
    Some(timestamp)
  }

  async fn try_add(
    &mut self,
    key: &crate::realm::puzzle::PlayerKey,
    player: &spadina_core::player::PlayerIdentifier<std::sync::Arc<str>>,
    _: bool,
  ) -> Result<(spadina_core::location::LocationResponse<crate::shstr::ShStr>, Vec<super::DestinationControl<Self::Response>>), ()> {
    match self.acl.check(player, &self.local_server) {
      spadina_core::access::SimpleAccess::Allow => {
        let result = Ok((
          spadina_core::location::LocationResponse::Guest { host: self.identifier.clone().convert_str() },
          self.send_event(spadina_core::self_hosted::HostEvent::PlayerEntered { player: player.as_ref() }).await,
        ));
        self.player.insert(player.clone().convert_str(), key.clone());
        result
      }

      spadina_core::access::SimpleAccess::Deny => Err(()),
    }
  }
}
impl futures::Stream for SelfHosted {
  type Item = spadina_core::self_hosted::HostCommand<crate::shstr::ShStr>;

  fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
    self.get_mut().rx.poll_recv(cx)
  }
}
