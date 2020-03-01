pub mod gradiator;
pub mod puzzle;
pub mod rules;

use std::{fmt::Pointer, hash::Hash};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Angle {
  Fixed(u16),
  Noisy { offset: u16, noise: u16 },
  Oriented { x: u32, y: u32, offset: u16, noise: Option<u16> },
  Random,
}

pub struct AssetDetails {
  pub children: Vec<String>,
  pub capabilities: Vec<String>,
}
pub trait ExtractChildren<S: AsRef<str>> {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>);
}
pub trait AssetKind<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Clone>:
  serde::de::DeserializeOwned + ExtractChildren<S>
{
  const KIND: &'static str;
  type Resolved: ExtractChildren<S> + 'static;
  fn extract_capabilities(resolved: &Self::Resolved) -> Vec<String>;
  fn resolve(
    self,
    mapper: &mut impl ResourceMapper<
      S,
      S,
      S,
      Audio = Loaded<AssetAnyAudio, S>,
      Custom = Loaded<AssetAnyCustom<S>, S>,
      Error = crate::AssetError,
      Model = Loaded<AssetAnyModel, S>,
    >,
  ) -> Result<Self::Resolved, crate::AssetError>;
  fn check(resolved: &Self::Resolved) -> bool;
}
pub async fn verify_submission<
  T: AssetKind<S>,
  Store: crate::asset_store::AsyncAssetStore,
  S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Send + Sync + Clone + 'static,
