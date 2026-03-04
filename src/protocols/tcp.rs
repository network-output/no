//! Raw TCP client and server handler for plain-text socket communication.

use crate::cli::TcpAction;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Dispatch TCP connect or listen operations.
///
/// In connect mode, establishes a TCP connection, optionally sends a message or stdin content, and
/// reads the response. In listen mode, binds to a local address and streams incoming connections
/// and their data.
///
/// # Errors
///
/// Returns [`NetError`] on connection refused, timeout, bind failure, or I/O errors during
/// read/write.
pub async fn run(
  action: TcpAction,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  match action {
    TcpAction::Connect(args) => {
      connect(
        &args.address,
        args.message.as_deref(),
        args.stdin,
        mode,
        no_color,
        timeout,
        count,
        verbose,
      )
      .await
    }
    TcpAction::Listen(args) => listen(&args.address, mode, no_color, count, verbose).await,
  }
}

#[allow(clippy::too_many_arguments)]
async fn connect(
  address: &str,
  message: Option<&str>,
  read_stdin: bool,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let connect_fut = TcpStream::connect(address);
  let mut stream = if let Some(dur) = timeout {
    tokio::time::timeout(dur, connect_fut)
      .await
      .map_err(|_| NetError::new(ErrorCode::ConnectionTimeout, "TCP connection timed out", Protocol::Tcp))?
      .map_err(map_tcp_error)?
  } else {
    connect_fut.await.map_err(map_tcp_error)?
  };

  let connected = NetResponse::new(
    ResponseType::Connection,
    Protocol::Tcp,
    json!({ "status": "connected", "address": address }),
  );
  if verbose {
    print_response(&connected.with_metadata(json!({ "address": address })), mode, no_color);
  } else {
    print_response(&connected, mode, no_color);
  }

  // Send message if provided
  if read_stdin {
    let mut input = String::new();
    tokio::io::stdin()
      .read_to_string(&mut input)
      .await
      .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;
    stream
      .write_all(input.as_bytes())
      .await
      .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;
  } else if let Some(msg) = message {
    stream
      .write_all(msg.as_bytes())
      .await
      .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;
  }

  // Read responses
  let mut buf = vec![0u8; 8192];
  let mut message_count: usize = 0;
  loop {
    let n = if let Some(dur) = timeout {
      tokio::time::timeout(dur, stream.read(&mut buf))
        .await
        .map_err(|_| NetError::new(ErrorCode::ConnectionTimeout, "TCP read timed out", Protocol::Tcp))?
        .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?
    } else {
      stream
        .read(&mut buf)
        .await
        .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?
    };

    if n == 0 {
      let closed = NetResponse::new(ResponseType::Connection, Protocol::Tcp, json!({ "status": "closed" }));
      print_response(&closed, mode, no_color);
      break;
    }

    message_count += 1;
    let text = String::from_utf8_lossy(&buf[..n]);
    let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| json!(text.to_string()));
    let response = NetResponse::new(ResponseType::Message, Protocol::Tcp, data);
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

async fn listen(
  address: &str,
  mode: OutputMode,
  no_color: bool,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let addr = crate::addr::parse_listen_addr(address, Protocol::Tcp)?;

  let listener = TcpListener::bind(addr)
    .await
    .map_err(|e| NetError::new(ErrorCode::IoError, format!("cannot bind to {addr}: {e}"), Protocol::Tcp))?;

  let local_addr = listener
    .local_addr()
    .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;

  let listening = NetResponse::new(
    ResponseType::Connection,
    Protocol::Tcp,
    json!({ "status": "listening", "address": local_addr.to_string() }),
  );
  print_response(&listening, mode, no_color);

  let mut total_messages: usize = 0;

  loop {
    let (mut socket, peer_addr) = listener
      .accept()
      .await
      .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;

    let peer = peer_addr.to_string();
    let connected = NetResponse::new(
      ResponseType::Connection,
      Protocol::Tcp,
      json!({ "status": "accepted", "peer": peer }),
    );
    print_response(&connected, mode, no_color);

    let mut buf = vec![0u8; 8192];
    loop {
      let n = socket
        .read(&mut buf)
        .await
        .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp))?;

      if n == 0 {
        let closed = NetResponse::new(
          ResponseType::Connection,
          Protocol::Tcp,
          json!({ "status": "disconnected", "peer": peer }),
        );
        print_response(&closed, mode, no_color);
        break;
      }

      total_messages += 1;
      let text = String::from_utf8_lossy(&buf[..n]);
      let data: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| json!(text.to_string()));
      let response = NetResponse::new(
        ResponseType::Message,
        Protocol::Tcp,
        json!({ "peer": peer, "data": data }),
      );
      if verbose {
        print_response(&response.with_metadata(json!({ "bytes": n })), mode, no_color);
      } else {
        print_response(&response, mode, no_color);
      }

      if count.is_some_and(|c| total_messages >= c) {
        return Ok(());
      }
    }
  }
}

fn map_tcp_error(e: std::io::Error) -> NetError {
  match e.kind() {
    std::io::ErrorKind::ConnectionRefused => NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Tcp),
    std::io::ErrorKind::TimedOut => NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Tcp),
    _ => NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Tcp),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::ErrorCode;

  #[test]
  fn map_tcp_error_connection_refused() {
    let err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let net_err = map_tcp_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionRefused);
  }

  #[test]
  fn map_tcp_error_timed_out() {
    let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    let net_err = map_tcp_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionTimeout);
  }

  #[test]
  fn map_tcp_error_other() {
    let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
    let net_err = map_tcp_error(err);
    assert_eq!(net_err.code, ErrorCode::IoError);
  }
}
