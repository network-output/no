//! URL normalization and scheme inference for protocol handlers.
//!
//! When a user omits the scheme prefix from a URL, this module infers the appropriate
//! scheme based on whether the target host is local/private or remote. Protocol handlers
//! call [`normalize_url`] before establishing connections to guarantee a fully qualified URL.

/// Scheme family used to select between encrypted and unencrypted variants.
#[derive(Debug, Clone, Copy)]
pub enum UrlScheme {
  /// HTTP/HTTPS scheme selection.
  Http,
  /// WebSocket WS/WSS scheme selection.
  Ws,
}

/// Infer and prepend the appropriate scheme based on the target host.
///
/// Local and private addresses (localhost, 127.0.0.1, RFC 1918 ranges) default to the
/// unencrypted variant (http / ws), while remote addresses default to the encrypted
/// variant (https / wss). Existing scheme prefixes are preserved unchanged.
pub fn normalize_url(url: &str, scheme: UrlScheme) -> String {
  let has_scheme =
    url.starts_with("http://") || url.starts_with("https://") || url.starts_with("ws://") || url.starts_with("wss://");

  if has_scheme {
    return url.to_string();
  }

  let host = extract_host(url);
  let prefix = if is_local_host(&host) {
    match scheme {
      UrlScheme::Http => "http://",
      UrlScheme::Ws => "ws://",
    }
  } else {
    match scheme {
      UrlScheme::Http => "https://",
      UrlScheme::Ws => "wss://",
    }
  };

  format!("{prefix}{url}")
}

fn extract_host(url: &str) -> String {
  let without_path = url.split('/').next().unwrap_or(url);

  // Handle bracketed IPv6 (e.g. "[::1]:3000")
  if let Some(rest) = without_path.strip_prefix('[') {
    return rest.split(']').next().unwrap_or(rest).to_lowercase();
  }

  // Handle bare IPv6 (multiple colons, no brackets): e.g. "::1", "fe80::1"
  let colon_count = without_path.chars().filter(|&c| c == ':').count();
  if colon_count >= 2 {
    return without_path.to_lowercase();
  }

  let without_port = without_path.split(':').next().unwrap_or(without_path);
  without_port.to_lowercase()
}

fn is_local_host(host: &str) -> bool {
  matches!(host, "localhost" | "0.0.0.0") || is_private_ip(host)
}