>(
  store: &Store,
  compression: Compression,
  data: &[u8],
) -> Result<AssetDetails, crate::AssetError>
where
  for<'a> &'a str: Into<S>,
{
  let mut mapper = crate::asset_store::CachingResourceMapper::<S>::new();
  match compression.decompress::<T>(&data) {
    Err(_) => Err(crate::AssetError::DecodeFailure),
    Ok(value) => {
      let mut direct_children = Default::default();
      value.extract_children(&mut direct_children);
      for child in direct_children {
        mapper.install_from(store, child.clone()).await?;
      }
      let resolved = value.resolve(&mut mapper)?;
      if T::check(&resolved) {
        let mut children = std::collections::BTreeSet::new();
        resolved.extract_children(&mut children);
        Ok(AssetDetails { capabilities: T::extract_capabilities(&resolved), children: children.into_iter().map(|c| c.to_string()).collect() })
      } else {
        Err(crate::AssetError::Invalid)
      }
    }
  }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Argument<S: AsRef<str>> {
  Material(u32),
  Color(LocalBlendableValue<Color, S>),
  Intensity(LocalBlendableValue<f64, S>),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Aesthetic {
  Cartoon(Color),
  Emo,
  Flat,
  HandPainted,
  PixelArt,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct AmbientAudio<A, S: AsRef<str>> {
  pub sound: AmbientAudioSound<A>,
  pub volume: GlobalValue<f64, S>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AmbientAudioSound<A> {
  Asset(A),
  Static,
  Wind,
}

/// An asset header for a game asset; this is common across all assets
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Asset {
  /// The type of the asset. This is not an enumeration to allow for new asset types to be added in the future. Program should handle the case of an unknown asset gracefully.  pub asset_type: String,
  pub asset_type: String,
  /// The player that uploaded this asset, usually encoded as _player_`@`_server_, but not required.
  pub author: String,
  /// Any capabilties this asset or its children require
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub capabilities: Vec<String>,
  /// The child assets this asset depends on; this is here so that a server can quickly pull all the appropriate assets and inform clients about which assets the require. This is more important for large aggregate assets (_i.e._, realms) over small distinct ones (_e.g._, meshes) and may be empty.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub children: Vec<String>,
  /// The compression used on the data
  pub compression: Compression,
  /// The actual asset. The format of this data is arbitrary and only interpretable based on `asset_type`. Usually a common external format (_e.g._, PNG) or a Message Pack-encoded Rust data structure.
  pub data: Vec<u8>,
  /// The licence terms applied to this asset. Currently, all licences provided are compatible, so this is mostly informative unless the asset is going to be exported.
  pub licence: Licence,
  /// A friendly name for the asset. This would be used if the asset is to appear in tool palettes and the like.
  pub name: String,
  /// A list of friendly tags to describe this asset to allow for searching.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub tags: Vec<String>,
  /// The time the asset was written
  pub created: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Debug)]
pub enum AssetAnyAudio {}
#[derive(Clone, Debug)]
pub enum AssetAnyCustom<S: AsRef<str> + std::cmp::Ord + std::hash::Hash> {
  Simple(PuzzleCustom<Loaded<AssetAnyAudio, S>, Loaded<AssetAnyModel, S>, S>),
}
#[derive(Clone, Debug)]
pub enum AssetAnyModel {
  Simple(SimpleSprayModel<Mesh, u32, u32, u32>),
}
#[derive(Clone, Debug)]
pub enum AssetAnyRealm<S: AsRef<str> + std::hash::Hash + std::cmp::Ord> {
  Simple(SimpleRealmDescription<Loaded<AssetAnyAudio, S>, Loaded<AssetAnyModel, S>, Loaded<AssetAnyCustom<S>, S>, S>),
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compression {
  MessagePack,
  ZstdMessagePack,
}
pub enum CompressionError {
  MessagePack(rmp_serde::encode::Error),
  Io(std::io::Error),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CycleArgument<S: AsRef<str>> {
  Material(u32),
  CycleMaterial(u32, Vec<u32>, Transition),
  Color(LocalBlendableValue<Color, S>),
  CycleColor(Color, Vec<Color>, Transition),
  Intensity(LocalBlendableValue<f64, S>),
  CycleIntensity(f64, Vec<f64>, Transition),
}
pub enum DecompressionError {
  MessagePack(rmp_serde::decode::Error),
  Io(std::io::Error),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct EventAudio<A, S: AsRef<str>> {
  pub name: S,
  pub sound: A,
  pub volume: GlobalValue<f64, S>,
  pub x: u32,
  pub y: u32,
  pub z: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EventAudioSound<A> {
  Asset(A),
}
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum Color {
  Rgb(u8, u8, u8),
  Hsl(u8, u8, u8),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CustomValue<T, S: AsRef<str>> {
  Fixed(T),
  PuzzleBool { id: S, when_true: T, when_false: T, transition: Transition },
  PuzzleNum { id: S, default: T, values: Vec<T>, transition: Transition },
  Masked(S),
  Random(Vec<T>),
  SettingBool { id: S, when_true: T, when_false: T },
  SettingNum { id: S, default: T, values: Vec<T> },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GlobalValue<T, S: AsRef<str>> {
  Fixed(T),
  PuzzleBool { id: S, when_true: T, when_false: T, transition: Transition },
  PuzzleNum { id: S, default: T, values: Vec<T>, transition: Transition },
  Masked(S),
  Random(Vec<T>),
  Setting(S),
  SettingBool { id: S, when_true: T, when_false: T, transition: Transition },
  SettingNum { id: S, default: T, values: Vec<T>, transition: Transition },
}

/// The licences associated with an asset
///
/// Players should be reminded of the asset when exporting it. Since the system requires unfettered replication of assets between servers, all licenses must permit copying unmodified. All assets should be available to other world builders, so remixing must be supported by all licences.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Licence {
  CreativeCommons(LicenceUses),
  CreativeCommonsNoDerivatives(LicenceUses),
  CreativeCommonsShareALike(LicenceUses),
  PubDom,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LicenceUses {
  Commercial,
  NonCommercial,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Light<I, K> {
  Point { position: (f32, f32, f32), color: K, intensity: I },
}
#[derive(Debug)]
pub struct Loaded<T, S: AsRef<str>> {
  asset: S,
  value: std::sync::Arc<T>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocalDiscreteValue<T, S: AsRef<str>> {
  Global(GlobalValue<T, S>),
  RandomLocal(Vec<T>),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LocalBlendableValue<T, S: AsRef<str>> {
  Altitude { top_value: T, bottom_value: T, top_limit: u32, bottom_limit: u32 },
  Global(GlobalValue<T, S>),
  Gradiator(S),
  RandomLocal(Vec<T>),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum LogicElement {
  Arithemetic(puzzle::ArithmeticOperation),
  Buffer(u8, puzzle::ListType),
  Clock { period: u32, max: u32, shift: Option<u32> },
  Compare(puzzle::ComparatorOperation, puzzle::ComparatorType),
  Counter(u32),
  HolidayBrazil,
  HolidayEaster,
  HolidayUnitedStates,
  HolidayWeekends,
  IndexList(puzzle::ListType),
  Logic(puzzle::LogicOperation),
  Metronome(u32),
  Permutation(u8),
  Timer { frequency: u32, initial_counter: u32 },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Mask<T> {
  Marked(Vec<u8>, T),
  HasBit(u8, T),
  NotMarked(Vec<u8>, T),
  HasNotBit(u8, T),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MaskConfiguration {
  Bool {
    masks: Vec<Mask<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<(Color, Color, Transition)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intensity: Option<(f64, f64, Transition)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    material: Option<(u32, u32, Transition)>,
  },
  Num {
    masks: Vec<Mask<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: MaskNumeric<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intensity: MaskNumeric<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    material: MaskNumeric<u32>,
  },
}

pub type MaskNumeric<T> = Option<(T, Vec<T>, Transition)>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Material<C, I, B> {
  BrushedMetal { color: C },
  Crystal { color: C, opacity: I },
  Gem { color: C, accent: Option<C>, glow: B },
  Metal { color: C, corrosion: Option<(C, I)> },
  Rock { color: C },
  Sand { color: C },
  ShinyMetal { color: C },
  Soil { color: C },
  Textile { color: C },
  TreadPlate { color: C, corrosion: Option<C> },
  Wood { background: C, grain: C },
}
#[derive(Clone, Debug)]
pub enum ChoiceValue<'a, T> {
  Bool(&'a T, &'a T, Transition),
  Num(&'a T, &'a [T], Transition),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Mesh {
  Triangle { elements: Vec<(f32, f32, f32)> },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PlatformBase {
  Thin,
  Box { thickness: u32 },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct PlatformDescription<A, M, C, S: AsRef<str> + std::cmp::Ord> {
  pub base: PlatformBase,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub contents: Vec<PlatformItem<A, M, C, S>>,
  pub length: u32,
  pub material: u32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub sprays: Vec<u32>,
  pub spray_blank_weight: u8,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub walls: Vec<(u32, Vec<WallPath>)>,
  pub width: u32,
  pub x: u32,
  pub y: u32,
  pub z: u32,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct PlatformItem<A, M, C, S: AsRef<str> + std::cmp::Ord> {
  pub x: u32,
  pub y: u32,
  pub item: PuzzleItem<A, M, C, S>,
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Perturbation {
  Fixed,
  Offset(f32),
  Range(f32),
  Gaussian(f32),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>, S: Deserialize<'de>"))]
pub enum PuzzleItem<A, M, C, S: AsRef<str> + std::cmp::Ord> {
  Button {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<Argument<S>>,
    enabled: bool,
    matcher: rules::PlayerMarkMatcher,
    model: M,
    name: S,
    transformation: Transformation,
  },
  Switch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<SwitchArgument<S>>,
    enabled: bool,
    initial: bool,
    matcher: rules::PlayerMarkMatcher,
    model: M,
    name: S,
    transformation: Transformation,
  },
  CycleButton {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<CycleArgument<S>>,
    enabled: bool,
    matcher: rules::PlayerMarkMatcher,
    model: M,
    name: S,
    states: u32,
    transformation: Transformation,
  },
  CycleDisplay {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<CycleArgument<S>>,
    model: M,
    name: S,
    states: u32,
    transformation: Transformation,
  },
  RealmSelector {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<Argument<S>>,
    matcher: rules::PlayerMarkMatcher,
    model: M,
    name: S,
    transformation: Transformation,
  },
  Display {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<Argument<S>>,
    model: M,
    transformation: Transformation,
  },
  Custom {
    item: C,
    transformation: Transformation,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    gradiators_color: std::collections::BTreeMap<S, S>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    gradiators_intensity: std::collections::BTreeMap<S, S>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    materials: std::collections::BTreeMap<S, u32>,
    #[serde(default = "std::collections::BTreeMap::new", skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    settings: std::collections::BTreeMap<S, PuzzleCustomSettingValue<A, M, S>>,
  },
  Proximity {
    length: u32,
    matcher: rules::PlayerMarkMatcher,
    name: S,
    width: u32,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct PuzzleCustom<A, M, S: AsRef<str> + std::cmp::Ord + std::hash::Hash> {
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub ambient_audio: Vec<AmbientAudio<PuzzleCustomAsset<A, S>, S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub event_audio: Vec<EventAudio<PuzzleCustomAsset<A, S>, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
  pub gradiators_color: std::collections::BTreeSet<S>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
  pub gradiators_intensity: std::collections::BTreeSet<S>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub ground: Vec<Vec<Option<PuzzleCustomGround>>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub lights: Vec<PuzzleCustomLight<S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub logic: Vec<LogicElement>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub materials: std::collections::BTreeMap<S, PuzzleCustomMaterial<S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub meshes: Vec<PuzzleCustomModel<S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub propagation_rules: Vec<rules::PropagationRule<PuzzleCustomInternalId<S>, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub settings: std::collections::BTreeMap<S, PuzzleCustomSetting<A, M, S>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomAsset<A, S: AsRef<str>> {
  Fixed(A),
  Setting(S),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomMaterial<S: AsRef<str>> {
  Fixed(Material<LocalBlendableValue<Color, S>, LocalBlendableValue<f64, S>, LocalDiscreteValue<bool, S>>),
  Replaceable { description: String, default: Material<LocalBlendableValue<Color, S>, LocalBlendableValue<f64, S>, LocalDiscreteValue<bool, S>> },
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PuzzleCustomInternalId<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> {
  Interact(crate::realm::InteractionKey<S>),
  Property(crate::realm::PropertyKey<S>),
  Logic(u32),
  Proximity(u32),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomLight<S: AsRef<str>> {
  Static(Light<GlobalValue<f64, S>, GlobalValue<Color, S>>),
  Output { light: Light<GlobalValue<f64, S>, GlobalValue<Color, S>>, id: S },
  Select { lights: Vec<Light<GlobalValue<f64, S>, GlobalValue<Color, S>>>, id: S },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomGround {
  Proximity(u32),
  Solid,
  Suppress,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PuzzleCustomModelElement<S: AsRef<str>> {
  pub material: S,
  pub mesh: PuzzleCustomAsset<Mesh, S>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomModel<S: AsRef<str>> {
  Button {
    elements: Vec<PuzzleCustomModelElement<S>>,
    enabled: bool,
    length: u32,
    name: S,
    width: u32,
    x: u32,
    y: u32,
  },
  Output {
    common_elements: Vec<PuzzleCustomModelElement<S>>,
    elements: Vec<Vec<PuzzleCustomModelElement<S>>>,
    name: S,
    x: u32,
    y: u32,
  },
  RadioButton {
    elements: Vec<PuzzleCustomModelElement<S>>,
    enabled: bool,
    initial: u32,
    length: u32,
    name: S,
    off_elements: Vec<PuzzleCustomModelElement<S>>,
    on_elements: Vec<PuzzleCustomModelElement<S>>,
    value: u32,
    width: u32,
    x: u32,
    y: u32,
  },
  RealmSelector {
    elements: Vec<PuzzleCustomModelElement<S>>,
    length: u32,
    name: S,
    width: u32,
    x: u32,
    y: u32,
  },
  Static {
    elements: Vec<PuzzleCustomModelElement<S>>,
    x: u32,
    y: u32,
  },
  Switch {
    elements: Vec<PuzzleCustomModelElement<S>>,
    enabled: bool,
    initial: bool,
    length: u32,
    name: S,
    off_elements: Vec<PuzzleCustomModelElement<S>>,
    on_elements: Vec<PuzzleCustomModelElement<S>>,
    width: u32,
    x: u32,
    y: u32,
  },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomSetting<A, M, S: AsRef<str>> {
  Audio(A),
  Bool(bool),
  Color(Color),
  Intensity(f64),
  Mesh(M),
  Num(u32),
  Realm(crate::realm::RealmTarget<S>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PuzzleCustomSettingValue<A, M, S: AsRef<str>> {
  Audio(CustomValue<A, S>),
  Bool(GlobalValue<bool, S>),
  Color(GlobalValue<Color, S>),
  Intensity(GlobalValue<f64, S>),
  Mesh(CustomValue<M, S>),
  Num(GlobalValue<u32, S>),
  Realm(GlobalValue<crate::realm::RealmTarget<S>, S>),
}

struct PuzzleCustomResourceMapper<'a, R>(&'a mut R);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "A: Deserialize<'de>, M: Deserialize<'de>, C: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct SimpleRealmDescription<A, M, C, S: AsRef<str> + std::hash::Hash + std::cmp::Ord> {
  //TODO: bridges, background/distance sky
  pub aesthetic: Aesthetic,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub ambient_audio: Vec<AmbientAudio<A, S>>,
  pub ambient_color: GlobalValue<Color, S>,
  pub ambient_intensity: GlobalValue<f64, S>,
  pub entry: S,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub event_audio: Vec<EventAudio<A, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub gradiators_audio: std::collections::BTreeMap<S, gradiator::Gradiator<AmbientAudio<A, S>, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub gradiators_color: std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub gradiators_intensity: std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub logic: Vec<LogicElement>,
  pub name: S,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub masks: std::collections::BTreeMap<S, MaskConfiguration>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub materials: Vec<Material<LocalBlendableValue<Color, S>, LocalBlendableValue<f64, S>, LocalDiscreteValue<bool, S>>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub platforms: Vec<PlatformDescription<A, M, C, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub player_effects: std::collections::BTreeMap<u8, crate::avatar::Effect>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub propagation_rules: Vec<rules::PropagationRule<SimpleRealmPuzzleId<S>, S>>,
  #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
  pub settings: std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub sprays: Vec<Spray<M, S>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub walls: Vec<Wall<M, S>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SimpleRealmPuzzleId<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> {
  Custom { platform: u32, item: u32, name: PuzzleCustomInternalId<S> },
  Interact(crate::realm::InteractionKey<S>),
  Logic(u32),
  Map(SimpleRealmMapId<S>),
  Property(crate::realm::PropertyKey<S>),
  Proximity(S),
}
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SimpleRealmMapId<S: AsRef<str>> {
  Wall(S),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Spray<M, S: AsRef<str>> {
  /// The rotation that should be applied to each model
  pub angle: Angle,
  /// The models that should be used and their relative proportions in the output
  pub elements: Vec<SprayElement<M, S>>,
  /// If true, the models have an angle in the direction of gravity. If false, it is normal to the surface
  pub vertical: bool,
  /// The amount the vertical angle should be changed for each model
  pub vertical_perturbation: Perturbation,
  /// If present, an output that allows the puzzle to control the visibility of this spray
  pub visible: Option<S>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "M: Deserialize<'de>, S: Deserialize<'de>"))]
pub struct SprayElement<M, S: AsRef<str>> {
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub arguments: Vec<Argument<S>>,
  pub model: M,
  pub weight: u8,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimpleSprayModel<Mesh, Material, Color, Intensity> {
  pub meshes: Vec<SprayModelElement<Mesh, Material>>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub lights: Vec<Light<Intensity, Color>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SprayModelElement<Mesh, Material> {
  pub material: Material,
  pub mesh: Mesh,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SwitchArgument<S: AsRef<str>> {
  Material(u32),
  SwitchMaterial(u32, u32, Transition),
  Color(LocalBlendableValue<Color, S>),
  SwitchColor(Color, Color, Transition),
  Intensity(LocalBlendableValue<f64, S>),
  SwitchIntensity(f64, f64, Transition),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Transformation {
  // Normal
  N,
  // Flipped horizontally
  H,
  // Flipped vertically
  V,
  // Rotated clockwise
  C,
  // Rotated anti-clockwise
  A,
  // Rotate anti-clockwise then flip vertically
  AV,
  // Rotate clockwise then flip vertically
  CV,
  // Flip vertically and horizontally
  VH,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Transition {
  Instant,
  Fade(std::time::Duration),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound(deserialize = "M: Deserialize<'de>, S: Deserialize<'de>"))]
pub enum Wall<M, S: AsRef<str>> {
  Solid {
    width: f32,
    width_perturbation: Perturbation,
    material: u32,
  },
  Fence {
    /// The rotation that should be applied to each model
    angle: Angle,
    posts: Vec<SprayElement<M, S>>,
    /// If true, the models have an angle in the direction of gravity. If false, it is normal to the surface
    vertical: bool,
    /// The amount the vertical angle should be changed for each model
    vertical_perturbation: Perturbation,
  },
  Gate {
    /// The rotation that should be applied to each model
    angle: Angle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<SwitchArgument<S>>,
    identifier: S,
    model: M,
    /// If true, the models have an angle in the direction of gravity. If false, it is normal to the surface
    vertical: bool,
    /// The amount the vertical angle should be changed for each model
    vertical_perturbation: Perturbation,
  },
  Block {
    /// The rotation that should be applied to each model
    angle: Angle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    arguments: Vec<Argument<S>>,
    identifier: S,
    model: M,
    /// If true, the models have an angle in the direction of gravity. If false, it is normal to the surface
    vertical: bool,
    /// The amount the vertical angle should be changed for each model
    vertical_perturbation: Perturbation,
  },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WallPath {
  Line { x1: u32, y1: u32, x2: u32, y2: u32 },
  Quadratic { x1: u32, y1: u32, x2: u32, y2: u32, xc: u32, yc: u32 },
}
impl Angle {
  pub fn compute(&self, seed: i32, current_x: u32, current_y: u32) -> f32 {
    let random = ((seed as i64).abs() as u64).wrapping_mul(current_x as u64).wrapping_mul(current_y as u64) as f32 / std::u64::MAX as f32;
    match self {
      &Angle::Fixed(v) => (v as f32) * std::f32::consts::TAU / u16::MAX as f32,
      &Angle::Noisy { offset, noise } => {
        (offset as f32) * std::f32::consts::TAU / u16::MAX as f32 + (noise as f32) * std::f32::consts::TAU / u16::MAX as f32 * random
      }
      Angle::Oriented { x, y, offset, noise } => {
        (current_x.abs_diff(*x) as f32).atan2(current_y.abs_diff(*y) as f32)
          + ((*offset as f32) / u16::MAX as f32
            + match noise {
              &Some(noise) => random * noise as f32 / u16::MAX as f32,
              None => 0.0,
            })
            * std::f32::consts::TAU
      }
      Angle::Random => random * std::f32::consts::TAU,
    }
  }
}
impl Asset {
  pub fn principal_hash(&self) -> String {
    use sha3::Digest;
    let mut principal_hash = sha3::Sha3_512::new();
    principal_hash.update(self.asset_type.as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(self.author.as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(self.name.as_bytes());
    principal_hash.update(&[0]);
    principal_hash.update(&self.data);
    principal_hash.update(&[0]);
    let license_str: &[u8] = match &self.licence {
      Licence::CreativeCommons(u) => match u {
        LicenceUses::Commercial => b"cc",
        LicenceUses::NonCommercial => b"cc-nc",
      },
      Licence::CreativeCommonsNoDerivatives(u) => match u {
        LicenceUses::Commercial => b"cc-nd",
        LicenceUses::NonCommercial => b"cc-nd-nc",
      },
      Licence::CreativeCommonsShareALike(u) => match u {
        LicenceUses::Commercial => b"cc-sa",
        LicenceUses::NonCommercial => b"cc-sa-nc",
      },
      Licence::PubDom => b"public",
    };
    principal_hash.update(license_str);
    principal_hash.update(&[0]);
    for tag in &self.tags {
      principal_hash.update(tag.as_bytes());
      principal_hash.update(&[0]);
    }
    principal_hash.update(self.created.to_rfc3339().as_bytes());
    hex::encode(principal_hash.finalize())
  }
}

pub trait ResourceMapper<A, M, C> {
  type Audio: 'static;
  type Custom: 'static;
  type Model: 'static;
  type Error: 'static;
  fn resolve_audio(&mut self, audio: A) -> Result<Self::Audio, Self::Error>;
  fn resolve_custom(&mut self, custom: C) -> Result<Self::Custom, Self::Error>;
  fn resolve_model(&mut self, model: M) -> Result<Self::Model, Self::Error>;
}

impl<'de, S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Clone + 'static> AssetKind<S>
  for PuzzleCustom<S, S, S>
{
  const KIND: &'static str = "puzzle-custom";
  type Resolved = PuzzleCustom<Loaded<AssetAnyAudio, S>, Loaded<AssetAnyModel, S>, S>;
  fn check(resolved: &Self::Resolved) -> bool {
    resolved.validate("puzzle custom").is_ok()
  }

  fn extract_capabilities(_: &Self::Resolved) -> Vec<String> {
    vec![]
  }

  fn resolve(
    self,
    mapper: &mut impl ResourceMapper<
      S,
      S,
      S,
      Audio = Loaded<AssetAnyAudio, S>,
      Custom = Loaded<AssetAnyCustom<S>, S>,
      Error = crate::AssetError,
      Model = Loaded<AssetAnyModel, S>,
    >,
  ) -> Result<Self::Resolved, crate::AssetError> {
    self.map(mapper)
  }
}
impl<S: AsRef<str> + std::hash::Hash + std::cmp::Ord + serde::de::DeserializeOwned + std::fmt::Display + Clone + 'static> AssetKind<S>
  for SimpleRealmDescription<S, S, S, S>
{
  const KIND: &'static str = "simple-realm";
  type Resolved = SimpleRealmDescription<Loaded<AssetAnyAudio, S>, Loaded<AssetAnyModel, S>, Loaded<AssetAnyCustom<S>, S>, S>;
  fn check(resolved: &Self::Resolved) -> bool {
    resolved.validate().is_ok()
  }

  fn extract_capabilities(_: &Self::Resolved) -> Vec<String> {
    vec![]
  }

  fn resolve(
    self,
    mapper: &mut impl ResourceMapper<
      S,
      S,
      S,
      Audio = Loaded<AssetAnyAudio, S>,
      Custom = Loaded<AssetAnyCustom<S>, S>,
      Error = crate::AssetError,
      Model = Loaded<AssetAnyModel, S>,
    >,
  ) -> Result<Self::Resolved, crate::AssetError> {
    self.map(mapper)
  }
}
impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Clone> AssetKind<S>
  for SimpleSprayModel<Mesh, u32, u32, u32>
{
  const KIND: &'static str = "simple-model";

  type Resolved = SimpleSprayModel<Mesh, u32, u32, u32>;

  fn check(resolved: &Self::Resolved) -> bool {
    let light_ids: std::collections::BTreeSet<_> = resolved
      .lights
      .iter()
      .flat_map(|l| {
        (match l {
          Light::Point { color, .. } => Some(*color),
        })
        .into_iter()
      })
      .collect();
    let material_ids: std::collections::BTreeSet<_> = resolved.meshes.iter().map(|e| e.material).collect();
    let light_max_id = light_ids.iter().max().copied().unwrap_or(0);
    let material_max_id = material_ids.iter().max().copied().unwrap_or(0);
    light_ids.into_iter().eq(0..light_max_id) && material_ids.into_iter().eq(0..material_max_id)
  }

  fn extract_capabilities(_: &Self::Resolved) -> Vec<String> {
    vec![]
  }

  fn resolve(
    self,
    _: &mut impl ResourceMapper<
      S,
      S,
      S,
      Audio = Loaded<AssetAnyAudio, S>,
      Custom = Loaded<AssetAnyCustom<S>, S>,
      Error = crate::AssetError,
      Model = Loaded<AssetAnyModel, S>,
    >,
  ) -> Result<Self::Resolved, crate::AssetError> {
    Ok(self)
  }
}
impl<S: AsRef<str> + std::cmp::Ord + Clone> ExtractChildren<S> for S {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    assets.insert(self.clone());
  }
}
impl<S: AsRef<str> + std::cmp::Ord + Clone, T: ExtractChildren<S>> ExtractChildren<S> for Loaded<T, S> {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    assets.insert(self.asset().clone());
    self.value.extract_children(assets);
  }
}
impl<T: ExtractChildren<S>, S: AsRef<str>> ExtractChildren<S> for CustomValue<T, S> {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    match self {
      CustomValue::Fixed(a) => {
        a.extract_children(assets);
      }
      CustomValue::PuzzleBool { when_true, when_false, .. } => {
        when_true.extract_children(assets);
        when_false.extract_children(assets);
      }
      CustomValue::PuzzleNum { default, values, .. } => {
        default.extract_children(assets);
        for value in values {
          value.extract_children(assets);
        }
      }
      CustomValue::Masked(_) => (),
      CustomValue::Random(values) => {
        for value in values {
          value.extract_children(assets);
        }
      }
      CustomValue::SettingBool { when_true, when_false, .. } => {
        when_true.extract_children(assets);
        when_false.extract_children(assets);
      }
      CustomValue::SettingNum { default, values, .. } => {
        default.extract_children(assets);
        for value in values {
          value.extract_children(assets);
        }
      }
    }
  }
}
impl<S: AsRef<str>> ExtractChildren<S> for SimpleSprayModel<Mesh, u32, u32, u32> {
  fn extract_children<'a>(&'a self, _: &mut std::collections::BTreeSet<S>) {}
}
impl<A, M, C, S: AsRef<str> + std::hash::Hash + std::cmp::Ord + Clone> SimpleRealmDescription<A, M, C, S> {
  pub fn map<R: ResourceMapper<A, M, C>>(self, mapper: &mut R) -> Result<SimpleRealmDescription<R::Audio, R::Model, R::Custom, S>, R::Error> {
    let mut ambient_audio = Vec::new();
    for a in self.ambient_audio {
      ambient_audio.push(a.map(mapper)?);
    }
    let mut event_audio = Vec::new();
    for a in self.event_audio {
      event_audio.push(a.map(mapper)?);
    }
    let mut gradiators_audio = std::collections::BTreeMap::new();
    for (n, g) in self.gradiators_audio {
      gradiators_audio.insert(n, g.map(|a| a.map(mapper))?);
    }
    Ok(SimpleRealmDescription {
      aesthetic: self.aesthetic,
      ambient_audio,
      ambient_color: self.ambient_color,
      ambient_intensity: self.ambient_intensity,
      entry: self.entry,
      event_audio,
      gradiators_audio,
      gradiators_color: self.gradiators_color,
      gradiators_intensity: self.gradiators_intensity,
      logic: self.logic,
      name: self.name,
      masks: self.masks,
      materials: self.materials,
      platforms: self.platforms.into_iter().map(|p| p.map(mapper)).collect::<Result<_, _>>()?,
      propagation_rules: self.propagation_rules,
      player_effects: self.player_effects,
      settings: self.settings,
      sprays: self.sprays.into_iter().map(|s| s.map(mapper)).collect::<Result<_, _>>()?,
      walls: self.walls.into_iter().map(|w| w.map(mapper)).collect::<Result<_, _>>()?,
    })
  }
}
impl<A: ExtractChildren<S>, M: ExtractChildren<S>, C: ExtractChildren<S>, S: AsRef<str> + std::hash::Hash + std::cmp::Ord> ExtractChildren<S>
  for SimpleRealmDescription<A, M, C, S>
{
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    for ambient_audio in &self.ambient_audio {
      match &ambient_audio.sound {
        AmbientAudioSound::Asset(a) => {
          a.extract_children(assets);
        }
        AmbientAudioSound::Static => (),
        AmbientAudioSound::Wind => (),
      }
    }
    for event_audio in &self.event_audio {
      event_audio.sound.extract_children(assets);
    }
    for platform in &self.platforms {
      for item in &platform.contents {
        match &item.item {
          PuzzleItem::Button { model, .. }
          | PuzzleItem::Switch { model, .. }
          | PuzzleItem::CycleButton { model, .. }
          | PuzzleItem::CycleDisplay { model, .. }
          | PuzzleItem::Display { model, .. }
          | PuzzleItem::RealmSelector { model, .. } => {
            model.extract_children(assets);
          }

          PuzzleItem::Custom { item, settings, .. } => {
            item.extract_children(assets);
            for (_, setting) in settings {
              match setting {
                PuzzleCustomSettingValue::Audio(a) => a.extract_children(assets),
                PuzzleCustomSettingValue::Bool(_) => (),
                PuzzleCustomSettingValue::Color(_) => (),
                PuzzleCustomSettingValue::Intensity(_) => (),
                PuzzleCustomSettingValue::Mesh(m) => m.extract_children(assets),
                PuzzleCustomSettingValue::Num(_) => (),
                PuzzleCustomSettingValue::Realm(_) => (),
              }
            }
          }
          PuzzleItem::Proximity { .. } => (),
        }
      }
    }
    for spray in &self.sprays {
      for element in &spray.elements {
        element.model.extract_children(assets);
      }
    }
    for wall in &self.walls {
      match wall {
        Wall::Solid { .. } => (),
        Wall::Fence { posts, .. } => {
          for post in posts {
            post.model.extract_children(assets);
          }
        }
        Wall::Gate { model, .. } | Wall::Block { model, .. } => {
          model.extract_children(assets);
        }
      }
    }
  }
}
trait SettingEquivalent<S: AsRef<str>> {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool;
  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool;
  fn included_in_mask(mask: &MaskConfiguration) -> bool;
}
impl<A, S: AsRef<str>> SettingEquivalent<S> for AmbientAudio<A, S> {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::AudioSource(_))
  }

  fn can_be_set_from_custom<AA, M>(setting: &PuzzleCustomSetting<AA, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Audio(_))
  }

  fn included_in_mask(_: &MaskConfiguration) -> bool {
    false
  }
}
impl<S: AsRef<str>> SettingEquivalent<S> for bool {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::Bool(_))
  }

  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Bool(_))
  }

  fn included_in_mask(_: &MaskConfiguration) -> bool {
    false
  }
}
impl<S: AsRef<str>> SettingEquivalent<S> for Color {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::Color(_))
  }

  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Color(_))
  }

  fn included_in_mask(mask: &MaskConfiguration) -> bool {
    match mask {
      MaskConfiguration::Bool { color, .. } => color.is_some(),
      MaskConfiguration::Num { color, .. } => color.is_some(),
    }
  }
}
impl<S: AsRef<str>> SettingEquivalent<S> for f64 {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::Intensity(_))
  }

  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Intensity(_))
  }

  fn included_in_mask(mask: &MaskConfiguration) -> bool {
    match mask {
      MaskConfiguration::Bool { material, .. } => material.is_some(),
      MaskConfiguration::Num { material, .. } => material.is_some(),
    }
  }
}
impl<S: AsRef<str>> SettingEquivalent<S> for crate::realm::RealmTarget<S> {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::Realm(_))
  }

  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Realm(_))
  }

  fn included_in_mask(_: &MaskConfiguration) -> bool {
    false
  }
}
impl<S: AsRef<str>> SettingEquivalent<S> for u32 {
  fn can_be_set_from(setting: &crate::realm::RealmSetting<S>) -> bool {
    matches!(setting, crate::realm::RealmSetting::Num(_))
  }

  fn can_be_set_from_custom<A, M>(setting: &PuzzleCustomSetting<A, M, S>) -> bool {
    matches!(setting, PuzzleCustomSetting::Num(_))
  }

  fn included_in_mask(_: &MaskConfiguration) -> bool {
    false
  }
}
impl<
    A: std::ops::Deref<Target = AssetAnyAudio>,
    M: std::ops::Deref<Target = AssetAnyModel>,
    C: std::ops::Deref<Target = AssetAnyCustom<S>>,
    S: AsRef<str> + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone + 'static,
  > SimpleRealmDescription<A, M, C, S>
{
  pub fn validate(&self) -> Result<(), String> {
    fn check_gradiator<'a, T: 'a + SettingEquivalent<S>, S: AsRef<str> + std::hash::Hash + std::cmp::Ord + Clone + std::fmt::Display + 'a>(
      gradiators: impl IntoIterator<Item = (&'a S, &'a gradiator::Gradiator<T, S>)>,
      defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
      settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    ) -> Result<(), String> {
      for (name, gradiator) in gradiators {
        for source in &gradiator.sources {
          match &source.source {
            gradiator::Current::Altitude { top_altitude, bottom_altitude, .. } => {
              if top_altitude <= bottom_altitude {
                return Err(format!("Altitude ranges from {} to {} which is not valid", bottom_altitude, top_altitude));
              }
            }
            gradiator::Current::BoolControlled { value, .. } => {
              defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::BoolSink(value.clone())));
            }
            gradiator::Current::Fixed(_) => (),
            gradiator::Current::NumControlled { value, .. } => {
              defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::NumSink(value.clone())));
            }
            gradiator::Current::Setting(setting) => match settings.get(setting) {
              None => return Err(format!("Gradiator {} references non-existent setting {}.", name, setting)),
              Some(s) => {
                if !T::can_be_set_from(s) {
                  return Err(format!("Gradiator {} references setting {} which is of type {}.", name, setting, s.type_name()));
                }
              }
            },
          }
        }
      }
      Ok(())
    }
    fn check_custom_value<T, S: AsRef<str> + std::cmp::Eq + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone>(
      name: &str,
      value: &CustomValue<T, S>,
      defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
      settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    ) -> Result<(), String> {
      match value {
        CustomValue::PuzzleBool { id, .. } => {
          defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::BoolSink(id.clone())));
          Ok(())
        }
        CustomValue::PuzzleNum { id, .. } => {
          defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::NumSink(id.clone())));
          Ok(())
        }
        CustomValue::SettingBool { id, .. } => match settings.get(id) {
          None => Err(format!("Custom value {} references non-existent setting {}.", name, id)),
          Some(s) => {
            if matches!(s, crate::realm::RealmSetting::Bool(_)) {
              Ok(())
            } else {
              Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
            }
          }
        },
        CustomValue::SettingNum { id, .. } => match settings.get(id) {
          None => Err(format!("Custom value {} references non-existent setting {}.", name, id)),
          Some(s) => {
            if matches!(s, crate::realm::RealmSetting::Num(_)) {
              Ok(())
            } else {
              Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
            }
          }
        },
        _ => Ok(()),
      }
    }
    fn check_arguments<'a, A: ValidatableArgument<S>, S: AsRef<str> + std::cmp::Eq + std::hash::Hash + 'static>(
      name: &str,
      args: impl IntoIterator<Item = &'a A>,
      defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
      settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
      masks: &std::collections::BTreeMap<S, MaskConfiguration>,
      materials: usize,
      color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
      intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
      context: A::Context,
    ) -> Result<(), String> {
      for (index, arg) in args.into_iter().enumerate() {
        arg.check_argument(
          &format!("argument {} of {}", index, name),
          defined_names,
          settings,
          masks,
          materials,
          color_gradiators,
          intensity_gradiators,
          context,
        )?;
      }
      Ok(())
    }
    fn check_model<'a, A: ValidatableArgument<S>, S: AsRef<str> + std::cmp::Eq + std::hash::Hash + 'static>(
      name: &str,
      args: impl IntoIterator<Item = &'a A>,
      model: &AssetAnyModel,
    ) -> Result<(), String> {
      let mut arguments = std::collections::BTreeMap::new();
      for (arg_number, arg_type) in match model {
        AssetAnyModel::Simple(simple) => {
          simple.meshes.iter().map(|m| (m.material, ArgumentType::Material)).chain(simple.lights.iter().map(|l| match l {
            Light::Point { color, .. } => (*color, ArgumentType::Color),
          }))
        }
      } {
        match arguments.entry(arg_number) {
          std::collections::btree_map::Entry::Vacant(v) => {
            v.insert(arg_type);
          }
          std::collections::btree_map::Entry::Occupied(o) => {
            if o.get() != &arg_type {
              return Err(format!("Argument {} in {} has type {} and {}.", arg_number, name, &arg_type, o.get()));
            }
          }
        }
      }
      for (arg_number, arg) in args.into_iter().enumerate() {
        let required_type = arg.argument_type();
        match arguments.get(&(arg_number as u32)) {
          None => {
            return Err(format!("Argument {} provided to model {} is not required by model.", arg_number, name));
          }
          Some(provided_type) => {
            if &required_type != provided_type {
              return Err(format!(
                "Argument {} provided to model {} must be {}, but {} is provided.",
                arg_number, name, &required_type, provided_type
              ));
            }
          }
        }
      }

      Ok(())
    }
    fn check_spray_element<
      M: std::ops::Deref<Target = AssetAnyModel>,
      S: AsRef<str> + std::cmp::Eq + std::cmp::Ord + std::hash::Hash + std::fmt::Display + Clone + 'static,
    >(
      name: &str,
      element: &SprayElement<M, S>,
      defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
      settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
      masks: &std::collections::BTreeMap<S, MaskConfiguration>,
      materials: usize,
      color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
      intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
    ) -> Result<(), String> {
      check_arguments(name, &element.arguments, defined_names, settings, masks, materials, color_gradiators, intensity_gradiators, ())?;
      check_model(name, &element.arguments, &element.model)?;
      Ok(())
    }
    let mut defined_names: std::collections::HashSet<_> = (0..self.logic.len()).into_iter().map(|id| SimpleRealmPuzzleId::Logic(id as u32)).collect();
    check_gradiator(&self.gradiators_audio, &mut defined_names, &self.settings)?;
    check_gradiator(&self.gradiators_color, &mut defined_names, &self.settings)?;
    check_gradiator(&self.gradiators_intensity, &mut defined_names, &self.settings)?;
    check_global_value("ambient color", &self.ambient_color, &mut defined_names, &self.masks, &self.settings)?;
    check_global_value("ambient color intensity", &self.ambient_intensity, &mut defined_names, &self.masks, &self.settings)?;
    for (id, audio) in self.event_audio.iter().enumerate() {
      check_global_value(&format!("volume of event audio {}", id), &audio.volume, &mut defined_names, &self.masks, &self.settings)?;
    }
    for (id, audio) in self.ambient_audio.iter().enumerate() {
      check_global_value(&format!("volume of ambient audio {}", id), &audio.volume, &mut defined_names, &self.masks, &self.settings)?;
    }
    for (name, audio) in &self.gradiators_audio {
      for source in &audio.sources {
        match &source.source {
          gradiator::Current::Altitude { top_value, bottom_value, .. } => {
            check_global_value(
              &format!("volume of top audio in gradiator {}", name),
              &top_value.volume,
              &mut defined_names,
              &self.masks,
              &self.settings,
            )?;
            check_global_value(
              &format!("volume of bottom audio in gradiator {}", name),
              &bottom_value.volume,
              &mut defined_names,
              &self.masks,
              &self.settings,
            )?;
          }
          gradiator::Current::BoolControlled { when_true, when_false, .. } => {
            check_global_value(
              &format!("volume of true audio in gradiator {}", name),
              &when_true.volume,
              &mut defined_names,
              &self.masks,
              &self.settings,
            )?;
            check_global_value(
              &format!("volume of false audio in gradiator {}", name),
              &when_false.volume,
              &mut defined_names,
              &self.masks,
              &self.settings,
            )?;
          }
          gradiator::Current::Fixed(_) => (),
          gradiator::Current::NumControlled { default_value, values, .. } => {
            check_global_value(
              &format!("volume of default audio in gradiator {}", name),
              &default_value.volume,
              &mut defined_names,
              &self.masks,
              &self.settings,
            )?;
            for (index, audio) in values.iter().enumerate() {
              check_global_value(
                &format!("volume of {} audio in gradiator {}", index, name),
                &audio.volume,
                &mut defined_names,
                &self.masks,
                &self.settings,
              )?;
            }
          }
          gradiator::Current::Setting(_) => (),
        }
      }
    }
    for (name, mask) in &self.masks {
      defined_names.insert(SimpleRealmPuzzleId::Property(match mask {
        MaskConfiguration::Bool { masks, color, intensity, material } => {
          if color.is_none() && intensity.is_none() && material.is_none() {
            return Err(format!("Mask {} does not set any values.", name));
          }
          if masks.is_empty() {
            return Err(format!("Mask {} does not have any special cases.", name));
          }
          crate::realm::PropertyKey::BoolSink(name.clone())
        }
        MaskConfiguration::Num { masks, color, intensity, material } => {
          if color.is_none() && intensity.is_none() && material.is_none() {
            return Err(format!("Mask {} does not set any values.", name));
          }
          if masks.is_empty() {
            return Err(format!("Mask {} does not have any special cases.", name));
          }
          let lengths = [color.as_ref().unwrap().1.len(), intensity.as_ref().unwrap().1.len(), material.as_ref().unwrap().1.len()];
          let max = lengths.iter().copied().max().unwrap_or(0) as u32;
          let min = lengths.iter().copied().min().unwrap_or(0) as u32;
          if min != max {
            return Err(format!("Mask {} has differing lengths for number of choices (min = {}, max = {})", name, min, max));
          }
          for (id, m) in masks.iter().enumerate() {
            let value = *match m {
              Mask::Marked(_, v) => v,
              Mask::HasBit(_, v) => v,
              Mask::NotMarked(_, v) => v,
              Mask::HasNotBit(_, v) => v,
            };
            if value >= max {
              return Err(format!("Mask {} has special case {}, uses value {} that is higher than available values ({}).", name, id, value, max));
            }
          }
          crate::realm::PropertyKey::NumSink(name.clone())
        }
      }));
    }

    for (id, wall) in self.walls.iter().enumerate() {
      match wall {
        Wall::Solid { material, .. } => {
          if *material as usize >= self.materials.len() {
            return Err(format!("Material {} for solid wall {} is not valid", material, id));
          }
        }
        Wall::Fence { posts, .. } => {
          for (index, post) in posts.iter().enumerate() {
            check_spray_element(
              &format!("post {} in wall {}", index, id),
              post,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
            )?;
          }
        }
        Wall::Gate { arguments, identifier, model, .. } => {
          check_arguments(
            &format!("arguments for gate {}", id),
            arguments,
            &mut defined_names,
            &self.settings,
            &self.masks,
            self.materials.len(),
            &self.gradiators_color,
            &self.gradiators_intensity,
            (),
          )?;

          check_model(&format!("gate {}", id), arguments, model)?;
          defined_names.insert(SimpleRealmPuzzleId::Map(SimpleRealmMapId::Wall(identifier.clone())));
        }
        Wall::Block { arguments, identifier, model, .. } => {
          check_arguments(
            &format!("arguments for block {}", id),
            arguments,
            &mut defined_names,
            &self.settings,
            &self.masks,
            self.materials.len(),
            &self.gradiators_color,
            &self.gradiators_intensity,
            (),
          )?;

          check_model(&format!("gate {}", id), arguments, model)?;
          defined_names.insert(SimpleRealmPuzzleId::Map(SimpleRealmMapId::Wall(identifier.clone())));
        }
      }
    }
    for (id, spray) in self.sprays.iter().enumerate() {
      if let Some(name) = &spray.visible {
        defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::BoolSink(name.clone())));
      }
      for (index, spray) in spray.elements.iter().enumerate() {
        check_spray_element(
          &format!("model {} in spray {}", index, id),
          spray,
          &mut defined_names,
          &self.settings,
          &self.masks,
          self.materials.len(),
          &self.gradiators_color,
          &self.gradiators_intensity,
        )?;
      }
    }
    for (id, material) in self.materials.iter().enumerate() {
      match material {
        Material::BrushedMetal { color } => check_local_blendable_value(
          &format!("color of brushed metal {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::Crystal { color, opacity } => {
          check_local_blendable_value(
            &format!("color of crystal {}", id),
            color,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?;
          check_local_blendable_value(
            &format!("opacity of crystal {}", id),
            opacity,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_intensity,
          )?;
        }
        Material::Gem { color, accent, glow } => {
          check_local_blendable_value(
            &format!("color of gem {}", id),
            color,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?;
          if let Some(accent) = accent {
            check_local_blendable_value(
              &format!("accent color of gem {}", id),
              accent,
              &mut defined_names,
              &self.settings,
              &self.masks,
              &self.gradiators_color,
            )?;
          }
          check_local_discrete_value(&format!("glow of gem{}", id), glow, &mut defined_names, &self.settings, &self.masks)?;
        }
        Material::Metal { color, corrosion } => {
          check_local_blendable_value(
            &format!("color of metal {}", id),
            &color,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?;
          if let Some((corrosion, intensity)) = corrosion {
            check_local_blendable_value(
              &format!("corrosion color of metal {}", id),
              &corrosion,
              &mut defined_names,
              &self.settings,
              &self.masks,
              &self.gradiators_color,
            )?;
            check_local_blendable_value(
              &format!("corrosion intensity of metal {}", id),
              &intensity,
              &mut defined_names,
              &self.settings,
              &self.masks,
              &self.gradiators_intensity,
            )?
          }
        }
        Material::Rock { color } => check_local_blendable_value(
          &format!("color of rock {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::Sand { color } => check_local_blendable_value(
          &format!("color of sand {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::ShinyMetal { color } => check_local_blendable_value(
          &format!("color of shiny metal {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::Soil { color } => check_local_blendable_value(
          &format!("color of soil {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::Textile { color } => check_local_blendable_value(
          &format!("color of textile {}", id),
          &color,
          &mut defined_names,
          &self.settings,
          &self.masks,
          &self.gradiators_color,
        )?,
        Material::TreadPlate { color, corrosion } => {
          check_local_blendable_value(
            &format!("color of treadplate {}", id),
            &color,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?;
          if let Some(corrosion) = corrosion {
            check_local_blendable_value(
              &format!("corrosion color of treadplate {}", id),
              &corrosion,
              &mut defined_names,
              &self.settings,
              &self.masks,
              &self.gradiators_color,
            )?
          }
        }
        Material::Wood { background, grain } => {
          check_local_blendable_value(
            &format!("color of wood background {}", id),
            &background,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?;
          check_local_blendable_value(
            &format!("color of wood_grain {}", id),
            &grain,
            &mut defined_names,
            &self.settings,
            &self.masks,
            &self.gradiators_color,
          )?
        }
      }
    }
    for (id, platform) in self.platforms.iter().enumerate() {
      if platform.material >= self.materials.len() as u32 {
        return Err(format!("Platform {} has material {} go out of bounds.", id, platform.material));
      }
      for (index, item) in platform.contents.iter().enumerate() {
        if item.x > platform.width || item.y > platform.length {
          return Err(format!("Platform {} has item {} go out of bounds.", id, index));
        }
        match &item.item {
          PuzzleItem::Button { arguments, model, name, .. } => {
            check_arguments(
              &format!("arguments of button item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              (),
            )?;
            check_model(&format!("button item {} on platform {}", index, id), arguments, model)?;
            defined_names.insert(SimpleRealmPuzzleId::Interact(crate::realm::InteractionKey::Button(name.clone())));
          }
          PuzzleItem::Switch { arguments, model, name, .. } => {
            check_arguments(
              &format!("arguments of switch item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              (),
            )?;

            check_model(&format!("switch item {} on platform {}", index, id), arguments, model)?;
            defined_names.insert(SimpleRealmPuzzleId::Interact(crate::realm::InteractionKey::Switch(name.clone())));
            defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::BoolSink(name.clone())));
          }
          PuzzleItem::CycleButton { arguments, model, name, states, .. } => {
            check_arguments(
              &format!("arguments of cycle button item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              *states,
            )?;
            check_model(&format!("cycle button item {} on platform {}", index, id), arguments, model)?;
            if *states == 0 {
              return Err(format!("Cycle button item {} on platform {} has no selections", index, id));
            }
            defined_names.insert(SimpleRealmPuzzleId::Interact(crate::realm::InteractionKey::Button(name.clone())));
            defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::NumSink(name.clone())));
          }
          PuzzleItem::CycleDisplay { arguments, model, name, states, .. } => {
            check_arguments(
              &format!("arguments of cycle display item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              *states,
            )?;
            check_model(&format!("cycle display item {} on platform {}", index, id), arguments, model)?;
            if *states == 0 {
              return Err(format!("Cycle display item {} on platform {} has no selections", index, id));
            }
            defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::NumSink(name.clone())));
          }
          PuzzleItem::Display { arguments, model, .. } => {
            check_arguments(
              &format!("display item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              (),
            )?;
            check_model(&format!("display item {} on platform {}", index, id), arguments, model)?;
          }

          PuzzleItem::RealmSelector { arguments, model, name, .. } => {
            check_arguments(
              &format!("arguments of realm selector item {} on platform {}", index, id),
              arguments,
              &mut defined_names,
              &self.settings,
              &self.masks,
              self.materials.len(),
              &self.gradiators_color,
              &self.gradiators_intensity,
              (),
            )?;
            check_model(&format!("realm selector item {} on platform {}", index, id), arguments, model)?;
            defined_names.insert(SimpleRealmPuzzleId::Interact(crate::realm::InteractionKey::RealmSelector(name.clone())));
          }
          PuzzleItem::Custom { item, gradiators_color, gradiators_intensity, materials, settings, .. } => match &**item {
            AssetAnyCustom::Simple(item) => {
              let names = item.validate(&format!("custom item {} on platform {}", index, id))?;
              for (custom_gradiator, world_gradiator) in gradiators_color {
                if !item.gradiators_color.contains(custom_gradiator) {
                  return Err(format!("Color gradiator {} does not exist in custom puzzle element {} on platform {}.", custom_gradiator, index, id));
                }
                if !self.gradiators_color.contains_key(world_gradiator) {
                  return Err(format!(
                    "Color gradiator {} does not exist in realm for custom puzzle element {} on platform {}.",
                    world_gradiator, index, id
                  ));
                }
              }
              for (custom_gradiator, world_gradiator) in gradiators_intensity {
                if !item.gradiators_intensity.contains(custom_gradiator) {
                  return Err(format!(
                    "Intensity gradiator {} does not exist in custom puzzle element {} on platform {}.",
                    custom_gradiator, index, id
                  ));
                }
                if !self.gradiators_intensity.contains_key(world_gradiator) {
                  return Err(format!(
                    "Intensity gradiator {} does not exist in realm for custom puzzle element {} on platform {}.",
                    world_gradiator, index, id
                  ));
                }
              }
              for (material_name, material_id) in materials {
                match item.materials.get(material_name) {
                  None => {
                    return Err(format!("Material {} is provided to custom puzzle {} on platform {} but it is not used.", material_name, index, id));
                  }
                  Some(PuzzleCustomMaterial::Fixed(_)) => {
                    return Err(format!(
                      "Material {} is provided to custom puzzle {} on platform {} but it is not modifable.",
                      material_name, index, id
                    ));
                  }
                  Some(_) => {
                    if *material_id as usize >= self.materials.len() {
                      return Err(format!(
                      "Material {} is provided from realm material {} to custom puzzle {} on platform {} but that material does not exist in the realm.",
                      material_name, material_id, index, id
                    ));
                    }
                  }
                }
              }

              for (setting_name, setting_value) in settings {
                match item.settings.get(setting_name) {
                  None => {
                    return Err(format!("Setting {} is provided to custom puzzle {} on platform {} but it is not used.", setting_name, index, id));
                  }
                  Some(setting) => match (setting, setting_value) {
                    (PuzzleCustomSetting::Audio(_), PuzzleCustomSettingValue::Audio(value)) => {
                      check_custom_value(
                        &format!("Audio setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Bool(_), PuzzleCustomSettingValue::Bool(value)) => {
                      check_global_value(
                        &format!("Boolean setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.masks,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Color(_), PuzzleCustomSettingValue::Color(value)) => {
                      check_global_value(
                        &format!("Color setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.masks,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Intensity(_), PuzzleCustomSettingValue::Intensity(value)) => {
                      check_global_value(
                        &format!("Intensity setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.masks,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Mesh(_), PuzzleCustomSettingValue::Mesh(value)) => {
                      check_custom_value(
                        &format!("Mesh setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Num(_), PuzzleCustomSettingValue::Num(value)) => {
                      check_global_value(
                        &format!("Numeric setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.masks,
                        &self.settings,
                      )?;
                    }
                    (PuzzleCustomSetting::Realm(_), PuzzleCustomSettingValue::Realm(value)) => {
                      check_global_value(
                        &format!("Realm setting {} for custom puzzle {} on platform {}", setting_name, index, id),
                        value,
                        &mut defined_names,
                        &self.masks,
                        &self.settings,
                      )?;
                    }
                    (s, _) => {
                      return Err(format!("Setting {} for custom puzzle {} on platform {} should be {}", setting_name, index, id, s.type_name()));
                    }
                  },
                }
              }
              defined_names.extend(names.into_iter().map(|name| SimpleRealmPuzzleId::Custom { platform: id as u32, item: index as u32, name }));
            }
          },
          PuzzleItem::Proximity { name, .. } => {
            if !defined_names.insert(SimpleRealmPuzzleId::Proximity(name.clone())) {
              return Err(format!("Proximity {} is defined multiple times.", name));
            }
          }
        }
      }
      for (index, spray) in platform.sprays.iter().enumerate() {
        if *spray as usize >= self.walls.len() {
          return Err(format!("Platform {} has invalid spray {} at position {}.", id, spray, index));
        }
      }
      for (index, (wall, path)) in platform.walls.iter().enumerate() {
        if *wall as usize >= self.walls.len() {
          return Err(format!("Platform {} references wall {} at position {} which does not exist.", id, wall, index));
        }
        for (segment_id, segment) in path.iter().enumerate() {
          match segment {
            WallPath::Line { x1, y1, x2, y2 } => {
              if *x1 > platform.width || *x2 > platform.width || *y1 > platform.length || *y2 > platform.length {
                return Err(format!("Platform {} has wall {} go out of bounds at position {}.", id, index, segment_id));
              }
            }
            WallPath::Quadratic { x1, y1, x2, y2, xc, yc } => {
              if *x1 > platform.width
                || *x2 > platform.width
                || *xc > platform.width
                || *y1 > platform.length
                || *y2 > platform.length
                || *yc > platform.length
              {
                return Err(format!("Platform {} has wall {} go out of bounds at position {}.", id, index, segment_id));
              }
            }
          }
        }
      }
    }
    if !defined_names.contains(&SimpleRealmPuzzleId::Proximity(self.entry.clone())) {
      return Err(format!("Entry point {} does not refer to a valid platform.", &self.entry));
    }

    for (index, rule) in self.propagation_rules.iter().enumerate() {
      if !defined_names.contains(&rule.recipient) {
        return Err(format!("Propagation rule {} refers to recipient {} which does not exist.", index, &rule.recipient));
      }
    }
    Ok(())
  }
}
impl<A, S: AsRef<str> + Clone> AmbientAudio<A, S> {
  pub fn map<M, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<AmbientAudio<T::Audio, S>, T::Error> {
    Ok(AmbientAudio { sound: self.sound.map(mapper)?, volume: self.volume.clone() })
  }
}
impl<A> AmbientAudioSound<A> {
  pub fn map<M, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<AmbientAudioSound<T::Audio>, T::Error> {
    Ok(match self {
      AmbientAudioSound::Asset(asset) => AmbientAudioSound::Asset(mapper.resolve_audio(asset)?),
      AmbientAudioSound::Static => AmbientAudioSound::Static,
      AmbientAudioSound::Wind => AmbientAudioSound::Wind,
    })
  }
}

impl AssetAnyAudio {
  pub async fn load<S: AsRef<str>, R>(_mapper: &R, asset: Asset) -> Result<AssetAnyAudio, crate::AssetError>
  where
    R: ResourceMapper<S, S, S, Audio = Loaded<AssetAnyAudio, S>, Model = Loaded<AssetAnyModel, S>, Error = crate::AssetError>,
  {
    Err(crate::AssetError::UnknownKind)
  }
}
impl<S: AsRef<str>> ExtractChildren<S> for AssetAnyAudio {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    todo!();
  }
}
impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + Clone> ExtractChildren<S> for AssetAnyCustom<S> {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    match self {
      AssetAnyCustom::Simple(s) => s.extract_children(assets),
    }
  }
}
impl<S: AsRef<str>> ExtractChildren<S> for AssetAnyModel {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    match self {
      AssetAnyModel::Simple(s) => s.extract_children(assets),
    }
  }
}
impl<S: AsRef<str> + std::cmp::Ord + std::hash::Hash + std::fmt::Display + serde::de::DeserializeOwned + Clone + 'static> AssetAnyCustom<S> {
  pub fn load<R>(mapper: &mut R, asset: Asset) -> Result<AssetAnyCustom<S>, crate::AssetError>
  where
    R: ResourceMapper<S, S, S, Audio = Loaded<AssetAnyAudio, S>, Model = Loaded<AssetAnyModel, S>, Error = crate::AssetError>,
  {
    match asset.asset_type.as_str() {
      PuzzleCustom::<_, _, S>::KIND => Ok(AssetAnyCustom::Simple(
        asset.compression.decompress::<PuzzleCustom<S, S, S>>(asset.data.as_slice()).map_err(|_| crate::AssetError::DecodeFailure)?.map(mapper)?,
      )),
      _ => Err(crate::AssetError::UnknownKind),
    }
  }
}
impl AssetAnyModel {
  pub fn load<S: AsRef<str> + std::hash::Hash + std::cmp::Ord + serde::de::DeserializeOwned + std::fmt::Display + Clone + Sync + 'static, R>(
    _mapper: &R,
    asset: Asset,
  ) -> Result<AssetAnyModel, crate::AssetError>
  where
    R: ResourceMapper<S, S, S, Audio = Loaded<AssetAnyAudio, S>, Model = Loaded<AssetAnyModel, S>, Error = crate::AssetError>,
  {
    match asset.asset_type.as_str() {
      <SimpleSprayModel<Mesh, u32, u32, u32> as AssetKind<String>>::KIND => Ok(AssetAnyModel::Simple(
        asset.compression.decompress::<SimpleSprayModel<Mesh, u32, u32, u32>>(asset.data.as_slice()).map_err(|_| crate::AssetError::DecodeFailure)?,
      )),

      _ => Err(crate::AssetError::UnknownKind),
    }
  }
}
impl<S: AsRef<str> + std::hash::Hash + std::cmp::Ord + serde::de::DeserializeOwned + std::fmt::Display + Clone + Send + Sync + 'static>
  AssetAnyRealm<S>
where
  for<'a> &'a str: Into<S>,
{
  pub async fn load<T: crate::asset_store::AsyncAssetStore>(asset: Asset, store: &T) -> Result<(AssetAnyRealm<S>, Vec<String>), crate::AssetError> {
    let mut mapper = crate::asset_store::CachingResourceMapper::new();
    for child in &asset.children {
      mapper.install_from(store, child.as_str().into()).await?;
    }
    match asset.asset_type.as_str() {
      SimpleRealmDescription::<_, _, _, S>::KIND => Ok((
        AssetAnyRealm::Simple(
          asset
            .compression
            .decompress::<SimpleRealmDescription<S, S, S, S>>(asset.data.as_slice())
            .map_err(|_| crate::AssetError::DecodeFailure)?
            .map(&mut mapper)?,
        ),
        asset.capabilities,
      )),
      _ => Err(crate::AssetError::UnknownKind),
    }
  }
  pub fn name_for(&self, owner: &str) -> String {
    match self {
      AssetAnyRealm::Simple(s) => format!("{}'s {}", owner, &s.name),
    }
  }
}
impl Compression {
  pub fn compress<T: serde::Serialize>(&self, data: &T) -> Result<Vec<u8>, CompressionError> {
    Ok(match self {
      Compression::MessagePack => rmp_serde::to_vec(data)?,
      Compression::ZstdMessagePack => {
        let mut buffer = Vec::new();
        let mut writer = zstd::stream::zio::Writer::new(&mut buffer, zstd::stream::raw::Encoder::new(20)?);
        rmp_serde::encode::write(&mut writer, data)?;
        writer.finish()?;
        buffer
      }
    })
  }
  pub fn decompress<T: serde::de::DeserializeOwned>(&self, data: &[u8]) -> Result<T, DecompressionError> {
    Ok(match self {
      Compression::MessagePack => rmp_serde::from_slice(data)?,
      Compression::ZstdMessagePack => {
        use read_restrict::ReadExt;
        rmp_serde::from_read(zstd::stream::zio::Reader::new(std::io::Cursor::new(data), zstd::stream::raw::Decoder::new()?).restrict(15 * 1024))?
      }
    })
  }
}
impl std::fmt::Display for CompressionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CompressionError::MessagePack(e) => e.fmt(f),
      CompressionError::Io(e) => e.fmt(f),
    }
  }
}
impl From<std::io::Error> for CompressionError {
  fn from(value: std::io::Error) -> Self {
    CompressionError::Io(value)
  }
}
impl From<rmp_serde::encode::Error> for CompressionError {
  fn from(value: rmp_serde::encode::Error) -> Self {
    CompressionError::MessagePack(value)
  }
}
impl<T, S: AsRef<str>> CustomValue<T, S> {
  pub fn map<R, E, M: FnMut(T) -> Result<R, E>>(self, mut mapper: M) -> Result<CustomValue<R, S>, E> {
    Ok(match self {
      CustomValue::Fixed(v) => CustomValue::Fixed(mapper(v)?),
      CustomValue::PuzzleBool { id, when_true, when_false, transition } => {
        CustomValue::PuzzleBool { id, when_true: mapper(when_true)?, when_false: mapper(when_false)?, transition }
      }
      CustomValue::PuzzleNum { id, default, values, transition } => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::PuzzleNum { id, default: mapper(default)?, values: new_values, transition }
      }
      CustomValue::Masked(value) => CustomValue::Masked(value),
      CustomValue::Random(values) => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::Random(new_values)
      }
      CustomValue::SettingBool { id, when_true, when_false } => {
        CustomValue::SettingBool { id, when_true: mapper(when_true)?, when_false: mapper(when_false)? }
      }
      CustomValue::SettingNum { id, default, values } => {
        let mut new_values = Vec::new();
        for value in values {
          new_values.push(mapper(value)?);
        }
        CustomValue::SettingNum { id, default: mapper(default)?, values: new_values }
      }
    })
  }
}
impl std::fmt::Display for DecompressionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      DecompressionError::MessagePack(e) => e.fmt(f),
      DecompressionError::Io(e) => e.fmt(f),
    }
  }
}
impl From<std::io::Error> for DecompressionError {
  fn from(value: std::io::Error) -> Self {
    DecompressionError::Io(value)
  }
}
impl From<rmp_serde::decode::Error> for DecompressionError {
  fn from(value: rmp_serde::decode::Error) -> Self {
    DecompressionError::MessagePack(value)
  }
}
impl<A, S: AsRef<str> + Clone> EventAudio<A, S> {
  pub fn map<M, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<EventAudio<T::Audio, S>, T::Error> {
    Ok(EventAudio { name: self.name, sound: mapper.resolve_audio(self.sound)?, volume: self.volume.clone(), x: self.x, y: self.y, z: self.z })
  }
}
impl<A> EventAudioSound<A> {
  pub fn map<M, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<EventAudioSound<T::Audio>, T::Error> {
    Ok(match self {
      EventAudioSound::Asset(asset) => EventAudioSound::Asset(mapper.resolve_audio(asset)?),
    })
  }
}
impl<T, S: AsRef<str>> Loaded<T, S> {
  pub fn new(asset: S, value: T) -> Self {
    Loaded { asset, value: std::sync::Arc::new(value) }
  }
  pub fn asset(&self) -> &S {
    &self.asset
  }
}
impl<T, S: AsRef<str> + Clone> Clone for Loaded<T, S> {
  fn clone(&self) -> Self {
    Self { asset: self.asset.clone(), value: self.value.clone() }
  }
}
impl<T, S: AsRef<str>> std::ops::Deref for Loaded<T, S> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &*self.value
  }
}
impl Perturbation {
  pub fn compute(&self, seed: i32, current_x: u32, current_y: u32) -> (f32, f32) {
    let random = ((seed as i64).abs() as u64).wrapping_mul(current_x as u64).wrapping_mul(current_y as u64);
    use rand::Rng;
    use rand::SeedableRng;
    use rand_distr::Distribution;
    let mut rng = rand::rngs::SmallRng::seed_from_u64(random);
    (
      rng.gen_range(0.0..std::f32::consts::TAU),
      match self {
        Perturbation::Fixed => 0.0,
        &Perturbation::Offset(v) => {
          if rng.gen() {
            if rng.gen() {
              v
            } else {
              -v
            }
          } else {
            0.0
          }
        }
        &Perturbation::Range(v) => rng.gen_range((-v)..(v)),
        &Perturbation::Gaussian(s) => match rand_distr::Normal::new(0.0, s) {
          Ok(normal) => normal.sample(&mut rng),
          Err(_) => 0.0,
        },
      },
    )
  }
}
impl<A, M, C, S: AsRef<str> + std::cmp::Ord> PlatformDescription<A, M, C, S> {
  pub fn map<T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<PlatformDescription<T::Audio, T::Model, T::Custom, S>, T::Error> {
    Ok(PlatformDescription {
      base: self.base,
      contents: self.contents.into_iter().map(|c| c.map(mapper)).collect::<Result<_, _>>()?,
      length: self.length,
      material: self.material,
      sprays: self.sprays,
      spray_blank_weight: self.spray_blank_weight,
      walls: self.walls,
      width: self.width,
      x: self.x,
      y: self.y,
      z: self.z,
    })
  }
}
impl<A: 'static, M: 'static, S: AsRef<str> + std::hash::Hash + std::fmt::Display + std::cmp::Ord + std::cmp::Eq + Clone + 'static>
  PuzzleCustom<A, M, S>
{
  pub fn map<C: 'static, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<PuzzleCustom<T::Audio, T::Model, S>, T::Error> {
    let mut settings = std::collections::BTreeMap::new();
    for (key, value) in self.settings {
      settings.insert(
        key,
        match value {
          PuzzleCustomSetting::Audio(value) => PuzzleCustomSetting::Audio(mapper.resolve_audio(value)?),
          PuzzleCustomSetting::Bool(value) => PuzzleCustomSetting::Bool(value),
          PuzzleCustomSetting::Color(value) => PuzzleCustomSetting::Color(value),
          PuzzleCustomSetting::Intensity(value) => PuzzleCustomSetting::Intensity(value),
          PuzzleCustomSetting::Mesh(value) => PuzzleCustomSetting::Mesh(mapper.resolve_model(value)?),
          PuzzleCustomSetting::Num(value) => PuzzleCustomSetting::Num(value),
          PuzzleCustomSetting::Realm(link) => PuzzleCustomSetting::Realm(link),
        },
      );
    }
    let mut custom_mapper = PuzzleCustomResourceMapper(mapper);
    Ok(PuzzleCustom {
      ambient_audio: self.ambient_audio.into_iter().map(|a| a.map(&mut custom_mapper)).collect::<Result<_, _>>()?,
      event_audio: self.event_audio.into_iter().map(|a| a.map(&mut custom_mapper)).collect::<Result<_, _>>()?,
      gradiators_color: self.gradiators_color,
      gradiators_intensity: self.gradiators_intensity,
      ground: self.ground,
      lights: self.lights,
      logic: self.logic,
      materials: self.materials,
      meshes: self.meshes,
      settings,
      propagation_rules: self.propagation_rules,
    })
  }
  pub fn validate(&self, name: &str) -> Result<std::collections::HashSet<PuzzleCustomInternalId<S>>, String> {
    fn check_global_value<A, M, T: SettingEquivalent<S>, S: AsRef<str> + std::hash::Hash + std::fmt::Display + std::cmp::Ord + Clone>(
      name: &str,
      value: &GlobalValue<T, S>,
      defined_names: &mut std::collections::HashSet<PuzzleCustomInternalId<S>>,
      settings: &std::collections::BTreeMap<S, PuzzleCustomSetting<A, M, S>>,
    ) -> Result<(), String> {
      match value {
        GlobalValue::PuzzleBool { id, .. } => {
          defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::BoolSink(id.clone())));
          Ok(())
        }
        GlobalValue::PuzzleNum { id, .. } => {
          defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::NumSink(id.clone())));
          Ok(())
        }
        GlobalValue::Setting(setting) => match settings.get(setting) {
          None => Err(format!("Global value {} references non-existent setting {}.", name, setting)),
          Some(s) => {
            if T::can_be_set_from_custom(s) {
              Ok(())
            } else {
              Err(format!("Gradiator {} references setting {} which is of type {}.", name, setting, s.type_name()))
            }
          }
        },
        GlobalValue::SettingBool { id, .. } => match settings.get(id) {
          None => Err(format!("Global value {} references non-existent setting {}.", name, id)),
          Some(s) => {
            if matches!(s, PuzzleCustomSetting::Bool(_)) {
              Ok(())
            } else {
              Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
            }
          }
        },
        GlobalValue::SettingNum { id, .. } => match settings.get(id) {
          None => Err(format!("Global value {} references non-existent setting {}.", name, id)),
          Some(s) => {
            if matches!(s, PuzzleCustomSetting::Num(_)) {
              Ok(())
            } else {
              Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
            }
          }
        },
        _ => Ok(()),
      }
    }
    fn check_local_blendable_value<A, M, T: SettingEquivalent<S>, S: AsRef<str> + std::hash::Hash + std::fmt::Display + std::cmp::Ord + Clone>(
      name: &str,
      value: &LocalBlendableValue<T, S>,
      defined_names: &mut std::collections::HashSet<PuzzleCustomInternalId<S>>,
      settings: &std::collections::BTreeMap<S, PuzzleCustomSetting<A, M, S>>,
      gradiators: &std::collections::BTreeSet<S>,
    ) -> Result<(), String> {
      match value {
        LocalBlendableValue::Altitude { top_limit, bottom_limit, .. } => {
          if top_limit <= bottom_limit {
            Err(format!("Altitude ranges from {} to {} which is not valid", bottom_limit, top_limit))
          } else {
            Ok(())
          }
        }
        LocalBlendableValue::Global(value) => check_global_value(name, value, defined_names, settings),
        LocalBlendableValue::Gradiator(gradiator) => {
          if gradiators.contains(gradiator) {
            Ok(())
          } else {
            Err(format!("Local value {} references gradiator {} which does not exist.", name, gradiator))
          }
        }
        LocalBlendableValue::RandomLocal(items) => {
          if items.is_empty() {
            Err(format!("{} has no item in random set", name))
          } else {
            Ok(())
          }
        }
      }
    }
    fn check_local_discrete_value<A, M, T: SettingEquivalent<S>, S: AsRef<str> + std::hash::Hash + std::fmt::Display + std::cmp::Ord + Clone>(
      name: &str,
      value: &LocalDiscreteValue<T, S>,
      defined_names: &mut std::collections::HashSet<PuzzleCustomInternalId<S>>,
      settings: &std::collections::BTreeMap<S, PuzzleCustomSetting<A, M, S>>,
    ) -> Result<(), String> {
      match value {
        LocalDiscreteValue::Global(value) => check_global_value(name, value, defined_names, settings),
        LocalDiscreteValue::RandomLocal(items) => {
          if items.is_empty() {
            Err(format!("{} has no item in random set", name))
          } else {
            Ok(())
          }
        }
      }
    }

    fn check_light<A, M, S: AsRef<str> + std::hash::Hash + std::fmt::Display + std::cmp::Ord + Clone>(
      name: &str,
      value: &Light<GlobalValue<f64, S>, GlobalValue<Color, S>>,
      defined_names: &mut std::collections::HashSet<PuzzleCustomInternalId<S>>,
      settings: &std::collections::BTreeMap<S, PuzzleCustomSetting<A, M, S>>,
    ) -> Result<(), String> {
      match value {
        Light::Point { color, intensity, .. } => {
          check_global_value(&format!("color of point light in {}", name), color, defined_names, settings)?;
          check_global_value(&format!("intensity of point light in {}", name), intensity, defined_names, settings)
        }
      }
    }

    let mut defined_names: std::collections::HashSet<_> =
      (0..self.logic.len()).into_iter().map(|id| PuzzleCustomInternalId::Logic(id as u32)).collect();
    for (id, audio) in self.ambient_audio.iter().enumerate() {
      check_global_value(&format!("volume of ambient audio {} in {}", id, name), &audio.volume, &mut defined_names, &self.settings)?;
    }

    for (id, audio) in self.event_audio.iter().enumerate() {
      check_global_value(&format!("volume of event audio {} in {}", id, name), &audio.volume, &mut defined_names, &self.settings)?;
      defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::EventSink(audio.name.clone())));
    }

    for (id, material) in &self.materials {
      let material = match material {
        PuzzleCustomMaterial::Fixed(material) => material,
        PuzzleCustomMaterial::Replaceable { default, .. } => default,
      };
      match material {
        Material::BrushedMetal { color } => {
          check_local_blendable_value(&format!("color of brushed metal {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::Crystal { color, opacity } => {
          check_local_blendable_value(&format!("color of crystal {}", id), color, &mut defined_names, &self.settings, &self.gradiators_color)?;
          check_local_blendable_value(
            &format!("opacity of crystal {}", id),
            opacity,
            &mut defined_names,
            &self.settings,
            &self.gradiators_intensity,
          )?;
        }
        Material::Gem { color, accent, glow } => {
          check_local_blendable_value(&format!("color of gem {}", id), color, &mut defined_names, &self.settings, &self.gradiators_color)?;
          if let Some(accent) = accent {
            check_local_blendable_value(&format!("accent color of gem {}", id), accent, &mut defined_names, &self.settings, &self.gradiators_color)?;
          }
          check_local_discrete_value(&format!("glow of gem {}", id), glow, &mut defined_names, &self.settings)?;
        }
        Material::Metal { color, corrosion } => {
          check_local_blendable_value(&format!("color of metal {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?;
          if let Some((corrosion, intensity)) = corrosion {
            check_local_blendable_value(
              &format!("corrosion color of metal {}", id),
              &corrosion,
              &mut defined_names,
              &self.settings,
              &self.gradiators_color,
            )?;
            check_local_blendable_value(
              &format!("corrosion intensity of metal {}", id),
              &intensity,
              &mut defined_names,
              &self.settings,
              &self.gradiators_intensity,
            )?
          }
        }
        Material::Rock { color } => {
          check_local_blendable_value(&format!("color of rock {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::Sand { color } => {
          check_local_blendable_value(&format!("color of sand {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::ShinyMetal { color } => {
          check_local_blendable_value(&format!("color of shiny metal {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::Soil { color } => {
          check_local_blendable_value(&format!("color of soil {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::Textile { color } => {
          check_local_blendable_value(&format!("color of textile {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
        Material::TreadPlate { color, corrosion } => {
          check_local_blendable_value(&format!("color of treadplate {}", id), &color, &mut defined_names, &self.settings, &self.gradiators_color)?;
          if let Some(corrosion) = corrosion {
            check_local_blendable_value(
              &format!("corrosion color of treadplate {}", id),
              &corrosion,
              &mut defined_names,
              &self.settings,
              &self.gradiators_color,
            )?
          }
        }
        Material::Wood { background, grain } => {
          check_local_blendable_value(
            &format!("color of wood background {}", id),
            &background,
            &mut defined_names,
            &self.settings,
            &self.gradiators_color,
          )?;
          check_local_blendable_value(&format!("color of wood_grain {}", id), &grain, &mut defined_names, &self.settings, &self.gradiators_color)?
        }
      }
    }

    let mut radio_buttons: std::collections::BTreeMap<_, std::collections::BTreeSet<_>> = std::collections::BTreeMap::new();

    for (id, light) in self.lights.iter().enumerate() {
      match light {
        PuzzleCustomLight::Static(light) => check_light(&format!("light {} in {}", id, name), light, &mut defined_names, &self.settings)?,
        PuzzleCustomLight::Output { light, id } => {
          check_light(&format!("light {} in {}", id, name), light, &mut defined_names, &self.settings)?;
          defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::BoolSink(id.clone())));
        }
        PuzzleCustomLight::Select { lights, id } => {
          for (light_number, light) in lights.iter().enumerate() {
            check_light(&format!("option {} in light {} in {}", light_number, id, name), light, &mut defined_names, &self.settings)?;
          }
          defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::NumSink(id.clone())));
        }
      }
    }
    for (id, mesh) in self.meshes.iter().enumerate() {
      match mesh {
        PuzzleCustomModel::Button { elements, name, .. } => {
          for (index, element) in elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in button in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          if elements.is_empty() {
            return Err(format!("Button {} in {} has no meshes", id, name));
          }
          defined_names.insert(PuzzleCustomInternalId::Interact(crate::realm::InteractionKey::Button(name.clone())));
        }
        PuzzleCustomModel::Output { common_elements, elements, name, .. } => {
          for (index, element) in common_elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Common mesh {} in output in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          for (state, elements) in elements.iter().enumerate() {
            for (index, element) in elements.iter().enumerate() {
              if !self.materials.contains_key(&element.material) {
                return Err(format!(
                  "Mesh {} for state {} in output in item {} in {} references material {} which does not exist",
                  index, state, id, name, &element.material
                ));
              }
            }
            if elements.is_empty() {
              return Err(format!("State {} in output {} in {} has no meshes", state, id, name));
            }
          }
          if elements.is_empty() {
            return Err(format!("Output {} in {} has no states", id, name));
          }
          defined_names.insert(PuzzleCustomInternalId::Property(crate::realm::PropertyKey::NumSink(name.clone())));
        }
        PuzzleCustomModel::RadioButton { elements, on_elements, off_elements, value, name, .. } => {
          for (index, element) in elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in radio button common element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          for (index, element) in on_elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in radio button 'on' element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          for (index, element) in off_elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in radio button 'off' element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          if on_elements.is_empty() && off_elements.is_empty() || elements.is_empty() && (on_elements.is_empty() || off_elements.is_empty()) {
            return Err(format!("Radio button {} in {} has no meshes that distinguish between on and off states", id, name));
          }
          if radio_buttons.entry(name).or_default().insert(value) {
            return Err(format!("Radio button {} in {} has duplicate value {}", id, name, value));
          }
          defined_names.insert(PuzzleCustomInternalId::Interact(crate::realm::InteractionKey::RadioButton(name.clone())));
        }
        PuzzleCustomModel::RealmSelector { elements, name, .. } => {
          for (index, element) in elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in realm selector in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          if elements.is_empty() {
            return Err(format!("Realm selector {} in {} has no meshes", id, name));
          }

          defined_names.insert(PuzzleCustomInternalId::Interact(crate::realm::InteractionKey::RealmSelector(name.clone())));
        }
        PuzzleCustomModel::Static { elements, .. } => {
          for (index, element) in elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in static display in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          if elements.is_empty() {
            return Err(format!("Static display {} in {} has no meshes", id, name));
          }
        }
        PuzzleCustomModel::Switch { elements, on_elements, off_elements, name, .. } => {
          for (index, element) in elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in switch common element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          for (index, element) in on_elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in switch 'on' element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          for (index, element) in off_elements.iter().enumerate() {
            if !self.materials.contains_key(&element.material) {
              return Err(format!(
                "Mesh {} in switch 'off' element in item {} in {} references material {} which does not exist",
                index, id, name, &element.material
              ));
            }
          }
          if on_elements.is_empty() && off_elements.is_empty() || elements.is_empty() && (on_elements.is_empty() || off_elements.is_empty()) {
            return Err(format!("Switch {} in {} has no meshes that distinguish between on and off states", id, name));
          }
          defined_names.insert(PuzzleCustomInternalId::Interact(crate::realm::InteractionKey::Switch(name.clone())));
        }
      }
    }
    for ground in self.ground.iter().flat_map(|g| g.iter()).flat_map(|o| o.iter()) {
      match ground {
        PuzzleCustomGround::Proximity(id) => {
          defined_names.insert(PuzzleCustomInternalId::Proximity(*id));
        }
        PuzzleCustomGround::Solid => (),
        PuzzleCustomGround::Suppress => (),
      }
    }

    for (id, propagation_rule) in self.propagation_rules.iter().enumerate() {
      if !defined_names.contains(&propagation_rule.sender) {
        return Err(format!("Propagation {} wants to has non-existent sender in {}", id, name));
      }
      if !defined_names.contains(&propagation_rule.recipient) {
        return Err(format!("Propagation {} wants to has non-existent recipient in {}", id, name));
      }
    }

    Ok(defined_names)
  }
}
impl<A: ExtractChildren<S>, M: ExtractChildren<S>, S: AsRef<str> + std::cmp::Ord + std::hash::Hash> ExtractChildren<S> for PuzzleCustom<A, M, S> {
  fn extract_children<'a>(&'a self, assets: &mut std::collections::BTreeSet<S>) {
    for ambient_audio in &self.ambient_audio {
      match &ambient_audio.sound {
        AmbientAudioSound::Asset(PuzzleCustomAsset::Fixed(a)) => {
          a.extract_children(assets);
        }
        _ => (),
      }
    }
    for event_audio in &self.event_audio {
      match &event_audio.sound {
        PuzzleCustomAsset::Fixed(a) => {
          a.extract_children(assets);
        }
        _ => (),
      }
    }
    for setting in self.settings.values() {
      match setting {
        PuzzleCustomSetting::Audio(a) => {
          a.extract_children(assets);
        }
        PuzzleCustomSetting::Bool(_) => (),
        PuzzleCustomSetting::Color(_) => (),
        PuzzleCustomSetting::Intensity(_) => (),
        PuzzleCustomSetting::Mesh(a) => {
          a.extract_children(assets);
        }
        PuzzleCustomSetting::Num(_) => (),
        PuzzleCustomSetting::Realm(_) => (),
      }
    }
  }
}
impl<'a, A: 'static, M: 'static, C: 'static, S: AsRef<str> + 'static, R> ResourceMapper<PuzzleCustomAsset<A, S>, PuzzleCustomAsset<M, S>, C>
  for PuzzleCustomResourceMapper<'a, R>
where
  R: ResourceMapper<A, M, C>,
{
  type Audio = PuzzleCustomAsset<R::Audio, S>;

  type Custom = R::Custom;

  type Model = PuzzleCustomAsset<R::Model, S>;

  type Error = R::Error;

  fn resolve_audio(&mut self, audio: PuzzleCustomAsset<A, S>) -> Result<Self::Audio, Self::Error> {
    Ok(match audio {
      PuzzleCustomAsset::Fixed(audio) => PuzzleCustomAsset::Fixed(self.0.resolve_audio(audio)?),
      PuzzleCustomAsset::Setting(setting) => PuzzleCustomAsset::Setting(setting),
    })
  }

  fn resolve_custom(&mut self, custom: C) -> Result<Self::Custom, Self::Error> {
    self.0.resolve_custom(custom)
  }

  fn resolve_model(&mut self, model: PuzzleCustomAsset<M, S>) -> Result<Self::Model, Self::Error> {
    Ok(match model {
      PuzzleCustomAsset::Fixed(model) => PuzzleCustomAsset::Fixed(self.0.resolve_model(model)?),
      PuzzleCustomAsset::Setting(setting) => PuzzleCustomAsset::Setting(setting),
    })
  }
}
impl<A, M, S: AsRef<str>> PuzzleCustomSetting<A, M, S> {
  fn type_name(&self) -> &'static str {
    match self {
      PuzzleCustomSetting::Audio(_) => "audio",
      PuzzleCustomSetting::Bool(_) => "Boolean",
      PuzzleCustomSetting::Color(_) => "color",
      PuzzleCustomSetting::Intensity(_) => "intensity",
      PuzzleCustomSetting::Mesh(_) => "mesh",
      PuzzleCustomSetting::Num(_) => "number",
      PuzzleCustomSetting::Realm(_) => "realm",
    }
  }
}
impl<A, M, S: AsRef<str>> PuzzleCustomSettingValue<A, M, S> {
  pub fn map<C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<PuzzleCustomSettingValue<T::Audio, T::Model, S>, T::Error> {
    Ok(match self {
      PuzzleCustomSettingValue::Audio(v) => PuzzleCustomSettingValue::Audio(v.map(|a| mapper.resolve_audio(a))?),
      PuzzleCustomSettingValue::Bool(v) => PuzzleCustomSettingValue::Bool(v),
      PuzzleCustomSettingValue::Color(v) => PuzzleCustomSettingValue::Color(v),
      PuzzleCustomSettingValue::Intensity(v) => PuzzleCustomSettingValue::Intensity(v),
      PuzzleCustomSettingValue::Mesh(v) => PuzzleCustomSettingValue::Mesh(v.map(|m| mapper.resolve_model(m))?),
      PuzzleCustomSettingValue::Num(v) => PuzzleCustomSettingValue::Num(v),
      PuzzleCustomSettingValue::Realm(v) => PuzzleCustomSettingValue::Realm(v),
    })
  }
}
impl<A, M, C, S: AsRef<str> + std::cmp::Ord> PlatformItem<A, M, C, S> {
  pub fn map<T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<PlatformItem<T::Audio, T::Model, T::Custom, S>, T::Error> {
    Ok(PlatformItem { x: self.x, y: self.y, item: self.item.map(mapper)? })
  }
}
impl<A, M, C, S: AsRef<str> + std::cmp::Ord> PuzzleItem<A, M, C, S> {
  pub fn map<T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<PuzzleItem<T::Audio, T::Model, T::Custom, S>, T::Error> {
    Ok(match self {
      PuzzleItem::Button { arguments, enabled, matcher, model, name, transformation } => {
        PuzzleItem::Button { arguments, enabled, matcher, model: mapper.resolve_model(model)?, name, transformation }
      }
      PuzzleItem::Switch { arguments, enabled, initial, matcher, model, name, transformation } => {
        PuzzleItem::Switch { arguments, enabled, initial, matcher, model: mapper.resolve_model(model)?, name, transformation }
      }
      PuzzleItem::CycleButton { arguments, enabled, matcher, model, name, states, transformation } => {
        PuzzleItem::CycleButton { arguments, enabled, matcher, model: mapper.resolve_model(model)?, states, name, transformation }
      }
      PuzzleItem::CycleDisplay { arguments, model, name, states, transformation } => {
        PuzzleItem::CycleDisplay { arguments, model: mapper.resolve_model(model)?, states, name, transformation }
      }
      PuzzleItem::RealmSelector { arguments, matcher, model, name, transformation } => {
        PuzzleItem::RealmSelector { arguments, matcher, model: mapper.resolve_model(model)?, name, transformation }
      }
      PuzzleItem::Display { arguments, model, transformation } => {
        PuzzleItem::Display { arguments, model: mapper.resolve_model(model)?, transformation }
      }
      PuzzleItem::Custom { item, transformation, gradiators_color, gradiators_intensity, materials, settings } => PuzzleItem::Custom {
        item: mapper.resolve_custom(item)?,
        transformation,
        gradiators_color,
        gradiators_intensity,
        materials,
        settings: settings
          .into_iter()
          .map(|(key, value)| match value.map(mapper) {
            Ok(v) => Ok((key, v)),
            Err(e) => Err(e),
          })
          .collect::<Result<_, _>>()?,
      },
      PuzzleItem::Proximity { name, width, length, matcher } => PuzzleItem::Proximity { name, width, length, matcher },
    })
  }
}
impl<M, S: AsRef<str>> Spray<M, S> {
  pub fn map<A, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<Spray<T::Model, S>, T::Error> {
    Ok(Spray {
      angle: self.angle,
      elements: self.elements.into_iter().map(|i| i.map(mapper)).collect::<Result<_, _>>()?,
      vertical: self.vertical,
      vertical_perturbation: self.vertical_perturbation,
      visible: self.visible,
    })
  }
}
impl<M, S: AsRef<str>> SprayElement<M, S> {
  pub fn map<A, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<SprayElement<T::Model, S>, T::Error> {
    Ok(SprayElement { arguments: self.arguments, model: mapper.resolve_model(self.model)?, weight: self.weight })
  }
}
impl Transformation {
  pub fn flip_horizontal(self) -> Self {
    match self {
      Transformation::N => Transformation::H,
      Transformation::H => Transformation::N,
      Transformation::V => Transformation::VH,
      Transformation::C => Transformation::AV,
      Transformation::A => Transformation::CV,
      Transformation::AV => Transformation::C,
      Transformation::CV => Transformation::A,
      Transformation::VH => Transformation::V,
    }
  }
  pub fn flip_vertical(self) -> Self {
    match self {
      Transformation::N => Transformation::V,
      Transformation::H => Transformation::VH,
      Transformation::V => Transformation::N,
      Transformation::C => Transformation::CV,
      Transformation::A => Transformation::AV,
      Transformation::AV => Transformation::A,
      Transformation::CV => Transformation::C,
      Transformation::VH => Transformation::V,
    }
  }
  pub fn rotate90(self) -> Self {
    match self {
      Transformation::N => Transformation::C,
      Transformation::H => Transformation::CV,
      Transformation::V => Transformation::AV,
      Transformation::C => Transformation::VH,
      Transformation::A => Transformation::N,
      Transformation::AV => Transformation::H,
      Transformation::CV => Transformation::V,
      Transformation::VH => Transformation::A,
    }
  }
  pub fn rotate_m90(self) -> Self {
    match self {
      Transformation::N => Transformation::A,
      Transformation::H => Transformation::AV,
      Transformation::V => Transformation::CV,
      Transformation::C => Transformation::N,
      Transformation::A => Transformation::VH,
      Transformation::AV => Transformation::V,
      Transformation::CV => Transformation::H,
      Transformation::VH => Transformation::C,
    }
  }
  pub fn map_range(&self, x: u32, width: u32, y: u32, length: u32) -> (std::ops::RangeInclusive<u32>, std::ops::RangeInclusive<u32>) {
    match self {
      Transformation::N | Transformation::H | Transformation::V | Transformation::VH => (x..=(x + width), y..=(y + length)),
      Transformation::A | Transformation::C | Transformation::AV | Transformation::CV => (x..=(x + length), y..=(y + width)),
    }
  }
  pub fn map_child_ranges(
    &self,
    outer_x: u32,
    outer_y: u32,
    outer_width: u32,
    outer_length: u32,
    inner_x: u32,
    inner_y: u32,
    width: u32,
    length: u32,
  ) -> Option<(std::ops::RangeInclusive<u32>, std::ops::RangeInclusive<u32>)> {
    let (start_x, x_distance, start_y, y_distance) = match self {
      Transformation::N => (outer_x + inner_x, width, outer_y + inner_y, length),
      Transformation::H => (outer_x + outer_width.checked_sub(inner_x + width)?, width, outer_y + inner_y, length),
      Transformation::V => (outer_x + inner_x, width, outer_y + outer_length.checked_sub(inner_y + length)?, length),
      Transformation::C => ((outer_x + outer_length).checked_sub(inner_y + length)?, length, outer_y + inner_x, width),
      Transformation::A => (outer_x + inner_y, length, (outer_y + outer_width).checked_sub(inner_x + width)?, width),
      Transformation::AV => {
        ((outer_x + outer_length).checked_sub(inner_y + length)?, length, (outer_y + outer_width).checked_sub(inner_x + width)?, width)
      }
      Transformation::CV => (outer_x + inner_y, length, outer_y + inner_x, width),
      Transformation::VH => {
        (outer_x + outer_width.checked_sub(inner_x + width)?, width, outer_y + outer_length.checked_sub(inner_y + length)?, length)
      }
    };
    Some((start_x..=(start_x + x_distance), start_y..=(start_y + y_distance)))
  }
  pub fn translate(&self, outer_x: u32, outer_y: u32, outer_width: u32, outer_length: u32, inner_x: u32, inner_y: u32) -> Option<(u32, u32)> {
    Some(match self {
      Transformation::N => (outer_x + inner_x, outer_y + inner_y),
      Transformation::H => (outer_x + outer_width.checked_sub(inner_x)?, outer_y + inner_y),
      Transformation::V => (outer_x + inner_x, outer_y + outer_length.checked_sub(inner_y)?),
      Transformation::C => ((outer_x + outer_length).checked_sub(inner_y)?, outer_y + inner_x),
      Transformation::A => (outer_x + inner_y, (outer_y + outer_width).checked_sub(inner_x)?),
      Transformation::AV => ((outer_x + outer_length).checked_sub(inner_y)?, (outer_y + outer_width).checked_sub(inner_x)?),
      Transformation::CV => (outer_x + inner_y, outer_y + inner_x),
      Transformation::VH => (outer_x + outer_width.checked_sub(inner_x)?, outer_y + outer_length.checked_sub(inner_y)?),
    })
  }
}
impl<M, S: AsRef<str>> Wall<M, S> {
  pub fn map<A, C, T: ResourceMapper<A, M, C>>(self, mapper: &mut T) -> Result<Wall<T::Model, S>, T::Error> {
    Ok(match self {
      Wall::Solid { width, width_perturbation, material } => Wall::Solid { width, width_perturbation, material },
      Wall::Fence { angle, posts, vertical, vertical_perturbation } => {
        Wall::Fence { angle, posts: posts.into_iter().map(|s| s.map(mapper)).collect::<Result<_, _>>()?, vertical, vertical_perturbation }
      }
      Wall::Gate { angle, arguments, identifier, model, vertical, vertical_perturbation } => {
        Wall::Gate { angle, arguments, identifier, model: mapper.resolve_model(model)?, vertical, vertical_perturbation }
      }
      Wall::Block { angle, arguments, identifier, model, vertical, vertical_perturbation } => {
        Wall::Block { angle, arguments, identifier, model: mapper.resolve_model(model)?, vertical, vertical_perturbation }
      }
    })
  }
}
fn check_global_value<T: SettingEquivalent<S>, S: AsRef<str> + std::cmp::Eq + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone>(
  name: &str,
  value: &GlobalValue<T, S>,
  defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
  masks: &std::collections::BTreeMap<S, MaskConfiguration>,
  settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
) -> Result<(), String> {
  match value {
    GlobalValue::PuzzleBool { id, .. } => {
      defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::BoolSink(id.clone())));
      Ok(())
    }
    GlobalValue::PuzzleNum { id, .. } => {
      defined_names.insert(SimpleRealmPuzzleId::Property(crate::realm::PropertyKey::NumSink(id.clone())));
      Ok(())
    }
    GlobalValue::Masked(mask) => match masks.get(mask).map(T::included_in_mask) {
      None => Err(format!("Mask {} does not exist as needed in {}.", mask, name)),
      Some(false) => Err(format!("Mask {} cannot provide required type of value in {}.", mask, name)),
      Some(true) => Ok(()),
    },
    GlobalValue::Setting(setting) => match settings.get(setting) {
      None => Err(format!("Global value {} references non-existent setting {}.", name, setting)),
      Some(s) => {
        if T::can_be_set_from(s) {
          Ok(())
        } else {
          Err(format!("Gradiator {} references setting {} which is of type {}.", name, setting, s.type_name()))
        }
      }
    },
    GlobalValue::SettingBool { id, .. } => match settings.get(id) {
      None => Err(format!("Global value {} references non-existent setting {}.", name, id)),
      Some(s) => {
        if matches!(s, crate::realm::RealmSetting::Bool(_)) {
          Ok(())
        } else {
          Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
        }
      }
    },
    GlobalValue::SettingNum { id, .. } => match settings.get(id) {
      None => Err(format!("Global value {} references non-existent setting {}.", name, id)),
      Some(s) => {
        if matches!(s, crate::realm::RealmSetting::Num(_)) {
          Ok(())
        } else {
          Err(format!("Gradiator {} references setting {} which is of type {}.", name, id, s.type_name()))
        }
      }
    },
    _ => Ok(()),
  }
}

fn check_local_blendable_value<
  T: SettingEquivalent<S>,
  S: AsRef<str> + std::cmp::Eq + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone,
>(
  name: &str,
  value: &LocalBlendableValue<T, S>,
  defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
  settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
  masks: &std::collections::BTreeMap<S, MaskConfiguration>,
  gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<T, S>>,
) -> Result<(), String> {
  match value {
    LocalBlendableValue::Altitude { top_limit, bottom_limit, .. } => {
      if top_limit <= bottom_limit {
        Err(format!("Altitude ranges from {} to {} which is not valid", bottom_limit, top_limit))
      } else {
        Ok(())
      }
    }
    LocalBlendableValue::Global(value) => check_global_value(name, value, defined_names, masks, settings),
    LocalBlendableValue::Gradiator(gradiator) => {
      if gradiators.contains_key(gradiator) {
        Ok(())
      } else {
        Err(format!("Local value {} references gradiator {} which does not exist.", name, gradiator))
      }
    }
    LocalBlendableValue::RandomLocal(items) => {
      if items.is_empty() {
        Err(format!("{} has no item in random set", name))
      } else {
        Ok(())
      }
    }
  }
}
fn check_local_discrete_value<T: SettingEquivalent<S>, S: AsRef<str> + std::cmp::Eq + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone>(
  name: &str,
  value: &LocalDiscreteValue<T, S>,
  defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
  settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
  masks: &std::collections::BTreeMap<S, MaskConfiguration>,
) -> Result<(), String> {
  match value {
    LocalDiscreteValue::Global(value) => check_global_value(name, value, defined_names, masks, settings),
    LocalDiscreteValue::RandomLocal(items) => {
      if items.is_empty() {
        Err(format!("{} has no item in random set", name))
      } else {
        Ok(())
      }
    }
  }
}
impl<S: AsRef<str> + std::cmp::Eq + std::hash::Hash> std::fmt::Display for SimpleRealmPuzzleId<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SimpleRealmPuzzleId::Custom { platform, item, name } => {
        f.write_str("custom ")?;
        platform.fmt(f)?;
        f.write_str("/")?;
        item.fmt(f)?;
        f.write_str(" ")?;
        name.fmt(f)
      }
      SimpleRealmPuzzleId::Interact(i) => {
        f.write_str("interact ")?;
        i.fmt(f)
      }
      SimpleRealmPuzzleId::Logic(l) => {
        f.write_str("logic ")?;
        l.fmt(f)
      }
      SimpleRealmPuzzleId::Map(m) => {
        f.write_str("map ")?;
        m.fmt(f)
      }
      SimpleRealmPuzzleId::Property(p) => {
        f.write_str("property ")?;
        p.fmt(f)
      }
      SimpleRealmPuzzleId::Proximity(n) => {
        f.write_str("proximity ")?;
        n.fmt(f)
      }
    }
  }
}

impl WallPath {
  pub fn plot_points(&self, mut consumer: impl FnMut(u32, u32) -> ()) {
    fn plot_line(x1: u32, y1: u32, x2: u32, y2: u32, consumer: &mut impl FnMut(u32, u32) -> ()) {
      if x1 == x2 {
        for y in y1..=y2 {
          consumer(x1, y);
        }
      } else if y1 == y2 {
        for x in x1..=x2 {
          consumer(x, y1);
        }
      } else {
        let dx = x2 as f64 - x1 as f64;
        let dy = y2 as f64 - y1 as f64;
        let step = if dx.abs() >= dy.abs() { dx.abs() } else { dy.abs() };
        let dx = dx / step;
        let dy = dy / step;
        let mut x = x1 as f64;
        let mut y = y1 as f64;
        let mut i = 1;
        let step = step as u32;
        while i <= step {
          consumer(x as u32, y as u32);
          x = x + dx;
          y = y + dy;
          i = i + 1;
        }
      }
    }
    // http://members.chello.at/easyfilter/bresenham.pdf
    match self {
      &WallPath::Line { x1, y1, x2, y2 } => {
        plot_line(x1, y1, x2, y2, &mut consumer);
      }
      &WallPath::Quadratic { x1, y1, x2, y2, xc, yc } => {
        let mut x1 = x1 as f64;
        let mut y1 = y1 as f64;
        let sx = if x1 < x2 as f64 { 1.0 } else { -1.0 };
        let sy = if y1 < y2 as f64 { 1.0 } else { -1.0 };
        let mut x = (x1 as f64) - 2.0 * (xc as f64) + (x2 as f64);
        let mut y = (y1 as f64) - 2.0 * (yc as f64) + (y2 as f64);
        let mut xy = 2.0 * (x as f64) * (y as f64) * (sx as f64) * (sy as f64);
        let curvature = sx * sy * (x * (y2 as f64 - yc as f64) - y * (x2 as f64 - xc as f64)) / 2.0;

        let mut dx =
          (1.0 - 2.0 * (x1 as f64 - xc as f64).abs()) * y * y + (y1 as f64 - yc as f64).abs() * xy - 2.0 * curvature * (y1 as f64 - y2 as f64).abs();

        let mut dy =
          (1.0 - 2.0 * (y1 as f64 - yc as f64).abs()) * x * x + (x1 as f64 - xc as f64).abs() * xy + 2.0 * curvature * (x1 as f64 - x2 as f64).abs();

        let mut ex =
          (1.0 - 2.0 * (x2 as f64 - xc as f64).abs()) * y * y + (y2 as f64 - yc as f64).abs() * xy + 2.0 * curvature * (y1 as f64 - y2 as f64).abs();
        let mut ey =
          (1.0 - 2.0 * (y2 as f64 - yc as f64).abs()) * x * x + (x2 as f64 - xc as f64).abs() * xy - 2.0 * curvature * (x1 as f64 - x2 as f64).abs();
        if (x1 - xc as f64) * (x2 as f64 - xc as f64) <= 0.0 && (y1 - yc as f64) * (y2 as f64 - yc as f64) <= 0.0 {
          if curvature == 0.0 {
            plot_line(x1 as u32, y1 as u32, x2, y2, &mut consumer);
          } else {
            x *= 2.0 * x;
            y *= 2.0 * y;
            if curvature < 0.0 {
              x = -x;
              dx = -dx;
              ex = -ex;
              xy = -xy;
              y = -y;
              dy = -dy;
              ey = -ey;
            }
            if dx >= -y || dy <= -x || ex <= -y || ey >= -x {
              x1 = (x1 + 4.0 * xc as f64 + x2 as f64) / 6.0;
              y1 = (y1 + 4.0 * yc as f64 + y2 as f64) / 6.0;
              plot_line(x1 as u32, y1 as u32, xc, yc, &mut consumer);
              plot_line(xc, yc, x2, y2, &mut consumer);
            } else {
              dx -= xy;
              ex = dx + dy;
              dy -= xy;

              loop {
                consumer(x1 as u32, y1 as u32);
                ey = 2.0 * ex - dy;
                if 2.0 * ex >= dx {
                  if x1 == x2 as f64 {
                    break;
                  }
                  x1 += sx;
                  dy -= xy;
                  dx += y;
                  ex += dx;
                }
                if ey <= 0.0 {
                  if y1 == y2 as f64 {
                    break;
                  }
                  y1 += sy;
                  dx -= xy;
                  dy += x;
                  ex += dy;
                }
              }
            }
          }
        }
      }
    }
  }
}

trait ValidatableArgument<S: AsRef<str> + std::cmp::Eq + std::hash::Hash + 'static>: 'static {
  type Context: Copy;
  fn check_argument(
    &self,
    name: &str,
    defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
    settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    masks: &std::collections::BTreeMap<S, MaskConfiguration>,
    materials: usize,
    color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
    intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
    context: Self::Context,
  ) -> Result<(), String>;
  fn argument_type(&self) -> ArgumentType;
}
#[derive(Copy, Clone, PartialEq, Eq)]
enum ArgumentType {
  Color,
  Material,
  Intensity,
}
impl std::fmt::Display for ArgumentType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      ArgumentType::Color => "color",
      ArgumentType::Material => "material",
      ArgumentType::Intensity => "intensity",
    })
  }
}
impl<S: AsRef<str> + std::cmp::Eq + std::hash::Hash + std::cmp::Ord + std::fmt::Display + Clone + 'static> ValidatableArgument<S> for Argument<S> {
  type Context = ();
  fn check_argument(
    &self,
    name: &str,
    defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
    settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    masks: &std::collections::BTreeMap<S, MaskConfiguration>,
    materials: usize,
    color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
    intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
    _: (),
  ) -> Result<(), String> {
    match self {
      Argument::Material(material) => {
        if *material as usize >= materials {
          return Err(format!("Material {} for {} is not valid.", material, name));
        }
      }
      Argument::Color(c) => {
        check_local_blendable_value(name, c, defined_names, settings, masks, color_gradiators)?;
      }
      Argument::Intensity(i) => {
        check_local_blendable_value(name, i, defined_names, settings, masks, intensity_gradiators)?;
      }
    }
    Ok(())
  }

  fn argument_type(&self) -> ArgumentType {
    match self {
      Argument::Color(_) => ArgumentType::Color,
      Argument::Intensity(_) => ArgumentType::Intensity,
      Argument::Material(_) => ArgumentType::Material,
    }
  }
}

