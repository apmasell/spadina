use crate::server::cache::Cacheable;

pub trait Export<C: ?Sized> {
  type Output<'a>: 'a
  where
    C: 'a;
  fn export<'a>(self, value: Option<&'a C>) -> Self::Output<'a>;
}
pub struct Cloned;
pub struct Inner;
pub struct ToJsonFile<P: AsRef<std::path::Path>>(pub P);
pub enum ToJsonError {
  Json(serde_json::Error),
  Io(std::io::Error),
  NotAvailable,
}
impl<C: Cacheable + Clone> Export<C> for Cloned {
  type Output<'a>
    = Option<C>
  where
    C: 'a;

  fn export<'a>(self, value: Option<&'a C>) -> Self::Output<'a> {
    value.cloned()
  }
}
impl<C: Cacheable> Export<C> for Inner {
  type Output<'a>
    = Option<&'a C>
  where
    C: 'a;

  fn export<'a>(self, value: Option<&'a C>) -> Self::Output<'a> {
    value
  }
}
impl<C: Cacheable + serde::ser::Serialize, P: AsRef<std::path::Path>> Export<C> for ToJsonFile<P> {
  type Output<'a>
    = Result<(), ToJsonError>
  where
    C: 'a;
  fn export<'a>(self, value: Option<&'a C>) -> Self::Output<'a> {
    if let Some(value) = value {
      serde_json::to_writer_pretty(std::fs::File::create(self.0)?, value)?;
      Ok(())
    } else {
      Err(ToJsonError::NotAvailable)
    }
  }
}
impl std::fmt::Display for ToJsonError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ToJsonError::Json(e) => e.fmt(f),
      ToJsonError::Io(e) => e.fmt(f),
      ToJsonError::NotAvailable => f.write_str("not available"),
    }
  }
}
impl From<serde_json::Error> for ToJsonError {
  fn from(value: serde_json::Error) -> Self {
    ToJsonError::Json(value)
  }
}

impl From<std::io::Error> for ToJsonError {
  fn from(value: std::io::Error) -> Self {
    ToJsonError::Io(value)
  }
}
