mod helpers;

use helpers::cli::{exit_code, free_port, no_cmd, parse_all_json, parse_first_json};
use helpers::mqtt_broker::MQTT_BROKER;

#[test]
fn mqtt_publish_success() {
  let broker_addr = MQTT_BROKER.addr();
  let output = no_cmd()
    .args(["mqtt", "pub", &broker_addr, "-t", "test/pub", "-m", "hi"])
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "publish failed: {}",
    String::from_utf8_lossy(&output.stdout)
  );
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], "published");
}

#[test]
fn mqtt_publish_json_payload() {
  let broker_addr = MQTT_BROKER.addr();
  let payload = r#"{"key":"value","num":42}"#;
  let output = no_cmd()
    .args(["mqtt", "pub", &broker_addr, "-t", "test/json", "-m", payload])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], "published");
  assert_eq!(json["data"]["payload"], payload);
}

#[test]
fn mqtt_sub_receives() {
  let broker_addr = MQTT_BROKER.addr();
  let topic = format!("test/sub/{}", std::process::id());

  // Start subscriber in background
  let mut sub = no_cmd()
    .args(["mqtt", "sub", &broker_addr, "-t", &topic])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  // Wait for subscriber to connect
  std::thread::sleep(std::time::Duration::from_millis(1000));

  // Publish a message
  let pub_output = no_cmd()
    .args(["mqtt", "pub", &broker_addr, "-t", &topic, "-m", "test-msg"])
    .output()
    .unwrap();
  assert!(
    pub_output.status.success(),
    "publish failed: {}",
    String::from_utf8_lossy(&pub_output.stderr)
  );

  // Give subscriber time to receive
  std::thread::sleep(std::time::Duration::from_millis(1000));

  // Kill subscriber and read output
  sub.kill().ok();
  let output = sub.wait_with_output().unwrap();
  let events = parse_all_json(&output);

  let has_subscribed = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "subscribed");
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();

  assert!(has_subscribed, "expected subscribed event in: {events:?}");
  assert!(!messages.is_empty(), "expected at least one message in: {events:?}");
  assert_eq!(messages[0]["data"]["payload"], "test-msg");
}

#[test]
fn mqtt_broker_scheme() {
  let broker_addr = format!("mqtt://{}", MQTT_BROKER.addr());
  let output = no_cmd()
    .args(["mqtt", "pub", &broker_addr, "-t", "test/scheme", "-m", "hi"])
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "mqtt:// scheme failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn mqtt_broker_no_scheme() {
  let broker_addr = MQTT_BROKER.addr();
  let output = no_cmd()
    .args(["mqtt", "pub", &broker_addr, "-t", "test/noscheme", "-m", "hi"])
    .output()
    .unwrap();
  assert!(
    output.status.success(),
    "no-scheme broker failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn mqtt_connection_refused() {
  let port = free_port();
  let output = no_cmd()
    .args([
      "--timeout",
      "2s",
      "mqtt",
      "pub",
      &format!("127.0.0.1:{port}"),
      "-t",
      "test/refused",
      "-m",
      "hi",
    ])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert!(
    code == exit_code::CONNECTION || code == exit_code::TIMEOUT,
    "expected exit code CONNECTION or TIMEOUT, got: {code}"
  );
}

#[test]
fn mqtt_publish_timeout() {
  let output = no_cmd()
    .args([
      "--timeout",
      "1s",
      "mqtt",
      "pub",
      "192.0.2.1:1883",
      "-t",
      "test/timeout",
      "-m",
      "hi",
    ])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(code, exit_code::TIMEOUT, "expected exit code TIMEOUT, got: {code}");
}

#[test]
fn mqtt_invalid_port() {
  let output = no_cmd()
    .args(["mqtt", "pub", "localhost:notaport", "-t", "test/bad", "-m", "hi"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(
    code,
    exit_code::INVALID_INPUT,
    "expected exit code INVALID_INPUT, got: {code}"
  );
}
