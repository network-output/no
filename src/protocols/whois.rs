//! WHOIS lookup handler using raw TCP connections to port 43.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::cli::WhoisArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};

/// Execute a WHOIS lookup and print the result as structured output.
///
/// Connects to the appropriate WHOIS server (auto-detected or user-specified),
/// sends the query, reads the full response, and emits a single `response` event.
///
/// # Errors
///
/// Returns [`NetError`] on connection failure, timeout, or I/O error.
pub async fn run(
  args: WhoisArgs,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  verbose: bool,
) -> Result<(), NetError> {
  let query = crate::addr::strip_brackets(&args.query).to_string();
  let server = args.server.clone().unwrap_or_else(|| detect_whois_server(&query));

  let addr = format!("{server}:43");
  let start = Instant::now();

  let mut stream = if let Some(dur) = timeout {
    tokio::time::timeout(dur, TcpStream::connect(&addr))
      .await
      .map_err(|_| {
        NetError::new(
          ErrorCode::ConnectionTimeout,
          format!("connection to {addr} timed out"),
          Protocol::Whois,
        )
      })?
      .map_err(|e| map_whois_error(e, &addr))?
  } else {
    TcpStream::connect(&addr).await.map_err(|e| map_whois_error(e, &addr))?
  };

  let query_bytes = format!("{query}\r\n");
  stream
    .write_all(query_bytes.as_bytes())
    .await
    .map_err(|e| map_whois_error(e, &addr))?;

  let mut buf = Vec::new();
  if let Some(dur) = timeout {
    tokio::time::timeout(dur, stream.read_to_end(&mut buf))
      .await
      .map_err(|_| {
        NetError::new(
          ErrorCode::ConnectionTimeout,
          format!("read from {addr} timed out"),
          Protocol::Whois,
        )
      })?
      .map_err(|e| map_whois_error(e, &addr))?;
  } else {
    stream
      .read_to_end(&mut buf)
      .await
      .map_err(|e| map_whois_error(e, &addr))?;
  }

  let elapsed = start.elapsed();
  let response_text = String::from_utf8_lossy(&buf).to_string();

  let response = NetResponse::new(
    ResponseType::Response,
    Protocol::Whois,
    json!({
      "query": query,
      "server": server,
      "response": response_text,
    }),
  );

  if verbose {
    let metadata = json!({
      "server": addr,
      "time_ms": elapsed.as_millis() as u64,
    });
    print_response(&response.with_metadata(metadata), mode, no_color);
  } else {
    print_response(&response, mode, no_color);
  }

  Ok(())
}

fn detect_whois_server(query: &str) -> String {
  if query.parse::<IpAddr>().is_ok() {
    return "whois.arin.net".to_owned();
  }

  let tld = query.rsplit('.').next().unwrap_or("");
  match tld.to_lowercase().as_str() {
    "com" | "net" => "whois.verisign-grs.com",
    "org" => "whois.pir.org",
    "io" => "whois.nic.io",
    "dev" | "app" => "whois.nic.google",
    "me" => "whois.nic.me",
    "co" => "whois.nic.co",
    "us" => "whois.nic.us",
    "uk" => "whois.nic.uk",
    "de" => "whois.denic.de",
    "fr" => "whois.nic.fr",
    "au" => "whois.auda.org.au",
    "br" => "whois.registro.br",
    _ => "whois.iana.org",
  }
  .to_owned()
}

fn map_whois_error(e: std::io::Error, addr: &str) -> NetError {
  match e.kind() {
    std::io::ErrorKind::ConnectionRefused => NetError::new(
      ErrorCode::ConnectionRefused,
      format!("connection to {addr} refused"),
      Protocol::Whois,
    ),
    std::io::ErrorKind::TimedOut => NetError::new(
      ErrorCode::ConnectionTimeout,
      format!("connection to {addr} timed out"),
      Protocol::Whois,
    ),
    _ => NetError::new(ErrorCode::IoError, format!("{addr}: {e}"), Protocol::Whois),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detect_server_com() {
    assert_eq!(detect_whois_server("example.com"), "whois.verisign-grs.com");
  }

  #[test]
  fn detect_server_net() {
    assert_eq!(detect_whois_server("example.net"), "whois.verisign-grs.com");
  }

  #[test]
  fn detect_server_org() {
    assert_eq!(detect_whois_server("example.org"), "whois.pir.org");
  }

  #[test]
  fn detect_server_io() {
    assert_eq!(detect_whois_server("example.io"), "whois.nic.io");
  }

  #[test]
  fn detect_server_dev() {
    assert_eq!(detect_whois_server("example.dev"), "whois.nic.google");
  }

  #[test]
  fn detect_server_app() {
    assert_eq!(detect_whois_server("example.app"), "whois.nic.google");
  }

  #[test]
  fn detect_server_ip() {
    assert_eq!(detect_whois_server("8.8.8.8"), "whois.arin.net");
  }

  #[test]
  fn detect_server_ipv6() {
    assert_eq!(detect_whois_server("::1"), "whois.arin.net");
  }

  #[test]
  fn detect_server_unknown_tld() {
    assert_eq!(detect_whois_server("example.xyz"), "whois.iana.org");
  }

  #[test]
  fn detect_server_br() {
    assert_eq!(detect_whois_server("example.br"), "whois.registro.br");
  }

  #[test]
  fn detect_server_uk() {
    assert_eq!(detect_whois_server("example.uk"), "whois.nic.uk");
  }

  #[test]
  fn map_whois_error_refused() {
    let err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let net_err = map_whois_error(err, "whois.example.com:43");
    assert!(matches!(net_err.code, ErrorCode::ConnectionRefused));
  }

  #[test]
  fn map_whois_error_timeout() {
    let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
    let net_err = map_whois_error(err, "whois.example.com:43");
    assert!(matches!(net_err.code, ErrorCode::ConnectionTimeout));
  }

  #[test]
  fn map_whois_error_other() {
    let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe");
    let net_err = map_whois_error(err, "whois.example.com:43");
    assert!(matches!(net_err.code, ErrorCode::IoError));
  }
}
