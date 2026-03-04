mod helpers;

use helpers::cli::{exit_code, free_port, no_cmd, parse_all_json};
use helpers::tcp_server;

#[test]
fn tcp_connect_send() {
  let server = tcp_server::start_echo_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string(), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let connected = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "connected");
  assert!(connected, "expected connected event");
}

#[test]
fn tcp_connect_receive() {
  let server = tcp_server::start_echo_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string(), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(!messages.is_empty(), "expected at least one message event");
  assert_eq!(messages[0]["data"], "hello");
}

#[test]
fn tcp_connect_close() {
  let server = tcp_server::start_echo_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string(), "-m", "hello"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let closed = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "closed");
  assert!(closed, "expected closed event");
}

#[test]
fn tcp_multi_messages() {
  let server = tcp_server::start_multi_message_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string()])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let messages: Vec<_> = events.iter().filter(|e| e["type"] == "message").collect();
  assert!(
    !messages.is_empty(),
    "expected at least 1 message event, got: {}",
    messages.len()
  );
}

#[test]
fn tcp_silent_close() {
  let server = tcp_server::start_silent_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string()])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  let connected = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "connected");
  let closed = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "closed");
  assert!(connected, "expected connected event");
  assert!(closed, "expected closed event");
}

#[test]
fn tcp_count_limits_messages() {
  let server = tcp_server::start_multi_message_server();
  let output = no_cmd()
    .args(["--count", "1", "tcp", "connect", &server.addr.to_string()])
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
fn tcp_connection_refused() {
  let port = free_port();
  let output = no_cmd()
    .args(["tcp", "connect", &format!("127.0.0.1:{port}")])
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
fn tcp_connection_timeout() {
  let output = no_cmd()
    .args(["--timeout", "500ms", "tcp", "connect", "192.0.2.1:1"])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(code, exit_code::TIMEOUT, "expected exit code TIMEOUT, got: {code}");
}

#[test]
fn tcp_connected_event() {
  let server = tcp_server::start_silent_server();
  let output = no_cmd()
    .args(["tcp", "connect", &server.addr.to_string()])
    .output()
    .unwrap();
  assert!(output.status.success());
  let events = parse_all_json(&output);
  assert!(!events.is_empty());
  assert_eq!(events[0]["type"], "connection");
  assert_eq!(events[0]["data"]["status"], "connected");
}

#[test]
fn tcp_listen_accept() {
  // Start the `no tcp listen :0` process
  let mut child = no_cmd()
    .args(["tcp", "listen", ":0"])
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

  // Connect a client and send a message using std::net
  {
    use std::io::Write;
    let mut stream = std::net::TcpStream::connect(listen_addr).unwrap();
    stream.write_all(b"test message").unwrap();
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
  }

  // Kill the listener and wait to avoid zombie
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

  let has_accepted = events
    .iter()
    .any(|e| e["type"] == "connection" && e["data"]["status"] == "accepted");
  let has_message = events.iter().any(|e| e["type"] == "message");

  assert!(has_accepted, "expected accepted event in: {events:?}");
  assert!(has_message, "expected message event in: {events:?}");
}

#[test]
fn tcp_listen_actual_port() {
  // Start listening on :0
  let mut child = no_cmd()
    .args(["tcp", "listen", ":0"])
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

  // The address should NOT be "0.0.0.0:0" -- it should have a real port
  assert!(!addr.ends_with(":0"), "expected real port, got: {addr}");
  assert!(addr.contains(':'), "expected host:port format, got: {addr}");

  // Parse the port and verify it's > 0
  let port: u16 = addr.rsplit_once(':').unwrap().1.parse().unwrap();
  assert!(port > 0, "expected port > 0, got: {port}");

  child.kill().ok();
  child.wait().ok();
}
