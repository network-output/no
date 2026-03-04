//! Structured output formatting and response envelope types.
//!
//! All protocol handlers emit results through [`NetResponse`], the canonical JSON envelope
//! that wraps every piece of output produced by `no`. The envelope carries a [`ResponseType`]
//! tag, the originating [`Protocol`], a UTC timestamp, an arbitrary JSON `data` payload, and
//! optional `metadata`. Rendering is delegated to [`print_response`], which formats the
//! envelope according to the active [`OutputMode`].

use chrono::Utc;
use owo_colors::OwoColorize;
use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;

static JQ_FILTER: OnceLock<String> = OnceLock::new();

/// Discriminant describing the nature of a [`NetResponse`] payload.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseType {
  /// Single request-response result (HTTP responses).
  Response,
  /// Streamed data message (WebSocket frames, MQTT messages, SSE events).
  Message,
  /// Connection lifecycle event (connect, disconnect, listen).
  Connection,
  /// Protocol or application error.
  Error,
}

/// Identifier for the network protocol that produced a [`NetResponse`].
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
  /// HTTP/HTTPS.
  Http,
  /// WebSocket (WS/WSS).
  Ws,
  /// Raw TCP.
  Tcp,
  /// MQTT.
  Mqtt,
  /// Server-Sent Events.
  Sse,
  /// UDP datagrams.
  Udp,
  /// DNS lookups.
  Dns,
  /// ICMP ping.
  Ping,
  /// WHOIS lookups.
  Whois,
}

/// Canonical JSON envelope wrapping all protocol output.
///
/// Every successful result and every error emitted by `no` is wrapped in this struct before
/// being serialized. The `type` field (renamed from `kind` during serialization) lets consumers
/// dispatch on the payload shape without inspecting the inner `data`.
#[derive(Debug, Serialize)]
pub struct NetResponse {
  /// The kind of payload contained in this response (serialized as `type`).
  #[serde(rename = "type")]
  pub kind: ResponseType,
  /// The protocol that produced this response.
  pub protocol: Protocol,
  /// RFC 3339 UTC timestamp with millisecond precision.
  pub timestamp: String,
  /// Protocol-specific payload.
  pub data: serde_json::Value,
  /// Optional auxiliary information (e.g. HTTP headers, timing data).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub metadata: Option<serde_json::Value>,
}

impl NetResponse {
  /// Create a response with the current UTC timestamp.
  pub fn new(kind: ResponseType, protocol: Protocol, data: serde_json::Value) -> Self {
    Self {
      kind,
      protocol,
      timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
      data,
      metadata: None,
    }
  }

  /// Attach optional metadata to an existing response.
  pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
    self.metadata = Some(metadata);
    self
  }
}

/// Controls how [`NetResponse`] values are rendered to stdout.
#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
  /// Machine-readable, one JSON object per line.
  Json,
  /// Human-readable with colors and formatting.
  Pretty,
}

impl OutputMode {
  /// Resolve the output mode from CLI flags with TTY-aware fallback.
  ///
  /// `--json` takes precedence over `--pretty`. When neither flag is set, the mode
  /// defaults to [`Pretty`](OutputMode::Pretty) if stdout is a terminal, or
  /// [`Json`](OutputMode::Json) otherwise.
  pub fn detect(json_flag: bool, pretty_flag: bool) -> Self {
    if json_flag {
      return OutputMode::Json;
    }
    if pretty_flag {
      return OutputMode::Pretty;
    }
    if io::stdout().is_terminal() {
      OutputMode::Pretty
    } else {
      OutputMode::Json
    }
  }
}

/// Validate a jq expression and store it for later use.
/// Returns `Err` with a message if the expression has syntax errors.
pub fn init_jq_filter(expr: &str) -> Result<(), String> {
  compile_filter(expr).map_err(|e| format!("invalid jq expression: {e}"))?;
  JQ_FILTER
    .set(expr.to_owned())
    .map_err(|_| "jq filter already initialized".to_owned())
}

pub(crate) fn compile_filter(expr: &str) -> Result<jaq_core::Filter<jaq_core::Native<jaq_json::Val>>, String> {
  use jaq_core::load::{Arena, File, Loader};

  let arena = Arena::default();
  let defs = jaq_std::defs().chain(jaq_json::defs());
  let loader = Loader::new(defs);
  let modules = loader
    .load(&arena, File { path: (), code: expr })
    .map_err(|errs| format!("{errs:?}"))?;
  let funs = jaq_std::funs().chain(jaq_json::funs());
  jaq_core::Compiler::default()
    .with_funs(funs)
    .compile(modules)
    .map_err(|errs| format!("{errs:?}"))
}

pub(crate) fn run_jq_filter(
  expr: &str,
  input: serde_json::Value,
) -> Vec<Result<jaq_json::Val, jaq_core::Error<jaq_json::Val>>> {
  use jaq_core::{Ctx, RcIter};

  let filter = match compile_filter(expr) {
    Ok(f) => f,
    Err(e) => {
      eprintln!("jq compile error: {e}");
      return Vec::new();
    }
  };

  let inputs = RcIter::new(core::iter::empty());
  let input_val: jaq_json::Val = input.into();
  filter.run((Ctx::new([], &inputs), input_val)).collect()
}

pub(crate) fn print_jq_value(val: jaq_json::Val) {
  let mut stdout = io::stdout().lock();
  let json_val: serde_json::Value = val.into();
  if let serde_json::Value::String(s) = &json_val {
    writeln!(stdout, "{s}").ok();
  } else {
    writeln!(stdout, "{}", serde_json::to_string(&json_val).unwrap_or_default()).ok();
  }
}

