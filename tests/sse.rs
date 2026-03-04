mod helpers;

use helpers::cli::{exit_code, free_port, no_cmd, parse_all_json};
use helpers::server::TestServer;

#[test]
fn sse_receive_events() {
  let server = TestServer::start();
  let output = no_cmd().args(["sse", &server.http_url("/events")]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let connection_events: Vec<_> = events.iter().filter(|e| e["type"] == "connection").collect();
  let message_events: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!connection_events.is_empty(), "expected connection event");
  assert_eq!(
    message_events.len(),
    3,
    "expected 3 message events, got: {}",
    message_events.len()
  );
}

#[test]
fn sse_json_data() {
  let server = TestServer::start();
  let output = no_cmd().args(["sse", &server.http_url("/events")]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty());
  // The SSE data is JSON, so it should be parsed as an object
  assert!(
    messages[0]["data"]["data"].is_object(),
    "expected data.data to be object, got: {}",
    messages[0]["data"]["data"]
  );
}

#[test]
fn sse_plain_text() {
  // The /events endpoint sends JSON data, so we use it and verify parsing
  // If the data was plain text, data.data would be a string
  let server = TestServer::start();
  let output = no_cmd().args(["sse", &server.http_url("/events")]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty());
  // Verify the data field exists
  assert!(
    !messages[0]["data"]["data"].is_null(),
    "expected data.data to be present"
  );
}

#[test]
fn sse_named_events() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["sse", &server.http_url("/events/named")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty(), "expected message events");
  // Check that event name and id are present
  let first = &messages[0];
  assert!(
    first["data"]["event"].is_string(),
    "expected event name, got: {}",
    first["data"]
  );
  assert!(
    first["data"]["id"].is_string(),
    "expected event id, got: {}",
    first["data"]
  );
}

#[test]
fn sse_bearer_auth_success() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["sse", &server.http_url("/events/auth"), "--bearer", "tok123"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(
    messages.len(),
    3,
    "expected 3 events with auth, got: {}",
    messages.len()
  );
}

#[test]
fn sse_bearer_auth_failure() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["sse", &server.http_url("/events/auth")])
    .output()
    .unwrap();
  // SSE handler does not fail on 401 -- it sees no SSE events
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(
    messages.len(),
    0,
    "expected no message events without auth, got: {}",
    messages.len()
  );
}

#[test]
fn sse_count_limits_messages() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--count", "1", "sse", &server.http_url("/events")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(
    messages.len(),
    1,
    "expected 1 message with --count 1, got: {}",
    messages.len()
  );
}

#[test]
fn sse_connection_refused() {
  let port = free_port();
  let output = no_cmd()
    .args(["--timeout", "2s", "sse", &format!("http://127.0.0.1:{port}/events")])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(
    code,
    exit_code::CONNECTION,
    "expected exit code CONNECTION, got: {code}"
  );
}

#[test]
fn sse_connection_timeout() {
  let output = no_cmd()
    .args(["--timeout", "500ms", "sse", "http://192.0.2.1:1/events"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(code, exit_code::TIMEOUT, "expected exit code TIMEOUT, got: {code}");
}

#[test]
fn sse_jq_filter_with_count() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".data.data", "--count", "1", "sse", &server.http_url("/events")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  // Filter out null lines from connection events
  let data_lines: Vec<_> = stdout
    .lines()
    .filter(|l| !l.trim().is_empty() && l.trim() != "null")
    .collect();
  assert_eq!(data_lines.len(), 1, "expected 1 data line, got: {data_lines:?}");
  let parsed: serde_json::Value = serde_json::from_str(data_lines[0]).expect("expected valid JSON");
  assert!(parsed.is_object(), "expected JSON object, got: {}", data_lines[0]);
}
