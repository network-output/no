use std::process::{Command, Output};

/// Exit codes from src/error.rs ErrorCode enum:
///   1 = ConnectionRefused | DnsResolution | IoError
///   2 = ProtocolError | TlsError
///   3 = ConnectionTimeout
///   4 = InvalidInput
pub mod exit_code {
  pub const CONNECTION: i32 = 1;
  pub const PROTOCOL: i32 = 2;
  pub const TIMEOUT: i32 = 3;
  pub const INVALID_INPUT: i32 = 4;
}

/// Build a `Command` for the `no` binary with `--json` output and clean env.
/// Always strips `NO_AUTH_TOKEN` and `NO_BASIC_AUTH` to isolate tests.
pub fn no_cmd() -> Command {
  let mut cmd = Command::new(env!("CARGO_BIN_EXE_no"));
  cmd.arg("--json");
  cmd.env_remove("NO_AUTH_TOKEN");
  cmd.env_remove("NO_BASIC_AUTH");
  cmd
}

/// Parse the first valid JSON line from the process stdout.
/// Panics with diagnostic info if no JSON is found.
pub fn parse_first_json(output: &Output) -> serde_json::Value {
  let stdout = String::from_utf8_lossy(&output.stdout);
  for line in stdout.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
      return val;
    }
  }
  panic!(
    "no valid JSON found in stdout.\nstdout: {stdout}\nstderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// Parse all valid JSON lines from the process stdout.
/// Skips empty lines and lines that aren't valid JSON.
///
/// Typical output for streaming protocols (WS, TCP, SSE, MQTT sub):
///   line 0: {"type":"connection", ...}   -- lifecycle event
///   line 1: {"type":"message", ...}      -- data frame
///   ...
///   line N: {"type":"connection", ...}   -- close event
pub fn parse_all_json(output: &Output) -> Vec<serde_json::Value> {
  let stdout = String::from_utf8_lossy(&output.stdout);
  let mut results = Vec::new();
  for line in stdout.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
      results.push(val);
    }
  }
  results
}

/// Find a free TCP port by binding to :0 and immediately closing.
/// The OS may reuse the port, but in practice this is reliable for tests.
pub fn free_port() -> u16 {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
  listener.local_addr().unwrap().port()
}
