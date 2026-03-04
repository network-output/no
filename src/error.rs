//! Error types for structured error reporting across all protocols.
//!
//! Every failure in `no` is represented as a [`NetError`] carrying a categorized [`ErrorCode`],
//! a human-readable message, and the [`Protocol`] context where the error originated.
//! Errors are rendered through the same [`NetResponse`] envelope used for successful output,
//! ensuring that consumers always receive a consistent JSON shape.

use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use serde::Serialize;
use std::fmt;
use std::process;

/// Categorized error codes mapping network failures to process exit codes.
///
/// Each variant captures a distinct failure class so that callers can branch on the
/// serialized `SCREAMING_SNAKE_CASE` string or on the numeric exit code returned by
/// [`ErrorCode::exit_code`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum ErrorCode {
  /// Remote host actively refused the connection.
  ConnectionRefused,
  /// Connection or read operation exceeded the timeout.
  ConnectionTimeout,
  /// Failed to resolve the hostname to an IP address.
  DnsResolution,
  /// TLS handshake or certificate verification failed.
  TlsError,
  /// Protocol-level error after connection was established.
  ProtocolError,
  /// User-provided input failed validation.
  InvalidInput,
  /// General I/O error during read or write.
  IoError,
}

impl ErrorCode {
  /// Map this error category to a process exit code (1--4).
  ///
  /// | Code | Meaning |
  /// |------|---------|
  /// | 1 | Connection / DNS / I/O failure |
  /// | 2 | Protocol or TLS error |
  /// | 3 | Timeout |
  /// | 4 | Invalid user input |
  pub fn exit_code(self) -> i32 {
    match self {
      ErrorCode::ConnectionRefused | ErrorCode::DnsResolution | ErrorCode::IoError => 1,
      ErrorCode::ProtocolError | ErrorCode::TlsError => 2,
      ErrorCode::ConnectionTimeout => 3,
      ErrorCode::InvalidInput => 4,
    }
  }
}

/// Structured error carrying a categorized code, descriptive message, and protocol context.
///
/// [`NetError`] is the single error type threaded through every protocol handler. When the
/// process must terminate, [`NetError::exit`] serializes the error as a [`NetResponse`] and
/// sets the exit code derived from [`ErrorCode::exit_code`].
#[derive(Debug)]
pub struct NetError {
  /// Categorized error code indicating the failure class.
  pub code: ErrorCode,
  /// Human-readable description of what went wrong.
  pub message: String,
  /// The protocol context in which the error occurred.
  pub protocol: Protocol,
}

impl fmt::Display for NetError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}: {}", self.code, self.message)
  }
}

impl std::error::Error for NetError {}

impl NetError {
  /// Create a new error with the given code, message, and protocol context.
  pub fn new(code: ErrorCode, message: impl Into<String>, protocol: Protocol) -> Self {
    Self {
      code,
      message: message.into(),
      protocol,
    }
  }

  /// Print the error as structured JSON output and terminate the process.
  ///
  /// The error is wrapped in a [`NetResponse`] with [`ResponseType::Error`] so that
  /// consumers receive the same envelope shape as successful responses. The process
  /// exits with the code returned by [`ErrorCode::exit_code`].
  pub fn exit(&self, mode: OutputMode, no_color: bool) -> ! {
    let response = NetResponse::new(
      ResponseType::Error,
      self.protocol,
      serde_json::json!({
        "code": self.code,
        "message": self.message,
      }),
    );
    print_response(&response, mode, no_color);
    process::exit(self.code.exit_code());
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn exit_code_connection_errors() {
    assert_eq!(ErrorCode::ConnectionRefused.exit_code(), 1);
    assert_eq!(ErrorCode::DnsResolution.exit_code(), 1);
    assert_eq!(ErrorCode::IoError.exit_code(), 1);
  }

  #[test]
  fn exit_code_protocol_errors() {
    assert_eq!(ErrorCode::ProtocolError.exit_code(), 2);
    assert_eq!(ErrorCode::TlsError.exit_code(), 2);
  }

  #[test]
  fn exit_code_timeout() {
    assert_eq!(ErrorCode::ConnectionTimeout.exit_code(), 3);
  }

  #[test]
  fn exit_code_invalid_input() {
    assert_eq!(ErrorCode::InvalidInput.exit_code(), 4);
  }

  #[test]
  fn error_code_serializes_to_screaming_snake_case() {
    let cases = [
      (ErrorCode::ConnectionRefused, "CONNECTION_REFUSED"),
      (ErrorCode::ConnectionTimeout, "CONNECTION_TIMEOUT"),
      (ErrorCode::DnsResolution, "DNS_RESOLUTION"),
      (ErrorCode::TlsError, "TLS_ERROR"),
      (ErrorCode::ProtocolError, "PROTOCOL_ERROR"),
      (ErrorCode::InvalidInput, "INVALID_INPUT"),
      (ErrorCode::IoError, "IO_ERROR"),
    ];
    for (code, expected) in cases {
      let value = serde_json::to_value(code).unwrap();
      assert_eq!(
        value,
        json!(expected),
        "ErrorCode::{:?} should serialize to \"{}\"",
        code,
        expected
      );
    }
  }

  #[test]
  fn net_error_display_format() {
    let err = NetError::new(ErrorCode::ConnectionRefused, "connection was refused", Protocol::Http);
    let display = format!("{err}");
    assert!(
      display.contains("ConnectionRefused"),
      "display should contain debug variant name, got: {display}"
    );
    assert!(
      display.contains("connection was refused"),
      "display should contain message, got: {display}"
    );
  }

  #[test]
  fn net_error_new_constructs_correctly() {
    let err = NetError::new(ErrorCode::InvalidInput, "bad url", Protocol::Http);
    assert!(matches!(err.code, ErrorCode::InvalidInput));
    assert_eq!(err.message, "bad url");
    assert!(matches!(err.protocol, Protocol::Http));
  }
}
