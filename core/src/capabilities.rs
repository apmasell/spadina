pub const CAPABILITIES: &[&str] = &[];

pub fn capabilities_from_header<T>(request: &http::Request<T>) -> Result<std::collections::BTreeSet<&'static str>, serde_json::Error> {
  match request.headers().get(crate::net::CAPABILITY_HEADER).map(|h| serde_json::from_slice::<Vec<String>>(h.as_bytes())).transpose() {
    Err(e) => Err(e),
    Ok(v) => Ok(v.into_iter().flatten().flat_map(|c| CAPABILITIES.iter().find(|&&f| f == &c).copied()).collect()),
  }
}

pub fn add_header(builder: http::request::Builder) -> http::request::Builder {
  builder.header(crate::net::CAPABILITY_HEADER, serde_json::to_string(&CAPABILITIES).unwrap())
}

pub fn all_supported<S: AsRef<str>>(capabilities: Vec<S>) -> Result<std::collections::BTreeSet<&'static str>, S> {
  capabilities
    .into_iter()
    .map(|cap| match CAPABILITIES.iter().find(|&&c| c == cap.as_ref()).copied() {
      Some(c) => Ok(c),
      None => Err(cap),
    })
    .collect()
}