impl<S: AsRef<str> + std::cmp::Eq + std::cmp::Ord + std::hash::Hash + std::fmt::Display + Clone + 'static> ValidatableArgument<S>
  for CycleArgument<S>
{
  type Context = u32;
  fn check_argument(
    &self,
    name: &str,
    defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
    settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    masks: &std::collections::BTreeMap<S, MaskConfiguration>,
    materials: usize,
    color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
    intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
    context: u32,
  ) -> Result<(), String> {
    match self {
      CycleArgument::Material(material) => {
        if *material as usize >= materials {
          return Err(format!("Material {} for {} is not valid.", material, name));
        }
      }
      CycleArgument::CycleMaterial(default, cycle_materials, _) => {
        if *default as usize >= materials {
          return Err(format!("Material {} for {} is not valid.", default, name));
        }
        if cycle_materials.len() != context as usize {
          return Err(format!("Got {} material for {} but should be {}.", cycle_materials.len(), name, context));
        }
        for material in cycle_materials {
          if *material as usize >= materials {
            return Err(format!("Material {} for {} is not valid.", material, name));
          }
        }
      }
      CycleArgument::Color(c) => {
        check_local_blendable_value(name, c, defined_names, settings, masks, color_gradiators)?;
      }
      CycleArgument::CycleColor(_, _, _) => (),
      CycleArgument::Intensity(i) => {
        check_local_blendable_value(name, i, defined_names, settings, masks, intensity_gradiators)?;
      }
      CycleArgument::CycleIntensity(_, _, _) => (),
    }
    Ok(())
  }

  fn argument_type(&self) -> ArgumentType {
    match self {
      CycleArgument::Material(_) | CycleArgument::CycleMaterial(_, _, _) => ArgumentType::Material,
      CycleArgument::Color(_) | CycleArgument::CycleColor(_, _, _) => ArgumentType::Color,
      CycleArgument::Intensity(_) | CycleArgument::CycleIntensity(_, _, _) => ArgumentType::Intensity,
    }
  }
}

