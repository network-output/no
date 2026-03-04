//! ICMP ping handler with per-ping statistics and aggregate summary.

use std::net::IpAddr;
use std::time::Duration;

use serde_json::json;
use surge_ping::{Client, Config, ICMP, IcmpPacket, PingIdentifier, PingSequence, SurgeError};

use crate::cli::PingArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};

/// Execute an ICMP ping sequence and print per-ping results plus a summary.
///
/// Resolves the target host, sends `count` pings (default 4) with the configured interval,
/// and emits a `message` event for each successful reply followed by a `response` event
/// with aggregate statistics.
///
/// # Errors
///
/// Returns [`NetError`] on DNS resolution failure or socket creation errors.
pub async fn run(
  args: PingArgs,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let host = crate::addr::strip_brackets(&args.host);
  let ip = resolve_host(host).await?;
  let icmp = match ip {
    IpAddr::V4(_) => ICMP::V4,
    IpAddr::V6(_) => ICMP::V6,
  };

  let config = Config::builder().kind(icmp).build();
  let client = Client::new(&config).map_err(|e| {
    NetError::new(
      ErrorCode::IoError,
      format!("failed to create ping socket: {e}"),
      Protocol::Ping,
    )
  })?;

  let ident = std::process::id() as u16;
  let mut pinger = client.pinger(ip, PingIdentifier(ident)).await;
  let ping_timeout = timeout.unwrap_or(Duration::from_secs(1));
  pinger.timeout(ping_timeout);

  let total = count.unwrap_or(4);
  let payload = vec![0u8; 56];

  let mut transmitted: usize = 0;
  let mut received: usize = 0;
  let mut min_ms = f64::MAX;
  let mut max_ms: f64 = 0.0;
  let mut sum_ms: f64 = 0.0;

  for seq in 0..total {
    if seq > 0 {
      tokio::time::sleep(args.interval).await;
    }

    transmitted += 1;

    match pinger.ping(PingSequence(seq as u16), &payload).await {
      Ok((packet, duration)) => {
        received += 1;
        let time_ms = duration.as_secs_f64() * 1000.0;
        if time_ms < min_ms {
          min_ms = time_ms;
        }
        if time_ms > max_ms {
          max_ms = time_ms;
        }
        sum_ms += time_ms;

        let (ttl, size) = extract_packet_info(&packet);

        let response = NetResponse::new(
          ResponseType::Message,
          Protocol::Ping,
          json!({
            "seq": seq,
            "host": args.host,
            "ip": ip.to_string(),
            "ttl": ttl,
            "size": size,
            "time_ms": round_ms(time_ms),
          }),
        );
        print_response(&response, mode, no_color);
      }
      Err(SurgeError::Timeout { .. }) => {}
      Err(e) => return Err(map_ping_error(e, &args.host)),
    }
  }

  let loss_pct = if transmitted > 0 {
    ((transmitted - received) as f64 / transmitted as f64) * 100.0
  } else {
    0.0
  };

  let summary_data = if received > 0 {
    let avg_ms = sum_ms / received as f64;
    json!({
      "host": args.host,
      "ip": ip.to_string(),
      "transmitted": transmitted,
      "received": received,
      "loss_pct": round_ms(loss_pct),
      "min_ms": round_ms(min_ms),
      "avg_ms": round_ms(avg_ms),
      "max_ms": round_ms(max_ms),
    })
  } else {
    json!({
      "host": args.host,
      "ip": ip.to_string(),
      "transmitted": transmitted,
      "received": received,
      "loss_pct": round_ms(loss_pct),
    })
  };

  let response = NetResponse::new(ResponseType::Response, Protocol::Ping, summary_data);

  if verbose {
    let metadata = json!({
      "identifier": ident,
      "payload_size": payload.len(),
    });
    print_response(&response.with_metadata(metadata), mode, no_color);
  } else {
    print_response(&response, mode, no_color);
  }

  Ok(())
}

async fn resolve_host(host: &str) -> Result<IpAddr, NetError> {
  if let Ok(ip) = host.parse::<IpAddr>() {
    return Ok(ip);
  }

  let addr = tokio::net::lookup_host(format!("{host}:0"))
    .await
    .map_err(|e| {
      NetError::new(
        ErrorCode::DnsResolution,
        format!("failed to resolve {host}: {e}"),
        Protocol::Ping,
      )
    })?
    .next()
    .ok_or_else(|| {
      NetError::new(
        ErrorCode::DnsResolution,
        format!("no addresses found for {host}"),
        Protocol::Ping,
      )
    })?;

  Ok(addr.ip())
}

fn extract_packet_info(packet: &IcmpPacket) -> (Option<u8>, usize) {
  match packet {
    IcmpPacket::V4(p) => (p.get_ttl(), p.get_size()),
    IcmpPacket::V6(p) => (Some(p.get_max_hop_limit()), p.get_size()),
  }
}

fn round_ms(val: f64) -> f64 {
  (val * 100.0).round() / 100.0
}

fn map_ping_error(e: SurgeError, host: &str) -> NetError {
  match e {
    SurgeError::Timeout { .. } => NetError::new(
      ErrorCode::ConnectionTimeout,
      format!("ping to {host} timed out"),
      Protocol::Ping,
    ),
    SurgeError::IOError(..) => {
      let msg = e.to_string();
      NetError::new(ErrorCode::IoError, msg, Protocol::Ping)
    }
    _ => NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Ping),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn map_ping_error_io() {
    let err = SurgeError::IOError(std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "permission denied",
    ));
    let net_err = map_ping_error(err, "localhost");
    assert!(matches!(net_err.code, ErrorCode::IoError));
  }

  #[test]
  fn map_ping_error_timeout() {
    let err = SurgeError::Timeout { seq: PingSequence(0) };
    let net_err = map_ping_error(err, "localhost");
    assert!(matches!(net_err.code, ErrorCode::ConnectionTimeout));
  }

  #[test]
  fn map_ping_error_protocol() {
    let err = SurgeError::IncorrectBufferSize;
    let net_err = map_ping_error(err, "localhost");
    assert!(matches!(net_err.code, ErrorCode::ProtocolError));
  }

  #[test]
  fn round_ms_precision() {
    assert_eq!(round_ms(12.345), 12.35);
    assert_eq!(round_ms(0.1), 0.1);
    assert_eq!(round_ms(100.0), 100.0);
  }
}
