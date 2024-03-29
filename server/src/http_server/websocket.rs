use hyper::http;

pub fn convert_key(input: &[u8]) -> String {
  use base64::Engine;
  use sha3::Digest;
  const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
  let mut digest = sha1::Sha1::new();
  digest.update(input);
  digest.update(WS_GUID);
  base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest.finalize().as_slice())
}
pub fn connection_has(value: &http::header::HeaderValue, needle: &str) -> bool {
  if let Ok(v) = value.to_str() {
    v.split(',').any(|s| s.trim().eq_ignore_ascii_case(needle))
  } else {
    false
  }
}
