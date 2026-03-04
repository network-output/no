//! WebSocket client handler for listening to incoming frames and sending messages.

use crate::cli::WsAction;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use crate::url::{UrlScheme, normalize_url};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Dispatch WebSocket listen or send operations.
///
/// Connects to a WebSocket server with automatic URL normalization and optional TLS. In listen
/// mode, streams incoming frames as individual messages. In send mode, transmits a single text
/// frame and prints the server response.
///
/// # Errors
///
/// Returns [`NetError`] on connection failure, WebSocket handshake errors, or message framing
/// errors.
pub async fn run(
  action: WsAction,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  match action {
    WsAction::Listen(args) => {
      let url = normalize_url(&args.url, UrlScheme::Ws);
      listen(&url, mode, no_color, timeout, count, verbose).await
    }
    WsAction::Send(args) => {
      let url = normalize_url(&args.url, UrlScheme::Ws);
      send(&url, &args.message, mode, no_color, timeout).await
    }
  }
}

async fn listen(
  url: &str,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let connect_fut = connect_async(url);
  let (ws_stream, _) = if let Some(dur) = timeout {
    tokio::time::timeout(dur, connect_fut)
      .await
      .map_err(|_| {
        NetError::new(
          ErrorCode::ConnectionTimeout,
          "WebSocket connection timed out",
          Protocol::Ws,
        )
      })?
      .map_err(map_ws_error)?
  } else {
    connect_fut.await.map_err(map_ws_error)?
  };

  let connected = NetResponse::new(
    ResponseType::Connection,
    Protocol::Ws,
    json!({ "status": "connected", "url": url }),
  );
  if verbose {
    print_response(&connected.with_metadata(json!({ "url": url })), mode, no_color);
  } else {
    print_response(&connected, mode, no_color);
  }

  let (_, mut read) = ws_stream.split();
  let mut message_count: usize = 0;

  while let Some(msg) = read.next().await {
    let msg = msg.map_err(map_ws_error)?;
    match msg {
      Message::Text(text) => {
        message_count += 1;
        let text_str: &str = &text;
        let data: serde_json::Value = serde_json::from_str(text_str).unwrap_or_else(|_| json!(text_str));
        let response = NetResponse::new(ResponseType::Message, Protocol::Ws, data);
        if verbose {
          print_response(
            &response.with_metadata(json!({ "message_number": message_count })),
            mode,
            no_color,
          );
        } else {
          print_response(&response, mode, no_color);
        }
        if count.is_some_and(|n| message_count >= n) {
          break;
        }
      }
      Message::Binary(data) => {
        message_count += 1;
        let response = NetResponse::new(
          ResponseType::Message,
          Protocol::Ws,
          json!({ "binary": true, "length": data.len() }),
        );
        if verbose {
          print_response(
            &response.with_metadata(json!({ "message_number": message_count })),
            mode,
            no_color,
          );
        } else {
          print_response(&response, mode, no_color);
        }
        if count.is_some_and(|n| message_count >= n) {
          break;
        }
      }
      Message::Close(frame) => {
        let response = NetResponse::new(
          ResponseType::Connection,
          Protocol::Ws,
          json!({
            "status": "closed",
            "reason": frame.map(|f| f.reason.to_string()).unwrap_or_default(),
          }),
        );
        print_response(&response, mode, no_color);
        break;
      }
      Message::Ping(_) | Message::Pong(_) => {}
      _ => {}
    }
  }

  Ok(())
}

async fn send(
  url: &str,
  message: &str,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
) -> Result<(), NetError> {
  let connect_fut = connect_async(url);
  let (ws_stream, _) = if let Some(dur) = timeout {
    tokio::time::timeout(dur, connect_fut)
      .await
      .map_err(|_| {
        NetError::new(
          ErrorCode::ConnectionTimeout,
          "WebSocket connection timed out",
          Protocol::Ws,
        )
      })?
      .map_err(map_ws_error)?
  } else {
    connect_fut.await.map_err(map_ws_error)?
  };

  let (mut write, mut read) = ws_stream.split();

  write.send(Message::Text(message.into())).await.map_err(map_ws_error)?;

  // Wait for one response
  if let Some(msg) = read.next().await {
    let msg = msg.map_err(map_ws_error)?;
    if let Message::Text(text) = msg {
      let text_str: &str = &text;
      let data: serde_json::Value = serde_json::from_str(text_str).unwrap_or_else(|_| json!(text_str));
      let response = NetResponse::new(ResponseType::Message, Protocol::Ws, data);
      print_response(&response, mode, no_color);
    }
  }

  write.send(Message::Close(None)).await.map_err(map_ws_error)?;

  Ok(())
}

fn map_ws_error(e: tokio_tungstenite::tungstenite::Error) -> NetError {
  match e {
    tokio_tungstenite::tungstenite::Error::Io(ref io_err) => match io_err.kind() {
      std::io::ErrorKind::ConnectionRefused => NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Ws),
      std::io::ErrorKind::TimedOut => NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Ws),
      _ => NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Ws),
    },
    _ => NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Ws),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::ErrorCode;

  #[test]
  fn map_ws_error_io_connection_refused() {
    let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let err = tokio_tungstenite::tungstenite::Error::Io(io_err);
    let net_err = map_ws_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionRefused);
  }

  #[test]
  fn map_ws_error_io_timed_out() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    let err = tokio_tungstenite::tungstenite::Error::Io(io_err);
    let net_err = map_ws_error(err);
    assert_eq!(net_err.code, ErrorCode::ConnectionTimeout);
  }

  #[test]
  fn map_ws_error_protocol() {
    let err = tokio_tungstenite::tungstenite::Error::ConnectionClosed;
    let net_err = map_ws_error(err);
    assert_eq!(net_err.code, ErrorCode::ProtocolError);
  }
}
