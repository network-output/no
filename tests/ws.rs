mod helpers;

use helpers::cli::{exit_code, free_port, no_cmd, parse_all_json, parse_first_json};
use helpers::server::TestServer;

#[test]
fn ws_listen_text_messages() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/multi")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(messages.len(), 3, "expected 3 text messages, got: {}", messages.len());
}

#[test]
fn ws_listen_close_with_reason() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/close")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let close_events: Vec<_> = events
    .iter()
    .filter(|e| e["type"] == "connection" && e["data"]["status"] == "closed")
    .collect();
  assert!(!close_events.is_empty(), "expected a close event");
  let reason = close_events[0]["data"]["reason"].as_str().unwrap_or("");
  assert!(!reason.is_empty(), "expected close reason to be non-empty");
}

#[test]
fn ws_listen_binary() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/binary")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let binary_msgs: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!binary_msgs.is_empty(), "expected binary message");
  assert_eq!(binary_msgs[0]["data"]["binary"], true);
  assert!(binary_msgs[0]["data"]["length"].as_u64().unwrap() > 0);
}

#[test]
fn ws_send_echo() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "send", &server.ws_url("/ws/echo"), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "message");
  assert_eq!(json["data"], "hello");
}

#[test]
fn ws_send_empty() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "send", &server.ws_url("/ws/echo"), "-m", ""])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "message");
  assert_eq!(json["data"], "");
}

#[test]
fn ws_connected_event() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/close")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  assert!(!events.is_empty());
  assert_eq!(events[0]["type"], "connection");
  assert_eq!(events[0]["data"]["status"], "connected");
}

#[test]
fn ws_closed_event() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/close")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let has_connected = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "connected");
  let has_closed = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "closed");
  assert!(has_connected, "expected connected event");
  assert!(has_closed, "expected closed event");
}

#[test]
fn ws_json_auto_parse() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["ws", "listen", &server.ws_url("/ws/multi")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty());
  // The multi endpoint sends JSON objects, so data should be parsed
  assert!(messages[0]["data"].is_object(), "expected JSON auto-parsed as object");
}

#[test]
fn ws_listen_count_limits_messages() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--count", "1", "ws", "listen", &server.ws_url("/ws/multi")])
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
fn ws_listen_verbose_metadata() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["-v", "ws", "listen", &server.ws_url("/ws/multi")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty());
  assert!(
    messages[0]["metadata"]["message_number"].is_number(),
    "expected message_number in metadata"
  );
}

#[test]
fn ws_connection_refused() {
  let port = free_port();
  let output = no_cmd()
    .args(["ws", "listen", &format!("ws://127.0.0.1:{port}/ws")])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert!(
    code == exit_code::CONNECTION || code == exit_code::PROTOCOL,
    "expected exit code CONNECTION or PROTOCOL, got: {code}"
  );
}

#[test]
fn ws_connection_timeout() {
  let output = no_cmd()
    .args(["--timeout", "500ms", "ws", "listen", "ws://192.0.2.1:1/ws"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(code, exit_code::TIMEOUT, "expected exit code TIMEOUT, got: {code}");
}

#[test]
fn ws_jq_filter_echo() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".data", "ws", "send", &server.ws_url("/ws/echo"), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert_eq!(stdout, "hello", "expected raw 'hello', got: {stdout}");
}
