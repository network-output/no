//! Shared address helpers for IPv4/IPv6 socket parsing.

use std::net::{IpAddr, SocketAddr};

use crate::error::{ErrorCode, NetError};
use crate::output::Protocol;

/// Parse "[::1]:9090", "127.0.0.1:9090", or ":9090" (defaults to 0.0.0.0).
/// Strips brackets from IPv6 for SocketAddr parsing.
pub fn parse_listen_addr(address: &str, protocol: Protocol) -> Result<SocketAddr, NetError> {
  let normalized = if address.starts_with(':') {
    format!("0.0.0.0{address}")
  } else if let Some(rest) = address.strip_prefix('[') {
    // Bracketed IPv6: [::1]:9090 or [::]:9090
    if let Some((ip, port_str)) = rest.split_once("]:") {
      format!("[{ip}]:{port_str}")
    } else {
      // Bare [::] without port -- invalid for listen
      return Err(NetError::new(
        ErrorCode::InvalidInput,
        format!("missing port in listen address: {address}"),
        protocol,
      ));
    }
  } else {
    address.to_string()
  };

  normalized.parse::<SocketAddr>().map_err(|e| {
    NetError::new(
      ErrorCode::InvalidInput,
      format!("invalid listen address \"{address}\": {e}"),
      protocol,
    )
  })
}

/// Returns the correct bind address for a client socket targeting `target`.
/// IPv6 target -> [::]:0, IPv4 target -> 0.0.0.0:0
pub fn client_bind_addr(target: &SocketAddr) -> SocketAddr {
  match target.ip() {
    IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0),
    IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
  }
}

/// Strip surrounding brackets from an address, useful for portless protocols
/// that accept both `::1` and `[::1]`.
pub fn strip_brackets(host: &str) -> &str {
  host.strip_prefix('[')
    .and_then(|s| s.strip_suffix(']'))
    .unwrap_or(host)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_listen_addr_bare_port() {
    let addr = parse_listen_addr(":9090", Protocol::Tcp).unwrap();
    assert_eq!(addr, "0.0.0.0:9090".parse::<SocketAddr>().unwrap());
  }

  #[test]
  fn parse_listen_addr_ipv4() {
    let addr = parse_listen_addr("127.0.0.1:8080", Protocol::Tcp).unwrap();
    assert_eq!(addr, "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
  }

  #[test]
  fn parse_listen_addr_bracketed_ipv6() {
    let addr = parse_listen_addr("[::1]:9090", Protocol::Tcp).unwrap();
    assert_eq!(addr, "[::1]:9090".parse::<SocketAddr>().unwrap());
  }

  #[test]
  fn parse_listen_addr_bracketed_ipv6_unspecified() {
    let addr = parse_listen_addr("[::]:9090", Protocol::Tcp).unwrap();
    assert_eq!(addr, "[::]:9090".parse::<SocketAddr>().unwrap());
  }

  #[test]
  fn parse_listen_addr_missing_port() {
    let err = parse_listen_addr("[::1]", Protocol::Tcp).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
  }

  #[test]
  fn parse_listen_addr_invalid() {
    let err = parse_listen_addr("not-an-addr", Protocol::Tcp).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
  }

  #[test]
  fn client_bind_addr_v4() {
    let target: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let bind = client_bind_addr(&target);
    assert!(bind.ip().is_unspecified());
    assert!(bind.is_ipv4());
    assert_eq!(bind.port(), 0);
  }

  #[test]
  fn client_bind_addr_v6() {
    let target: SocketAddr = "[::1]:8080".parse().unwrap();
    let bind = client_bind_addr(&target);
    assert!(bind.ip().is_unspecified());
    assert!(bind.is_ipv6());
    assert_eq!(bind.port(), 0);
  }

  #[test]
  fn strip_brackets_with_brackets() {
    assert_eq!(strip_brackets("[::1]"), "::1");
  }

  #[test]
  fn strip_brackets_without_brackets() {
    assert_eq!(strip_brackets("::1"), "::1");
    assert_eq!(strip_brackets("127.0.0.1"), "127.0.0.1");
  }

  #[test]
  fn strip_brackets_partial() {
    // Only strip if both brackets present
    assert_eq!(strip_brackets("[::1"), "[::1");
    assert_eq!(strip_brackets("::1]"), "::1]");
  }
}
