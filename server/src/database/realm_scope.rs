pub enum RealmScope<S: AsRef<str>> {
  Train { owner: i32, train: u16 },
  Asset { owner: i32, asset: S },
  NamedTrain { owner: S, train: u16 },
  NamedAsset { owner: S, asset: S },
}
pub(crate) enum RealmListScope<S: AsRef<str>> {
  Single(RealmScope<S>),
  All,
  Any(Vec<RealmListScope<S>>),
  InDirectory,
  Intersection(Vec<RealmListScope<S>>),
  Owner(i32),
  OwnerByName(S),
}

impl<S: AsRef<str>> RealmScope<S> {
  pub fn as_expression<'a, T>(&'a self) -> Box<dyn diesel::BoxableExpression<T, diesel::pg::Pg, SqlType = diesel::sql_types::Bool> + 'a>
  where
    crate::database::schema::realm::dsl::asset: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::owner: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::train: diesel::expression::SelectableExpression<T>,
    crate::database::schema::player::dsl::name: diesel::expression::SelectableExpression<T>,
  {
    use crate::database::schema::player::dsl as player_schema;
    use crate::database::schema::realm::dsl as realm_schema;
    use diesel::prelude::*;
    match self {
      RealmScope::Train { owner, train } => {
        Box::new(realm_schema::owner.eq(*owner).and(super::sql_coalesce_bool(realm_schema::train.eq(*train as i32), false)))
      }
      RealmScope::Asset { owner, asset } => Box::new(realm_schema::owner.eq(*owner).and(realm_schema::asset.eq(asset.as_ref()))),
      RealmScope::NamedTrain { owner, train } => {
        Box::new(player_schema::name.eq(owner.as_ref()).and(super::sql_coalesce_bool(realm_schema::train.eq(*train as i32), false)))
      }
      RealmScope::NamedAsset { owner, asset } => Box::new(player_schema::name.eq(owner.as_ref()).and(realm_schema::asset.eq(asset.as_ref()))),
    }
  }
}
impl<S: AsRef<str>> RealmListScope<S> {
  pub fn as_expression<'a, T: 'a>(&'a self) -> Box<dyn diesel::BoxableExpression<T, diesel::pg::Pg, SqlType = diesel::sql_types::Bool> + 'a>
  where
    crate::database::schema::player::dsl::name: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::asset: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::in_directory: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::owner: diesel::expression::SelectableExpression<T>,
    crate::database::schema::realm::dsl::train: diesel::expression::SelectableExpression<T>,
  {
    use crate::database::schema::player::dsl as player_schema;
    use crate::database::schema::realm::dsl as realm_schema;
    use diesel::prelude::*;
    match self {
      RealmListScope::Single(scope) => scope.as_expression(),
      RealmListScope::All => Box::new(<bool as diesel::expression::AsExpression<diesel::sql_types::Bool>>::as_expression(true)),
      RealmListScope::Any(scopes) => scopes
        .into_iter()
        .map(|s| s.as_expression())
        .reduce(|l, r| Box::new(l.or(r)))
        .unwrap_or(Box::new(<bool as diesel::expression::AsExpression<diesel::sql_types::Bool>>::as_expression(false))),
      RealmListScope::InDirectory => Box::new(realm_schema::in_directory),
      RealmListScope::Intersection(scopes) => scopes
        .into_iter()
        .map(|s| s.as_expression())
        .reduce(|l, r| Box::new(l.and(r)))
        .unwrap_or(Box::new(<bool as diesel::expression::AsExpression<diesel::sql_types::Bool>>::as_expression(false))),
      RealmListScope::Owner(owner) => Box::new(realm_schema::owner.eq(owner)),
      RealmListScope::OwnerByName(owner) => Box::new(player_schema::name.eq(owner.as_ref())),
    }
  }
}