/// Serialize and print a [`NetResponse`] in the active output mode.
///
/// When a global jq filter is active (set via [`init_jq_filter`]), non-error responses are
/// piped through the filter instead of being rendered directly. Error responses always bypass
/// the filter so that failures are never silently swallowed.
pub fn print_response(response: &NetResponse, mode: OutputMode, no_color: bool) {
  if let Some(expr) = JQ_FILTER.get() {
    if !matches!(response.kind, ResponseType::Error) {
      let value = serde_json::to_value(response).unwrap_or_default();
      let results = run_jq_filter(expr, value);
      for result in results {
        match result {
          Ok(val) => print_jq_value(val),
          Err(e) => eprintln!("jq error: {e}"),
        }
      }
      return;
    }
  }

  match mode {
    OutputMode::Json => print_json(response),
    OutputMode::Pretty => print_pretty(response, no_color),
  }
}

fn print_json(response: &NetResponse) {
  let mut stdout = io::stdout().lock();
  serde_json::to_writer(&mut stdout, response).expect("failed to write JSON");
  writeln!(stdout).expect("failed to write newline");
}

fn print_pretty(response: &NetResponse, no_color: bool) {
  let mut stdout = io::stdout().lock();

  let type_str = format!("{:?}", response.kind).to_uppercase();
  let protocol_str = serde_json::to_value(response.protocol)
    .ok()
    .and_then(|v| v.as_str().map(|s| s.to_uppercase()))
    .unwrap_or_default();

  if no_color {
    writeln!(stdout, "[{protocol_str}] {type_str} @ {}", response.timestamp).ok();
    writeln!(
      stdout,
      "{}",
      serde_json::to_string_pretty(&response.data).unwrap_or_default()
    )
    .ok();
  } else {
    writeln!(
      stdout,
      "{} {} @ {}",
      format!("[{protocol_str}]").cyan(),
      type_str.bold(),
      response.timestamp.dimmed()
    )
    .ok();
    writeln!(
      stdout,
      "{}",
      serde_json::to_string_pretty(&response.data).unwrap_or_default()
    )
    .ok();
  }

  if let Some(ref meta) = response.metadata {
    if no_color {
      writeln!(
        stdout,
        "metadata: {}",
        serde_json::to_string_pretty(meta).unwrap_or_default()
      )
      .ok();
    } else {
      writeln!(
        stdout,
        "{} {}",
        "metadata:".dimmed(),
        serde_json::to_string_pretty(meta).unwrap_or_default().dimmed()
      )
      .ok();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn detect_json_flag_overrides() {
    let mode = OutputMode::detect(true, false);
    assert!(matches!(mode, OutputMode::Json));
  }

  #[test]
  fn detect_pretty_flag() {
    let mode = OutputMode::detect(false, true);
    assert!(matches!(mode, OutputMode::Pretty));
  }

  #[test]
  fn detect_json_takes_precedence() {
    let mode = OutputMode::detect(true, true);
    assert!(matches!(mode, OutputMode::Json));
  }

  #[test]
  fn net_response_new_has_required_fields() {
    let response = NetResponse::new(ResponseType::Response, Protocol::Http, json!({}));
    assert!(matches!(response.kind, ResponseType::Response));
    assert!(matches!(response.protocol, Protocol::Http));
    assert!(!response.timestamp.is_empty());
    assert!(response.timestamp.ends_with('Z'));
    assert_eq!(response.data, json!({}));
    assert!(response.metadata.is_none());
  }

  #[test]
  fn net_response_with_metadata() {
    let meta = json!({"status": 200, "headers": {}});
    let response = NetResponse::new(ResponseType::Response, Protocol::Http, json!({})).with_metadata(meta.clone());
    assert!(response.metadata.is_some());
    assert_eq!(response.metadata.unwrap(), meta);
  }

  #[test]
  fn json_serialization_type_field_renamed() {
    let response = NetResponse::new(ResponseType::Response, Protocol::Http, json!({"key": "val"}));
    let value = serde_json::to_value(&response).unwrap();
    assert!(value.get("type").is_some(), "expected 'type' key in serialized JSON");
    assert!(value.get("kind").is_none(), "unexpected 'kind' key in serialized JSON");
    assert_eq!(value["protocol"], "http");
  }

  #[test]
  fn json_serialization_metadata_skipped_when_none() {
    let response = NetResponse::new(ResponseType::Response, Protocol::Http, json!({}));
    let value = serde_json::to_value(&response).unwrap();
    assert!(
      value.get("metadata").is_none(),
      "expected 'metadata' to be absent when None"
    );
  }

  #[test]
  fn protocol_serializes_to_lowercase() {
    let cases = [
      (Protocol::Http, "http"),
      (Protocol::Ws, "ws"),
      (Protocol::Tcp, "tcp"),
      (Protocol::Mqtt, "mqtt"),
      (Protocol::Sse, "sse"),
      (Protocol::Udp, "udp"),
      (Protocol::Dns, "dns"),
      (Protocol::Ping, "ping"),
      (Protocol::Whois, "whois"),
    ];
    for (protocol, expected) in cases {
      let value = serde_json::to_value(protocol).unwrap();
      assert_eq!(
        value, expected,
        "Protocol::{:?} should serialize to \"{}\"",
        protocol, expected
      );
    }
  }

  #[test]
  fn response_type_serializes_to_lowercase() {
    let cases = [
      (ResponseType::Response, "response"),
      (ResponseType::Message, "message"),
      (ResponseType::Connection, "connection"),
      (ResponseType::Error, "error"),
    ];
    for (response_type, expected) in cases {
      let value = serde_json::to_value(response_type).unwrap();
      assert_eq!(
        value, expected,
        "ResponseType::{:?} should serialize to \"{}\"",
        response_type, expected
      );
    }
  }
}
