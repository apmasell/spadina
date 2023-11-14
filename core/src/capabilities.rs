pub const CAPABILITIES: &[&str] = &[];

pub fn all_supported<S: AsRef<str>>(capabilities: Vec<S>) -> Result<std::collections::BTreeSet<&'static str>, S> {
  capabilities
    .into_iter()
    .map(|cap| match CAPABILITIES.iter().find(|&&c| c == cap.as_ref()).copied() {
      Some(c) => Ok(c),
      None => Err(cap),
    })
    .collect()
}
