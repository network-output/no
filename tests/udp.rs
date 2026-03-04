mod helpers;

use helpers::cli::{no_cmd, parse_all_json, parse_first_json};
use helpers::udp_server;

#[test]
fn udp_send_message() {
  let server = udp_server::start_echo_server();
  let output = no_cmd()
    .args(["udp", "send", &server.addr.to_string(), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "connection");
  assert_eq!(json["data"]["status"], "sent");
  assert_eq!(json["data"]["bytes"], 5);
}

#[test]
fn udp_send_receive() {
  let server = udp_server::start_echo_server();
  let output = no_cmd()
    .args(["udp", "send", &server.addr.to_string(), "-m", "hello", "--wait", "1s"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty(), "expected at least one message event");
  assert_eq!(messages[0]["data"]["data"], "hello");
}

#[test]
fn udp_listen_receive() {
  // Start listener on :0
  let mut child = no_cmd()
    .args(["udp", "listen", ":0"])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  // Read the first line to get the listening address
  use std::io::BufRead;
  let stdout = child.stdout.take().unwrap();
  let mut reader = std::io::BufReader::new(stdout);
  let mut first_line = String::new();
  reader.read_line(&mut first_line).unwrap();

  let listen_json: serde_json::Value = serde_json::from_str(&first_line).unwrap();
  let listen_addr = listen_json["data"]["address"].as_str().unwrap();
  // The listener binds to 0.0.0.0:PORT, so send to 127.0.0.1:PORT
  let port = listen_addr.rsplit_once(':').unwrap().1;
  let send_addr = format!("127.0.0.1:{port}");

  // Send a UDP datagram using std::net
  {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.send_to(b"test message", &send_addr).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
  }

  // Kill the listener and read output
  child.kill().ok();
  let remaining_output = {
    let mut buf = String::new();
    use std::io::Read;
    reader.read_to_string(&mut buf).ok();
    buf
  };
  child.wait().ok();

  let all_output = format!("{first_line}{remaining_output}");
  let events: Vec<serde_json::Value> = all_output
    .lines()
    .filter_map(|line| serde_json::from_str(line).ok())
    .collect();

  let has_listening = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "listening");
  let has_message = events.iter().any(|e| e["type"] == "message");

  assert!(has_listening, "expected listening event in: {events:?}");
  assert!(has_message, "expected message event in: {events:?}");
}

#[test]
fn udp_listen_actual_port() {
  let mut child = no_cmd()
    .args(["udp", "listen", ":0"])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  use std::io::BufRead;
  let stdout = child.stdout.take().unwrap();
  let mut reader = std::io::BufReader::new(stdout);
  let mut first_line = String::new();
  reader.read_line(&mut first_line).unwrap();

  let json: serde_json::Value = serde_json::from_str(&first_line).unwrap();
  let addr = json["data"]["address"].as_str().unwrap();

  assert!(!addr.ends_with(":0"), "expected real port, got: {addr}");
  assert!(addr.contains(':'), "expected host:port format, got: {addr}");

  let port: u16 = addr.rsplit_once(':').unwrap().1.parse().unwrap();
  assert!(port > 0, "expected port > 0, got: {port}");

  child.kill().ok();
  child.wait().ok();
}

#[test]
fn udp_count_limits_messages() {
  let server = udp_server::start_multi_message_server();
  let output = no_cmd()
    .args([
      "--count",
      "1",
      "udp",
      "send",
      &server.addr.to_string(),
      "-m",
      "trigger",
      "--wait",
      "1s",
    ])
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
fn udp_send_timeout() {
  let server = udp_server::start_silent_server();
  let output = no_cmd()
    .args([
      "udp",
      "send",
      &server.addr.to_string(),
      "-m",
      "hello",
      "--wait",
      "500ms",
    ])
    .output()
    .unwrap();
  // --wait with duration exits cleanly (exit 0) when timeout expires
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert_eq!(messages.len(), 0, "expected no messages from silent server");
}
