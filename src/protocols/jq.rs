//! Standalone jq filter that processes JSON from stdin.

use crate::cli::JqArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{Protocol, compile_filter, print_jq_value, run_jq_filter};

/// Read JSON from stdin and apply a jq filter expression.
///
/// Parses stdin as a JSON value and applies the user-provided jq expression using the embedded jaq
/// engine. Results are printed to stdout, one value per line.
///
/// # Errors
///
/// Returns [`NetError`] on empty stdin, invalid JSON input, or jq filter compilation/evaluation
/// errors.
pub async fn run(args: JqArgs) -> Result<(), NetError> {
  compile_filter(&args.filter).map_err(|e| {
    NetError::new(
      ErrorCode::InvalidInput,
      format!("invalid jq expression: {e}"),
      Protocol::Http,
    )
  })?;

  let input = read_stdin().await?;

  let value: serde_json::Value = serde_json::from_str(&input).map_err(|e| {
    NetError::new(
      ErrorCode::InvalidInput,
      format!("invalid JSON input: {e}"),
      Protocol::Http,
    )
  })?;

  let results = run_jq_filter(&args.filter, value);
  for result in results {
    match result {
      Ok(val) => print_jq_value(val),
      Err(e) => eprintln!("jq error: {e}"),
    }
  }

  Ok(())
}

async fn read_stdin() -> Result<String, NetError> {
  use tokio::io::AsyncReadExt;
  let mut buf = String::new();
  tokio::io::stdin()
    .read_to_string(&mut buf)
    .await
    .map_err(|e| NetError::new(ErrorCode::IoError, format!("failed to read stdin: {e}"), Protocol::Http))?;
  Ok(buf)
}