fn is_private_ip(host: &str) -> bool {
  use std::net::IpAddr;

  match host.parse::<IpAddr>() {
    Ok(IpAddr::V4(v4)) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
    Ok(IpAddr::V6(v6)) => {
      let seg = v6.segments();
      v6.is_loopback()
        || (seg[0] & 0xfe00) == 0xfc00 // ULA fc00::/7
        || (seg[0] & 0xffc0) == 0xfe80 // Link-local fe80::/10
    }
    Err(_) => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn preserves_existing_http_scheme() {
    assert_eq!(
      normalize_url("http://example.com", UrlScheme::Http),
      "http://example.com"
    );
  }

  #[test]
  fn preserves_existing_https_scheme() {
    assert_eq!(
      normalize_url("https://example.com", UrlScheme::Http),
      "https://example.com"
    );
  }

  #[test]
  fn preserves_existing_ws_scheme() {
    assert_eq!(
      normalize_url("ws://localhost:8080", UrlScheme::Ws),
      "ws://localhost:8080"
    );
  }

  #[test]
  fn preserves_existing_wss_scheme() {
    assert_eq!(
      normalize_url("wss://example.com/ws", UrlScheme::Ws),
      "wss://example.com/ws"
    );
  }

  #[test]
  fn localhost_gets_http() {
    assert_eq!(
      normalize_url("localhost:3000/api", UrlScheme::Http),
      "http://localhost:3000/api"
    );
  }

  #[test]
  fn localhost_gets_ws() {
    assert_eq!(
      normalize_url("localhost:8080/ws", UrlScheme::Ws),
      "ws://localhost:8080/ws"
    );
  }

  #[test]
  fn loopback_gets_http() {
    assert_eq!(
      normalize_url("127.0.0.1:3000", UrlScheme::Http),
      "http://127.0.0.1:3000"
    );
  }

  #[test]
  fn ipv6_loopback_gets_http() {
    assert_eq!(normalize_url("[::1]:3000", UrlScheme::Http), "http://[::1]:3000");
  }

  #[test]
  fn bare_ipv6_loopback_gets_http() {
    assert_eq!(normalize_url("::1", UrlScheme::Http), "http://::1");
  }

  #[test]
  fn zero_addr_gets_http() {
    assert_eq!(normalize_url("0.0.0.0:3000", UrlScheme::Http), "http://0.0.0.0:3000");
  }

  #[test]
  fn private_10_gets_http() {
    assert_eq!(normalize_url("10.0.0.1:3000", UrlScheme::Http), "http://10.0.0.1:3000");
  }

  #[test]
  fn private_172_gets_http() {
    assert_eq!(
      normalize_url("172.16.0.1:3000", UrlScheme::Http),
      "http://172.16.0.1:3000"
    );
  }

  #[test]
  fn private_172_31_gets_http() {
    assert_eq!(
      normalize_url("172.31.255.1:3000", UrlScheme::Http),
      "http://172.31.255.1:3000"
    );
  }

  #[test]
  fn private_172_32_gets_https() {
    assert_eq!(
      normalize_url("172.32.0.1:3000", UrlScheme::Http),
      "https://172.32.0.1:3000"
    );
  }

  #[test]
  fn private_192_168_gets_http() {
    assert_eq!(
      normalize_url("192.168.1.1:3000", UrlScheme::Http),
      "http://192.168.1.1:3000"
    );
  }

  #[test]
  fn remote_host_gets_https() {
    assert_eq!(
      normalize_url("example.com/api", UrlScheme::Http),
      "https://example.com/api"
    );
  }

  #[test]
  fn remote_host_gets_wss() {
    assert_eq!(
      normalize_url("api.example.com/ws", UrlScheme::Ws),
      "wss://api.example.com/ws"
    );
  }

  #[test]
  fn remote_host_with_port_gets_https() {
    assert_eq!(
      normalize_url("example.com:8443/api", UrlScheme::Http),
      "https://example.com:8443/api"
    );
  }

  #[test]
  fn is_local_host_known_locals() {
    assert!(is_local_host("localhost"));
    assert!(is_local_host("127.0.0.1"));
    assert!(is_local_host("::1"));
    assert!(is_local_host("0.0.0.0"));
  }

  #[test]
  fn is_local_host_private_ranges() {
    assert!(is_local_host("10.0.0.1"));
    assert!(is_local_host("172.16.0.1"));
    assert!(is_local_host("192.168.0.1"));
  }

  #[test]
  fn is_local_host_public() {
    assert!(!is_local_host("example.com"));
    assert!(!is_local_host("8.8.8.8"));
    assert!(!is_local_host("172.32.0.1"));
  }

  #[test]
  fn ipv6_ula_is_local() {
    assert!(is_local_host("fd12:3456:789a::1"));
  }

  #[test]
  fn ipv6_link_local_is_local() {
    assert!(is_local_host("fe80::1"));
  }

  #[test]
  fn ipv6_public_is_not_local() {
    assert!(!is_local_host("2001:db8::1"));
  }

  #[test]
  fn extract_host_bare_ipv6() {
    assert_eq!(extract_host("fe80::1/path"), "fe80::1");
  }

  #[test]
  fn extract_host_bracketed_ipv6() {
    assert_eq!(extract_host("[::1]:3000/path"), "::1");
  }

  #[test]
  fn ipv6_ula_gets_http() {
    assert_eq!(
      normalize_url("fd00::1", UrlScheme::Http),
      "http://fd00::1"
    );
  }

  #[test]
  fn ipv6_link_local_gets_ws() {
    assert_eq!(
      normalize_url("fe80::1", UrlScheme::Ws),
      "ws://fe80::1"
    );
  }
}
