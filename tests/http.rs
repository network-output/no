mod helpers;

use helpers::cli::{exit_code, free_port, no_cmd, parse_first_json};
use helpers::server::TestServer;

#[test]
fn http_get_success() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success(), "expected success exit code");
  let json = parse_first_json(&output);
  assert_eq!(json["type"], "response");
  assert_eq!(json["protocol"], "http");
  assert_eq!(json["data"]["status"], 200);
  assert!(json["data"]["body"].is_object());
}

#[test]
fn http_post_with_body() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "POST", &server.http_url("/post"), "-b", "hello world"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 200);
  assert_eq!(json["data"]["body"]["body"], "hello world");
}

#[test]
fn http_custom_headers() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/get"), "-H", "X-Custom:test-value"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["body"]["headers"]["x-custom"], "test-value");
}

#[test]
fn http_multiple_headers() {
  let server = TestServer::start();
  let output = no_cmd()
    .args([
      "http",
      "GET",
      &server.http_url("/get"),
      "-H",
      "X-First:one",
      "-H",
      "X-Second:two",
    ])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["body"]["headers"]["x-first"], "one");
  assert_eq!(json["data"]["body"]["headers"]["x-second"], "two");
}

#[test]
fn http_bearer_auth_success() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/auth"), "--bearer", "tok123"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 200);
  assert_eq!(json["data"]["body"]["authenticated"], true);
  assert_eq!(json["data"]["body"]["token"], "tok123");
}

#[test]
fn http_bearer_auth_failure() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/auth")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 401);
  assert_eq!(json["data"]["body"]["authenticated"], false);
}

#[test]
fn http_basic_auth() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/get"), "--basic", "user:pass"])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  let auth_header = json["data"]["body"]["headers"]["authorization"].as_str().unwrap_or("");
  assert!(
    auth_header.starts_with("Basic "),
    "expected Basic auth header, got: {auth_header}"
  );
}

#[test]
fn http_file_download() {
  let server = TestServer::start();
  let tmp_file = std::env::temp_dir().join(format!("no-test-download-{}", std::process::id()));
  let output = no_cmd()
    .args([
      "http",
      "GET",
      &server.http_url("/download"),
      "-o",
      tmp_file.to_str().unwrap(),
    ])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 200);
  assert!(json["data"]["bytes"].as_u64().unwrap() > 0);
  assert!(tmp_file.exists(), "downloaded file should exist");
  let content = std::fs::read(&tmp_file).unwrap();
  assert_eq!(content, b"test download content here");
  let _ = std::fs::remove_file(&tmp_file);
}

#[test]
fn http_timeout_triggers() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--timeout", "500ms", "http", "GET", &server.http_url("/slow")])
    .output()
    .unwrap();
  assert!(!output.status.success());
  let code = output.status.code().unwrap();
  assert_eq!(code, exit_code::TIMEOUT, "expected exit code TIMEOUT, got: {code}");
}

#[test]
fn http_connection_refused() {
  let port = free_port();
  let output = no_cmd()
    .args([
      "--timeout",
      "2s",
      "http",
      "GET",
      &format!("http://127.0.0.1:{port}/get"),
    ])
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
fn http_invalid_method() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET/POST", &server.http_url("/get")])
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

#[test]
fn http_verbose_metadata() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["-v", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert!(json["metadata"].is_object(), "expected metadata in verbose output");
  assert_eq!(json["metadata"]["method"], "GET");
}

#[test]
fn http_status_code_passthrough() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/status/404")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 404);
}

#[test]
fn http_body_json_auto_parse() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  // The /get endpoint returns JSON, so body should be parsed as an object
  assert!(json["data"]["body"].is_object(), "expected body to be parsed as object");
}

#[test]
fn http_env_bearer_fallback() {
  let server = TestServer::start();
  let output = no_cmd()
    .env("NO_AUTH_TOKEN", "env-token-123")
    .args(["http", "GET", &server.http_url("/auth")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let json = parse_first_json(&output);
  assert_eq!(json["data"]["status"], 200);
  assert_eq!(json["data"]["body"]["authenticated"], true);
  assert_eq!(json["data"]["body"]["token"], "env-token-123");
}

#[test]
fn http_no_color_pretty() {
  let server = TestServer::start();
  // Use --json to get structured output, but also test that --no-color --pretty doesn't crash
  let output = no_cmd()
    .args(["--no-color", "--pretty", "http", "GET", &server.http_url("/get")])
    // Remove the --json flag that no_cmd adds by creating a fresh command
    .output()
    .unwrap();
  // no_cmd already adds --json, so output will be JSON regardless
  // Just verify it succeeds and produces output
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(!stdout.is_empty(), "expected non-empty output");
  // ANSI escape sequences start with \x1b[
  assert!(
    !stdout.contains("\x1b["),
    "expected no ANSI escapes in output with --no-color"
  );
}

#[test]
fn http_no_color_pretty_clean() {
  let server = TestServer::start();
  // Build a fresh command without --json to test --no-color --pretty
  let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_no"));
  cmd.env_remove("NO_AUTH_TOKEN");
  cmd.env_remove("NO_BASIC_AUTH");
  let output = cmd
    .args(["--no-color", "--pretty", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(!stdout.is_empty());
  assert!(!stdout.contains("\x1b["), "expected no ANSI escapes with --no-color");
}

#[test]
fn http_jq_filter_status() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".data.status", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert_eq!(stdout, "200", "expected raw status 200, got: {stdout}");
}

#[test]
fn http_jq_filter_string_raw() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".protocol", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert_eq!(stdout, "http", "expected raw string 'http' (no quotes), got: {stdout}");
}

#[test]
fn http_jq_filter_nested() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".data.body", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("expected valid JSON object");
  assert!(parsed.is_object(), "expected JSON object, got: {stdout}");
}

#[test]
fn http_jq_filter_type() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".type", "http", "GET", &server.http_url("/get")])
    .output()
    .unwrap();
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert_eq!(stdout, "response", "expected raw 'response', got: {stdout}");
}

#[test]
fn http_jq_invalid_expression() {
  let server = TestServer::start();
  let output = no_cmd()
    .args(["--jq", ".[[bad", "http", "GET", &server.http_url("/get")])
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
