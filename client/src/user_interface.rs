pub enum RealmSelector {
  Player,
  Local,
  Bookmarks,
  Remote(String),
  Url(String),
}
impl Default for RealmSelector {
  fn default() -> Self {
    RealmSelector::Player
  }
}
