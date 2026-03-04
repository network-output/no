mod helpers;

use helpers::cli::{no_cmd, parse_all_json};

#[test]
fn ping_missing_host() {
  let output = no_cmd().args(["ping"]).output().unwrap();
  assert!(!output.status.success(), "expected failure when host is missing");
}

#[test]
#[ignore]
fn ping_localhost() {
  let output = no_cmd().args(["-n", "2", "ping", "127.0.0.1"]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  let responses: Vec<_> = events.iter().filter(|e| e["type"] == "response").collect();
  assert_eq!(messages.len(), 2, "expected 2 message events");
  assert_eq!(responses.len(), 1, "expected 1 response event");
  assert_eq!(messages[0]["protocol"], "ping");
  assert!(messages[0]["data"]["seq"].is_number());
  assert!(messages[0]["data"]["time_ms"].is_number());
  let summary = &responses[0]["data"];
  assert_eq!(summary["transmitted"], 2);
  assert_eq!(summary["received"], 2);
  assert_eq!(summary["loss_pct"], 0.0);
}

#[test]
#[ignore]
fn ping_ipv4() {
  let output = no_cmd().args(["ping", "127.0.0.1"]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(messages.len(), 4, "expected default 4 pings");
}

#[test]
#[ignore]
fn ping_count() {
  let output = no_cmd().args(["-n", "1", "ping", "127.0.0.1"]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  let responses: Vec<_> = events.iter().filter(|e| e["type"] == "response").collect();
  assert_eq!(messages.len(), 1, "expected exactly 1 message");
  assert_eq!(responses.len(), 1, "expected exactly 1 response");
}

#[test]
#[ignore]
fn ping_timeout() {
  let output = no_cmd()
    .args(["--timeout", "1ms", "ping", "192.0.2.1"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  let responses: Vec<_> = events.iter().filter(|e| e["type"] == "response").collect();
  assert!(messages.is_empty(), "expected no message events for timed out pings");
  assert_eq!(responses.len(), 1);
  let summary = &responses[0]["data"];
  assert_eq!(summary["loss_pct"], 100.0);
  assert_eq!(summary["received"], 0);
}

#[test]
#[ignore]
fn ping_bracketed_ipv6_loopback() {
  let output = no_cmd().args(["-n", "1", "ping", "[::1]"]).output().unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(messages.len(), 1, "expected 1 ping reply from [::1]");
  assert_eq!(messages[0]["data"]["ip"], "::1");
}
