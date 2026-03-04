use std::io::Write;
use std::process::{Command, Stdio};

fn no_cmd() -> Command {
  Command::new(env!("CARGO_BIN_EXE_no"))
}

#[test]
fn help_output() {
  let output = no_cmd().arg("--help").output().expect("failed to execute process");
  assert!(output.status.success(), "expected success exit code");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("networking CLI"),
    "expected stdout to contain 'networking CLI', got: {stdout}"
  );
}

#[test]
fn version_output() {
  let output = no_cmd().arg("--version").output().expect("failed to execute process");
  assert!(output.status.success(), "expected success exit code");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains(env!("CARGO_PKG_VERSION")),
    "expected stdout to contain version '{}', got: {stdout}",
    env!("CARGO_PKG_VERSION")
  );
}

#[test]
fn invalid_subcommand_fails() {
  let output = no_cmd().arg("notacommand").output().expect("failed to execute process");
  assert!(
    !output.status.success(),
    "expected non-zero exit code for unknown subcommand"
  );
}

#[test]
fn http_missing_args_fails() {
  let output = no_cmd().arg("http").output().expect("failed to execute process");
  assert!(
    !output.status.success(),
    "expected non-zero exit code when http subcommand has no args"
  );
}

#[test]
fn http_invalid_url_exits_nonzero() {
  let output = no_cmd()
    .args(["http", "GET", "http://192.0.2.1:1", "--timeout", "1s", "--json"])
    .output()
    .expect("failed to execute process");
  assert!(
    !output.status.success(),
    "expected non-zero exit code for unreachable URL"
  );
}

#[test]
fn help_shows_exit_codes() {
  let output = no_cmd().arg("--help").output().expect("failed to execute process");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("EXIT CODES:"), "expected EXIT CODES section in help");
  assert!(
    stdout.contains("ENVIRONMENT VARIABLES:"),
    "expected ENVIRONMENT VARIABLES section in help"
  );
  assert!(stdout.contains("NO_AUTH_TOKEN"), "expected NO_AUTH_TOKEN in help");
  assert!(stdout.contains("NO_BASIC_AUTH"), "expected NO_BASIC_AUTH in help");
}

#[test]
fn jq_flag_in_help() {
  let output = no_cmd().arg("--help").output().expect("failed to execute process");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("--jq"), "expected --jq in help output");
}

fn jq_with_stdin(filter: &str, input: &str) -> std::process::Output {
  let mut child = no_cmd()
    .args(["jq", filter])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("failed to spawn process");
  child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
  child.wait_with_output().expect("failed to wait for process")
}

#[test]
fn jq_stdin_object() {
  let output = jq_with_stdin(".a", r#"{"a":1}"#);
  assert!(output.status.success());
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
}

#[test]
fn jq_stdin_string_raw() {
  let output = jq_with_stdin(".s", r#"{"s":"hello"}"#);
  assert!(output.status.success());
  assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
fn jq_stdin_array() {
  let output = jq_with_stdin(".[]", "[1,2,3]");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  let lines: Vec<&str> = stdout.trim().lines().collect();
  assert_eq!(lines, vec!["1", "2", "3"]);
}

#[test]
fn jq_stdin_invalid_filter() {
  let output = jq_with_stdin(".[[bad", "{}");
  assert!(!output.status.success());
  assert_eq!(output.status.code(), Some(4));
}

#[test]
fn jq_stdin_invalid_json() {
  let output = jq_with_stdin(".", "not json");
  assert!(!output.status.success());
  assert_eq!(output.status.code(), Some(4));
}

// requires network
#[test]
#[ignore]
fn json_flag_produces_valid_json() {
  if std::env::var("NO_NETWORK_TESTS").is_ok() {
    return;
  }
  let output = no_cmd()
    .args(["--json", "http", "GET", "https://httpbin.org/get"])
    .output()
    .expect("failed to execute process");
  if output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is not valid JSON");
    let obj = parsed.as_object().expect("expected JSON object at top level");
    for key in &["type", "protocol", "timestamp", "data"] {
      assert!(
        obj.contains_key(*key),
        "expected JSON to contain key '{key}', got: {parsed}"
      );
    }
  }
}
