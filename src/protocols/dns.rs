//! DNS lookup handler supporting forward and reverse queries with customizable DNS servers.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use hickory_resolver::Resolver;
use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::proto::rr::{RData, RecordType};
use hickory_resolver::proto::xfer::Protocol as DnsTransport;
use hickory_resolver::{ResolveError, ResolveErrorKind};
use serde_json::json;

use crate::cli::DnsArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};

/// Execute a DNS lookup and print the results as structured output.
///
/// Supports forward lookups (A, AAAA, MX, TXT, CNAME, NS, SOA, SRV, PTR) and auto-detects
/// reverse lookups when the input is an IP address. A custom DNS server can be specified
/// with `--server`.
///
/// # Errors
///
/// Returns [`NetError`] on invalid record type, DNS resolution failure, timeout, or I/O error.
pub async fn run(
  args: DnsArgs,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  verbose: bool,
) -> Result<(), NetError> {
  let name = crate::addr::strip_brackets(&args.name).to_string();
  let server = args.server.as_deref().map(crate::addr::strip_brackets).map(String::from);

  let is_reverse = name.parse::<IpAddr>().ok();
  let record_type = if is_reverse.is_some() {
    RecordType::PTR
  } else {
    parse_record_type(&args.record_type)?
  };
  let record_type_str = format!("{record_type}");

  let resolver = build_resolver(&server, timeout)?;
  let start = Instant::now();

  let records: Vec<serde_json::Value> = if let Some(ip) = is_reverse {
    let lookup = execute_with_timeout(resolver.reverse_lookup(ip), timeout).await?;
    lookup
      .as_lookup()
      .record_iter()
      .map(|record| format_record(record.data(), record.ttl()))
      .collect()
  } else {
    let lookup = execute_with_timeout(resolver.lookup(&name, record_type), timeout).await?;
    lookup
      .record_iter()
      .map(|record| format_record(record.data(), record.ttl()))
      .collect()
  };

  let elapsed = start.elapsed();

  let response = NetResponse::new(
    ResponseType::Response,
    Protocol::Dns,
    json!({
      "name": name,
      "type": record_type_str,
      "records": records,
    }),
  );

  if verbose {
    let server_str = server.as_deref().unwrap_or("system");
    let metadata = json!({
      "server": server_str,
      "time_ms": elapsed.as_millis() as u64,
    });
    print_response(&response.with_metadata(metadata), mode, no_color);
  } else {
    print_response(&response, mode, no_color);
  }

  Ok(())
}

fn parse_record_type(s: &str) -> Result<RecordType, NetError> {
  match s.to_uppercase().as_str() {
    "A" => Ok(RecordType::A),
    "AAAA" => Ok(RecordType::AAAA),
    "MX" => Ok(RecordType::MX),
    "TXT" => Ok(RecordType::TXT),
    "CNAME" => Ok(RecordType::CNAME),
    "NS" => Ok(RecordType::NS),
    "SOA" => Ok(RecordType::SOA),
    "SRV" => Ok(RecordType::SRV),
    "PTR" => Ok(RecordType::PTR),
    _ => Err(NetError::new(
      ErrorCode::InvalidInput,
      format!("unsupported record type: {s}"),
      Protocol::Dns,
    )),
  }
}

fn build_resolver(
  server: &Option<String>,
  timeout: Option<Duration>,
) -> Result<Resolver<hickory_resolver::name_server::TokioConnectionProvider>, NetError> {
  if let Some(server_str) = server {
    let addr = parse_server_addr(server_str)?;
    let mut config = ResolverConfig::new();
    config.add_name_server(NameServerConfig::new(addr, DnsTransport::Udp));
    let mut opts = ResolverOpts::default();
    if let Some(dur) = timeout {
      opts.timeout = dur;
    }
    let mut builder = Resolver::builder_with_config(
      config,
      hickory_resolver::name_server::TokioConnectionProvider::default(),
    );
    *builder.options_mut() = opts;
    Ok(builder.build())
  } else {
    let mut builder = Resolver::builder_tokio().map_err(|e| {
      NetError::new(
        ErrorCode::IoError,
        format!("failed to create DNS resolver: {e}"),
        Protocol::Dns,
      )
    })?;
    if let Some(dur) = timeout {
      builder.options_mut().timeout = dur;
    }
    Ok(builder.build())
  }
}

fn parse_server_addr(server: &str) -> Result<SocketAddr, NetError> {
  if let Ok(addr) = server.parse::<SocketAddr>() {
    return Ok(addr);
  }
  if let Ok(ip) = server.parse::<IpAddr>() {
    return Ok(SocketAddr::new(ip, 53));
  }
  Err(NetError::new(
    ErrorCode::InvalidInput,
    format!("invalid DNS server address: {server}"),
    Protocol::Dns,
  ))
}

