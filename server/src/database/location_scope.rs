use crate::database::player_reference::PlayerReference;
use crate::database::schema;
use diesel::expression::{AsExpression, SelectableExpression};
use diesel::pg::Pg;
use diesel::sql_types::Bool;
use diesel::BoxableExpression;
use spadina_core::location::directory::{SearchCriteria, TimeRange, Visibility};
use spadina_core::location::{Descriptor, DescriptorKind};
use spadina_core::reference_converter::AsReference;
use std::fmt::Debug;

pub struct LocationScope<S: AsRef<str>> {
  pub owner: PlayerReference<S>,
  pub descriptor: Descriptor<S>,
}
pub(crate) enum LocationListScope<S: AsRef<str>> {
  All,
  And(Vec<LocationListScope<S>>),
  Created(TimeRange),
  Exact(LocationScope<S>),
  Kind(DescriptorKind<S>),
  NameContains(S, bool),
  Not(Box<LocationListScope<S>>),
  Or(Vec<LocationListScope<S>>),
  Owner(PlayerReference<S>),
  Updated(TimeRange),
  Visibility(Vec<Visibility>),
}

impl<S: AsRef<str>> LocationScope<S> {
  pub fn as_expression<'a, T: 'a>(&'a self) -> Box<dyn BoxableExpression<T, Pg, SqlType = Bool> + 'a>
  where
    schema::location::dsl::descriptor: SelectableExpression<T>,
    schema::player::dsl::id: SelectableExpression<T>,
    schema::player::dsl::name: SelectableExpression<T>,
  {
    use crate::database::schema::location::dsl as location_schema;
    use diesel::prelude::*;
    Box::new(
      self.owner.as_expression().and(location_schema::descriptor.eq(diesel_json::Json(self.descriptor.reference(AsReference::<str>::default())))),
    )
  }
}
impl<S: AsRef<str> + Debug> LocationListScope<S> {
  pub fn as_expression<'a, T: 'a>(&'a self) -> Box<dyn BoxableExpression<T, Pg, SqlType = Bool> + 'a>
  where
    schema::location::dsl::created: SelectableExpression<T>,
    schema::location::dsl::descriptor: SelectableExpression<T>,
    schema::location::dsl::name: SelectableExpression<T>,
    schema::location::dsl::owner: SelectableExpression<T>,
    schema::location::dsl::updated_at: SelectableExpression<T>,
    schema::location::dsl::visibility: SelectableExpression<T>,
    schema::player::dsl::id: SelectableExpression<T>,
    schema::player::dsl::name: SelectableExpression<T>,
  {
    use crate::database::schema::location::dsl as location_schema;
    use diesel::prelude::*;
    match self {
      LocationListScope::All => Box::new(<bool as AsExpression<Bool>>::as_expression(true)),
      LocationListScope::And(scopes) => scopes
        .into_iter()
        .map(|s| s.as_expression())
        .reduce(|l, r| Box::new(l.and(r)))
        .unwrap_or(Box::new(<bool as AsExpression<Bool>>::as_expression(false))),
      LocationListScope::Created(range) => match range {
        TimeRange::After(date) => Box::new(location_schema::created.gt(date)),
        TimeRange::Before(date) => Box::new(location_schema::created.lt(date)),
        TimeRange::In(start, end) => Box::new(location_schema::created.between(start, end)),
      },
      LocationListScope::Exact(scope) => scope.as_expression(),
      LocationListScope::Kind(kind) => match kind {
        DescriptorKind::Asset(a) => Box::new(location_schema::descriptor.eq(diesel_json::Json(a.as_ref()))),
        DescriptorKind::Application(application) => {
          Box::new(location_schema::descriptor.retrieve_by_path_as_object(vec!["1"]).eq(diesel_json::Json(*application)))
        }
        DescriptorKind::Unsupported(k) => Box::new(location_schema::descriptor.retrieve_by_path_as_text(vec!["1"]).eq(k.as_ref())),
      },
      LocationListScope::NameContains(name, case_sensitive) => {
        if *case_sensitive {
          Box::new(location_schema::name.like(name.as_ref()).escape('*'))
        } else {
          Box::new(location_schema::name.ilike(name.as_ref()).escape('*'))
        }
      }
      LocationListScope::Not(scope) => Box::new(diesel::dsl::not(scope.as_expression())),
      LocationListScope::Or(scopes) => scopes
        .into_iter()
        .map(|s| s.as_expression())
        .reduce(|l, r| Box::new(l.or(r)))
        .unwrap_or(Box::new(<bool as AsExpression<Bool>>::as_expression(false))),
      LocationListScope::Owner(owner) => owner.as_expression(),
      LocationListScope::Visibility(visibility) => {
        Box::new(location_schema::visibility.eq_any(visibility.iter().map(|v| *v as i16).collect::<Vec<_>>()))
      }
      LocationListScope::Updated(range) => match range {
        TimeRange::After(date) => Box::new(location_schema::updated_at.gt(date)),
        TimeRange::Before(date) => Box::new(location_schema::updated_at.lt(date)),
        TimeRange::In(start, end) => Box::new(location_schema::updated_at.between(start, end)),
      },
    }
  }
}
impl<S: AsRef<str>> From<SearchCriteria<S>> for LocationListScope<S> {
  fn from(value: SearchCriteria<S>) -> Self {
    match value {
      SearchCriteria::And(criteria) => LocationListScope::And(criteria.into_iter().map(|c| c.into()).collect()),
      SearchCriteria::Created(t) => LocationListScope::Created(t),
      SearchCriteria::Kind(v) => LocationListScope::Kind(v),
      SearchCriteria::NameContains { text, case_sensitive } => LocationListScope::NameContains(text, case_sensitive),
      SearchCriteria::Not(criterion) => LocationListScope::Not(Box::new(LocationListScope::from(*criterion))),
      SearchCriteria::Or(criteria) => LocationListScope::Or(criteria.into_iter().map(|c| c.into()).collect()),
      SearchCriteria::Player(player) => LocationListScope::Owner(PlayerReference::Name(player)),
      SearchCriteria::Updated(t) => LocationListScope::Updated(t),
    }
  }
}
