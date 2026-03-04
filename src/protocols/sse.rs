//! Server-Sent Events client handler for streaming event sources.

use crate::cli::SseArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use crate::url::{UrlScheme, normalize_url};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;
use std::str::FromStr;
use std::time::Duration;

/// Connect to an SSE endpoint and stream incoming events as structured output.
///
/// Supports custom headers, bearer token and basic authentication. Each SSE event is emitted as a
/// separate [`NetResponse`] message with the event type, data, and
/// optional ID.
///
/// # Errors
///
/// Returns [`NetError`] on connection failure, HTTP errors (non-2xx status), authentication
/// errors, or stream interruption.
pub async fn run(
  args: SseArgs,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let url = normalize_url(&args.url, UrlScheme::Http);

  let mut client_builder = reqwest::Client::builder();
  if let Some(dur) = timeout {
    client_builder = client_builder.connect_timeout(dur);
  }
  let client = client_builder
    .build()
    .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Sse))?;

  let mut request = client.get(&url);

  // Headers
  let mut headers = HeaderMap::new();
  for h in &args.headers {
    let (key, value) = h.split_once(':').ok_or_else(|| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header format: {h}"),
        Protocol::Sse,
      )
    })?;
    let name = HeaderName::from_str(key.trim()).map_err(|_| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header name: {key}"),
        Protocol::Sse,
      )
    })?;
    let val = HeaderValue::from_str(value.trim()).map_err(|_| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header value: {value}"),
        Protocol::Sse,
      )
    })?;
    headers.insert(name, val);
  }
  request = request.headers(headers);

  // Auth
  let bearer = args.bearer.or_else(|| std::env::var("NO_AUTH_TOKEN").ok());
  if let Some(token) = bearer {
    request = request.bearer_auth(token);
  } else {
    let basic = args.basic.or_else(|| std::env::var("NO_BASIC_AUTH").ok());
    if let Some(creds) = basic {
      let (user, pass) = creds.split_once(':').unwrap_or((&creds, ""));
      request = request.basic_auth(user, Some(pass));
    }
  }

  let response = request.send().await.map_err(|e| {
    if e.is_timeout() {
      NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Sse)
    } else if e.is_connect() {
      NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Sse)
    } else {
      NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Sse)
    }
  })?;

  let connected = NetResponse::new(
    ResponseType::Connection,
    Protocol::Sse,
    json!({ "status": "connected", "url": url }),
  );
  if verbose {
    print_response(&connected.with_metadata(json!({ "url": url })), mode, no_color);
  } else {
    print_response(&connected, mode, no_color);
  }

  let mut stream = response.bytes_stream().eventsource();
  let mut message_count: usize = 0;

  while let Some(event) = stream.next().await {
    match event {
      Ok(ev) => {
        message_count += 1;
        let data: serde_json::Value = serde_json::from_str(&ev.data).unwrap_or_else(|_| json!(ev.data));
        let response = NetResponse::new(
          ResponseType::Message,
          Protocol::Sse,
          json!({
            "event": ev.event,
            "data": data,
            "id": ev.id,
          }),
        );
        if verbose {
          print_response(
            &response.with_metadata(json!({ "event_type": ev.event })),
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
      Err(e) => {
        return Err(NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Sse));
      }
    }
  }

  Ok(())
}
