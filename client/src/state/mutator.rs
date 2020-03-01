use std::hash::Hash;

#[derive(Debug)]
pub struct Mutator<M: Mutable>(super::Shared<std::collections::HashMap<M::Name, Result<M::Output, M::Failure>>>);
pub struct MutatorRef<'a, M: Mutable, S> {
  client: &'a super::ServerConnection<S>,
  creator: &'a Mutator<M>,
}
pub trait Mutable {
  const OPERATION: super::InflightOperation;
  type Failure;
  type Name: Copy + Hash + Eq;
  type Output;
  fn into_operation(self, id: i32) -> (Self::Name, spadina_core::ClientRequest<String>);
}

impl<M: Mutable> Mutator<M> {
  pub(crate) fn capture<'a, S>(&'a self, client: &'a super::ServerConnection<S>) -> MutatorRef<'a, M, S> {
    MutatorRef { client, creator: self }
  }
  pub(crate) fn finish(&self, name: M::Name, value: Result<M::Output, M::Failure>) {
    self.0.lock().unwrap().insert(name, value);
  }
}

impl<'a, M: Mutable, S> MutatorRef<'a, M, S> {
  pub fn push(&self, value: M) -> M::Name {
    let id = self.client.cache_state.add_operation(M::OPERATION.clone());
    let (name, request) = value.into_operation(id);
    self.client.outbound_tx.send(crate::state::connection::ServerRequest::Deliver(request)).unwrap();
    name
  }
  pub fn try_remove(&self, name: M::Name) -> Option<Result<M::Output, M::Failure>> {
    self.creator.0.lock().unwrap().remove(&name)
  }
}

impl<C: Mutable> Default for Mutator<C> {
  fn default() -> Self {
    Self(Default::default())
  }
}
impl<C: Mutable> Clone for Mutator<C> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct AssetUpload(pub(crate) i32);
#[derive(Debug)]
pub struct AssetUploadRequest {
  pub asset_type: String,
  pub name: String,
  pub tags: Vec<String>,
  pub licence: spadina_core::asset::Licence,
  pub compression: spadina_core::asset::Compression,
  pub data: Vec<u8>,
}

impl Mutable for AssetUploadRequest {
  const OPERATION: super::InflightOperation = super::InflightOperation::AssetCreation;

  type Failure = spadina_core::AssetError;
  type Name = AssetUpload;
  type Output = String;

  fn into_operation(self, id: i32) -> (Self::Name, spadina_core::ClientRequest<String>) {
    (
      AssetUpload(id),
      spadina_core::ClientRequest::AssetCreate {
        id,
        asset_type: self.asset_type,
        name: self.name,
        tags: self.tags,
        licence: self.licence,
        compression: self.compression,
        data: self.data,
      },
    )
  }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Invitation(pub(crate) i32);
impl Mutable for () {
  const OPERATION: super::InflightOperation = super::InflightOperation::InvitationCreation;
  type Failure = spadina_core::communication::InvitationError;
  type Name = Invitation;
  type Output = String;

  fn into_operation(self, id: i32) -> (Self::Name, spadina_core::ClientRequest<String>) {
    (Invitation(id), spadina_core::ClientRequest::Invite { id })
  }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct RealmDeletion(pub(crate) i32);

impl Mutable for String {
  const OPERATION: super::InflightOperation = super::InflightOperation::RealmDeletion;

  type Failure = bool;

  type Name = RealmDeletion;

  type Output = ();

  fn into_operation(self, id: i32) -> (Self::Name, spadina_core::ClientRequest<String>) {
    (RealmDeletion(id), spadina_core::ClientRequest::RealmDelete { id, asset: self, owner: None })
  }
}
impl Mutable for spadina_core::realm::LocalRealmTarget<String> {
  const OPERATION: super::InflightOperation = super::InflightOperation::RealmDeletion;

  type Failure = ();

  type Name = RealmDeletion;

  type Output = ();

  fn into_operation(self, id: i32) -> (Self::Name, spadina_core::ClientRequest<String>) {
    (RealmDeletion(id), spadina_core::ClientRequest::RealmDelete { id, asset: self.asset, owner: Some(self.owner) })
  }
}