async fn execute_with_timeout<F, T>(future: F, timeout: Option<Duration>) -> Result<T, NetError>
where
  F: std::future::Future<Output = Result<T, ResolveError>>,
{
  if let Some(dur) = timeout {
    tokio::time::timeout(dur, future)
      .await
      .map_err(|_| NetError::new(ErrorCode::ConnectionTimeout, "DNS query timed out", Protocol::Dns))?
      .map_err(map_dns_error)
  } else {
    future.await.map_err(map_dns_error)
  }
}

fn format_record(rdata: &RData, ttl: u32) -> serde_json::Value {
  match rdata {
    RData::A(a) => json!({ "value": a.to_string(), "ttl": ttl }),
    RData::AAAA(aaaa) => json!({ "value": aaaa.to_string(), "ttl": ttl }),
    RData::MX(mx) => json!({
      "value": mx.exchange().to_string(),
      "priority": mx.preference(),
      "ttl": ttl,
    }),
    RData::TXT(txt) => json!({ "value": txt.to_string(), "ttl": ttl }),
    RData::CNAME(cname) => json!({ "value": cname.0.to_string(), "ttl": ttl }),
    RData::NS(ns) => json!({ "value": ns.0.to_string(), "ttl": ttl }),
    RData::SOA(soa) => json!({
      "value": soa.mname().to_string(),
      "mname": soa.mname().to_string(),
      "rname": soa.rname().to_string(),
      "serial": soa.serial(),
      "refresh": soa.refresh(),
      "retry": soa.retry(),
      "expire": soa.expire(),
      "minimum": soa.minimum(),
      "ttl": ttl,
    }),
    RData::SRV(srv) => json!({
      "value": srv.target().to_string(),
      "priority": srv.priority(),
      "weight": srv.weight(),
      "port": srv.port(),
      "ttl": ttl,
    }),
    RData::PTR(ptr) => json!({ "value": ptr.0.to_string(), "ttl": ttl }),
    _ => json!({ "value": rdata.to_string(), "ttl": ttl }),
  }
}

fn map_dns_error(e: ResolveError) -> NetError {
  if e.is_no_records_found() || e.is_nx_domain() {
    return NetError::new(ErrorCode::DnsResolution, e.to_string(), Protocol::Dns);
  }

  match e.kind() {
    ResolveErrorKind::Proto(proto) => {
      let msg = e.to_string();
      if msg.contains("timed out") || msg.contains("Timeout") {
        NetError::new(ErrorCode::ConnectionTimeout, msg, Protocol::Dns)
      } else if proto.is_io() {
        NetError::new(ErrorCode::IoError, msg, Protocol::Dns)
      } else {
        NetError::new(ErrorCode::DnsResolution, msg, Protocol::Dns)
      }
    }
    _ => NetError::new(ErrorCode::DnsResolution, e.to_string(), Protocol::Dns),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_record_type_valid() {
    assert_eq!(parse_record_type("A").unwrap(), RecordType::A);
    assert_eq!(parse_record_type("aaaa").unwrap(), RecordType::AAAA);
    assert_eq!(parse_record_type("Mx").unwrap(), RecordType::MX);
    assert_eq!(parse_record_type("txt").unwrap(), RecordType::TXT);
    assert_eq!(parse_record_type("CNAME").unwrap(), RecordType::CNAME);
    assert_eq!(parse_record_type("ns").unwrap(), RecordType::NS);
    assert_eq!(parse_record_type("SOA").unwrap(), RecordType::SOA);
    assert_eq!(parse_record_type("srv").unwrap(), RecordType::SRV);
    assert_eq!(parse_record_type("PTR").unwrap(), RecordType::PTR);
  }

  #[test]
  fn parse_record_type_invalid() {
    let err = parse_record_type("INVALID").unwrap_err();
    assert!(matches!(err.code, ErrorCode::InvalidInput));
    assert!(err.message.contains("INVALID"));
  }

  #[test]
  fn parse_server_addr_ip_only() {
    let addr = parse_server_addr("8.8.8.8").unwrap();
    assert_eq!(
      addr,
      SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, 8)), 53)
    );
  }

  #[test]
  fn parse_server_addr_ip_port() {
    let addr = parse_server_addr("1.1.1.1:5353").unwrap();
    assert_eq!(
      addr,
      SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(1, 1, 1, 1)), 5353)
    );
  }

  #[test]
  fn parse_server_addr_invalid() {
    let err = parse_server_addr("not-an-ip").unwrap_err();
    assert!(matches!(err.code, ErrorCode::InvalidInput));
  }

  #[test]
  fn map_dns_error_generic() {
    let err = ResolveError::from(ResolveErrorKind::Msg("test error".into()));
    let net_err = map_dns_error(err);
    assert!(matches!(net_err.code, ErrorCode::DnsResolution));
  }
}
