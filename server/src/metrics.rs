use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, LabelValueEncoder};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use std::fmt::Error;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct SharedString(pub Arc<str>);

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct BuildLabel {
  pub build_id: &'static str,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct PlayerLabel {
  pub player: SharedString,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct PeerLabel {
  pub peer: SharedString,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct SettingLabel {
  pub setting: &'static str,
}

impl EncodeLabelValue for SharedString {
  fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), Error> {
    EncodeLabelValue::encode(&self.0.as_ref(), encoder)
  }
}
lazy_static::lazy_static! {
    pub(crate) static ref BUILD_ID_MON: Family<BuildLabel, Counter>  = Default::default();
}
lazy_static::lazy_static! {
    pub(crate) static ref BAD_CLIENT_REQUESTS: Family<PlayerLabel, Counter> = Default::default();
}

lazy_static::lazy_static! {
    pub(crate) static ref BAD_JWT: Family<(), Counter> = Default::default();
}
lazy_static::lazy_static! {
    pub(crate) static ref BAD_PEER_REQUESTS: Family<PeerLabel, Counter> = Default::default();
}
lazy_static::lazy_static! {
    pub(crate) static ref BAD_PEER_SEND: Family<PeerLabel, Counter> = Default::default();

}
lazy_static::lazy_static! {
    pub(crate) static ref BAD_WEB_REQUEST: Family<(), Counter> = Default::default();

}
lazy_static::lazy_static! {
    pub(crate) static ref FAILED_SERVER_CALLBACK: Family<PeerLabel, Counter> = Default::default();
}
lazy_static::lazy_static! {
    pub(crate) static ref SETTING: crate::prometheus_locks::PrometheusLabelled<crate::prometheus_locks::rwlock::RwLockStatus<SettingLabel>> = Default::default();
}

pub(crate) fn register(registry: &mut prometheus_client::registry::Registry) {
  registry.register("spadina_build_id", "Current server build ID.", BUILD_ID_MON.clone());
  registry.register("spadina_bad_client_requests", "Number of client requests that couldn't be decoded.", BAD_CLIENT_REQUESTS.clone());
  registry.register("spadina_bad_jwt", "Number of times a bad JWT was received from a client or server.", BAD_JWT.clone());
  registry.register("spadina_bad_peer_requests", "Number of peer requests that couldn't be decoded.", BAD_PEER_REQUESTS.clone());
  registry.register("spadina_bad_peer_send", "Number of peer messages that couldn't be sent.", BAD_PEER_SEND.clone());
  registry.register("spadina_bad_web_request", "Number of invalid HTTP requests.", BAD_WEB_REQUEST.clone());
  registry.register(
    "spadina_failed_server_callback",
    "Number of times a server asked for a connection and then failed to be accessible.",
    FAILED_SERVER_CALLBACK.clone(),
  );
  SETTING.register(registry, "server_setting", "server-level setting");
  BUILD_ID_MON.get_or_create(&BuildLabel { build_id: git_version::git_version!() }).inc();
}
