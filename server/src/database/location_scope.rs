use crate::database::diesel_serde_jsonb::AsJsonb;
use crate::database::player_reference::PlayerReference;
use crate::database::schema;
use crate::database::schema::location::dsl as location_schema;
use diesel::expression::{AsExpression, SelectableExpression};
use diesel::prelude::*;
use diesel::sql_types::Bool;
use diesel::sqlite::Sqlite;
use diesel::BoxableExpression;
use spadina_core::location::directory::{SearchCriteria, TimeRange, Visibility};
use spadina_core::location::{Descriptor, DescriptorKind};
use spadina_core::reference_converter::AsReference;
use std::fmt::Debug;

diesel::postfix_operator!(NoCaseCollation, " COLLATE UNICODE_NOCASE ", Bool, backend: Sqlite);
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
  pub fn as_expression<'a, T: 'a>(&'a self) -> Box<dyn BoxableExpression<T, Sqlite, SqlType = Bool> + 'a>
  where
    location_schema::descriptor: SelectableExpression<T>,
    schema::player::dsl::id: SelectableExpression<T>,
    schema::player::dsl::name: SelectableExpression<T>,
  {
    use diesel::prelude::*;
    Box::new(self.owner.as_expression().and(location_schema::descriptor.eq(AsJsonb(self.descriptor.reference(AsReference::<str>::default())))))
  }
}
impl<S: AsRef<str> + Debug> LocationListScope<S> {
  pub fn as_expression<'a, T: 'a>(&'a self) -> Box<dyn BoxableExpression<T, Sqlite, SqlType = Bool> + 'a>
  where
    location_schema::created: SelectableExpression<T>,
    location_schema::descriptor: SelectableExpression<T>,
    location_schema::name: SelectableExpression<T>,
    location_schema::owner: SelectableExpression<T>,
    location_schema::updated_at: SelectableExpression<T>,
    location_schema::visibility: SelectableExpression<T>,
    schema::player::dsl::id: SelectableExpression<T>,
    schema::player::dsl::name: SelectableExpression<T>,
  {
    match self {
      LocationListScope::All => Box::new(<bool as AsExpression<Bool>>::as_expression(true)),
      LocationListScope::And(scopes) => scopes
        .into_iter()
        .map(|s| s.as_expression())
        .reduce(|l, r| Box::new(l.and(r)))
        .unwrap_or(Box::new(<bool as AsExpression<Bool>>::as_expression(false))),
      LocationListScope::Created(range) => match range {
        TimeRange::After(date) => Box::new(location_schema::created.gt(date.naive_utc())),
        TimeRange::Before(date) => Box::new(location_schema::created.lt(date.naive_utc())),
        TimeRange::In(start, end) => Box::new(location_schema::created.between(start.naive_utc(), end.naive_utc())),
      },
      LocationListScope::Exact(scope) => scope.as_expression(),
      LocationListScope::Kind(kind) => match kind {
        DescriptorKind::Asset(a) => Box::new(location_schema::descriptor.eq(AsJsonb(a.as_ref()))),
        DescriptorKind::Application(application) => Box::new(super::get_kind_from_descriptor(location_schema::descriptor).eq(AsJsonb(*application))),
        DescriptorKind::Unsupported(k) => Box::new(super::get_kind_from_descriptor(location_schema::descriptor).eq(AsJsonb(k.as_ref()))),
      },
      LocationListScope::NameContains(name, case_sensitive) => {
        if *case_sensitive {
          Box::new(location_schema::name.like(name.as_ref()).escape('*'))
        } else {
          Box::new(NoCaseCollation::new(location_schema::name.like(name.as_ref()).escape('*')))
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
        TimeRange::After(date) => Box::new(location_schema::updated_at.gt(date.naive_utc())),
        TimeRange::Before(date) => Box::new(location_schema::updated_at.lt(date.naive_utc())),
        TimeRange::In(start, end) => Box::new(location_schema::updated_at.between(start.naive_utc(), end.naive_utc())),
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
