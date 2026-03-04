//! UDP datagram handler for sending and listening.

use crate::cli::UdpAction;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use serde_json::json;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Dispatch UDP send or listen operations.
///
/// In send mode, transmits a datagram to the target address and optionally waits for a response.
/// In listen mode, binds to a local address and streams incoming datagrams.
///
/// # Errors
///
/// Returns [`NetError`] on bind failure, send errors, or I/O errors during read/write.
pub async fn run(
  action: UdpAction,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  match action {
    UdpAction::Send(args) => {
      let message = if args.stdin {
        let mut input = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::stdin(), &mut input)
          .await
          .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Udp))?;
        Some(input)
      } else {
        args.message
      };
      send(
        &args.address,
        message.as_deref(),
        args.wait,
        mode,
        no_color,
        timeout,
        count,
        verbose,
      )
      .await
    }
    UdpAction::Listen(args) => listen(&args.address, mode, no_color, timeout, count, verbose).await,
  }
}

#[allow(clippy::too_many_arguments)]
async fn send(
  address: &str,
  message: Option<&str>,
  wait: Option<Option<Duration>>,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let target: std::net::SocketAddr = address.parse().map_err(|e| {
    NetError::new(
      ErrorCode::InvalidInput,
      format!("invalid target address \"{address}\": {e}"),
      Protocol::Udp,
    )
  })?;
  let bind_addr = crate::addr::client_bind_addr(&target);
  let socket = UdpSocket::bind(bind_addr).await.map_err(|e| {
    NetError::new(
      ErrorCode::IoError,
      format!("cannot bind UDP socket: {e}"),
      Protocol::Udp,
    )
  })?;

  if let Some(msg) = message {
    let bytes_sent = socket.send_to(msg.as_bytes(), target).await.map_err(map_udp_error)?;

    let sent = NetResponse::new(
      ResponseType::Connection,
      Protocol::Udp,
      json!({ "status": "sent", "address": address, "bytes": bytes_sent }),
    );
    print_response(&sent, mode, no_color);
  }

  // Handle --wait flag
  if let Some(wait_duration) = wait {
    // Duration::ZERO is the sentinel for bare --wait (no explicit duration)
    let effective_timeout = match wait_duration {
      Some(dur) if !dur.is_zero() => Some(dur),
      _ => timeout,
    };
    let mut buf = vec![0u8; 65535];
    let mut message_count: usize = 0;

    loop {
      let result = if let Some(dur) = effective_timeout {
        match tokio::time::timeout(dur, socket.recv_from(&mut buf)).await {
          Ok(result) => result.map_err(map_udp_error)?,
          Err(_) => break, // wait timeout expired, clean exit
        }
      } else {
        socket.recv_from(&mut buf).await.map_err(map_udp_error)?
      };

      let (n, peer_addr) = result;
      message_count += 1;
      let text = String::from_utf8_lossy(&buf[..n]);
      let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| json!(text.to_string()));
      let response = NetResponse::new(
        ResponseType::Message,
        Protocol::Udp,
        json!({ "peer": peer_addr.to_string(), "data": data }),
      );
      if verbose {
        print_response(&response.with_metadata(json!({ "bytes": n })), mode, no_color);
      } else {
        print_response(&response, mode, no_color);
      }

      if count.is_some_and(|c| message_count >= c) {
        break;
      }
    }
  }

  Ok(())
}

async fn listen(
  address: &str,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let addr = crate::addr::parse_listen_addr(address, Protocol::Udp)?;

  let socket = UdpSocket::bind(addr)
    .await
    .map_err(|e| NetError::new(ErrorCode::IoError, format!("cannot bind to {addr}: {e}"), Protocol::Udp))?;

  let local_addr = socket
    .local_addr()
    .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Udp))?;

  let listening = NetResponse::new(
    ResponseType::Connection,
    Protocol::Udp,
    json!({ "status": "listening", "address": local_addr.to_string() }),
  );
  print_response(&listening, mode, no_color);

  let mut buf = vec![0u8; 65535];
  let mut message_count: usize = 0;

  loop {
    let (n, peer_addr) = if let Some(dur) = timeout {
      match tokio::time::timeout(dur, socket.recv_from(&mut buf)).await {
        Ok(result) => result.map_err(map_udp_error)?,
        Err(_) => {
          return Err(NetError::new(
            ErrorCode::ConnectionTimeout,
            "UDP listen timed out",
            Protocol::Udp,
          ));
        }
      }
    } else {
      socket.recv_from(&mut buf).await.map_err(map_udp_error)?
    };

    message_count += 1;
    let text = String::from_utf8_lossy(&buf[..n]);
    let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| json!(text.to_string()));
    let response = NetResponse::new(
      ResponseType::Message,
      Protocol::Udp,
      json!({ "peer": peer_addr.to_string(), "data": data }),
    );
    if verbose {
      print_response(&response.with_metadata(json!({ "bytes": n })), mode, no_color);
    } else {
      print_response(&response, mode, no_color);
    }

    if count.is_some_and(|c| message_count >= c) {
      break;
    }
  }

  Ok(())
}

fn map_udp_error(e: std::io::Error) -> NetError {
  match e.kind() {
    std::io::ErrorKind::ConnectionRefused => NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Udp),
    std::io::ErrorKind::TimedOut => NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Udp),
    _ => NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Udp),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::ErrorCode;

  #[test]
  fn map_udp_error_connection_refused() {
    let err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let net_err = map_udp_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionRefused);
  }

  #[test]
  fn map_udp_error_timed_out() {
    let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    let net_err = map_udp_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionTimeout);
  }

  #[test]
  fn map_udp_error_other() {
    let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
    let net_err = map_udp_error(err);
    assert_eq!(net_err.code, ErrorCode::IoError);
  }
}