impl<S: AsRef<str> + std::cmp::Eq + std::cmp::Ord + std::hash::Hash + std::fmt::Display + Clone + 'static> ValidatableArgument<S>
  for SwitchArgument<S>
{
  type Context = ();
  fn check_argument(
    &self,
    name: &str,
    defined_names: &mut std::collections::HashSet<SimpleRealmPuzzleId<S>>,
    settings: &std::collections::BTreeMap<S, crate::realm::RealmSetting<S>>,
    masks: &std::collections::BTreeMap<S, MaskConfiguration>,
    materials: usize,
    color_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<Color, S>>,
    intensity_gradiators: &std::collections::BTreeMap<S, gradiator::Gradiator<f64, S>>,
    _: (),
  ) -> Result<(), String> {
    match self {
      SwitchArgument::Material(material) => {
        if *material as usize >= materials {
          return Err(format!("Material {} for {} is not valid.", material, name));
        }
      }
      SwitchArgument::SwitchMaterial(on_material, off_material, _) => {
        if *on_material as usize >= materials {
          return Err(format!("“On” material {} for {} is not valid.", on_material, name));
        }
        if *off_material as usize >= materials {
          return Err(format!("“Off” material {} for {} is not valid.", off_material, name));
        }
      }
      SwitchArgument::Color(c) => {
        check_local_blendable_value(name, c, defined_names, settings, masks, color_gradiators)?;
      }
      SwitchArgument::SwitchColor(_, _, _) => (),
      SwitchArgument::Intensity(i) => {
        check_local_blendable_value(name, i, defined_names, settings, masks, intensity_gradiators)?;
      }
      SwitchArgument::SwitchIntensity(_, _, _) => (),
    }
    Ok(())
  }

  fn argument_type(&self) -> ArgumentType {
    match self {
      SwitchArgument::Material(_) | SwitchArgument::SwitchMaterial(_, _, _) => ArgumentType::Material,
      SwitchArgument::Color(_) | SwitchArgument::SwitchColor(_, _, _) => ArgumentType::Color,
      SwitchArgument::Intensity(_) | SwitchArgument::SwitchIntensity(_, _, _) => ArgumentType::Intensity,
    }
  }
}
impl Default for Transition {
  fn default() -> Self {
    Transition::Instant
  }
}
