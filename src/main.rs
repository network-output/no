//! Fast, structured networking CLI with multi-protocol support.
//!
//! `no` provides a unified interface for interacting with HTTP, WebSocket, TCP, MQTT, and SSE
//! endpoints from the command line. Every protocol handler emits structured output through a
//! consistent [`NetResponse`](output::NetResponse) JSON envelope, making results easy to parse,
//! pipe, and filter with the built-in jq engine.
//!
//! The architecture follows a simple pipeline: CLI parsing via clap derive macros
//! ([`Cli`]) dispatches to the appropriate protocol handler, which produces
//! [`NetResponse`](output::NetResponse) values rendered in the active [`OutputMode`].
//! All failures are surfaced through [`NetError`] with categorized
//! [`ErrorCode`] variants that map to deterministic process exit codes.

mod addr;
mod cli;
mod error;
mod output;
mod protocols;
mod url;

use clap::Parser;
use cli::{Cli, Command};
use error::{ErrorCode, NetError};
use output::OutputMode;

#[tokio::main]
async fn main() {
  let cli = Cli::parse();
  let mode = OutputMode::detect(cli.json, cli.pretty);
  let timeout = cli.timeout;
  let no_color = cli.no_color;

  if let Some(ref expr) = cli.jq {
    if let Err(msg) = output::init_jq_filter(expr) {
      NetError::new(ErrorCode::InvalidInput, msg, output::Protocol::Http).exit(mode, no_color);
    }
  }

  let count = cli.count;
  let verbose = cli.verbose;

  let result = match cli.command {
    Command::Http(args) => protocols::http::run(args, mode, no_color, timeout, verbose).await,
    Command::Ws { action } => protocols::ws::run(action, mode, no_color, timeout, count, verbose).await,
    Command::Tcp { action } => protocols::tcp::run(action, mode, no_color, timeout, count, verbose).await,
    Command::Mqtt { action } => protocols::mqtt::run(action, mode, no_color, timeout, count, verbose).await,
    Command::Udp { action } => protocols::udp::run(action, mode, no_color, timeout, count, verbose).await,
    Command::Sse(args) => protocols::sse::run(args, mode, no_color, timeout, count, verbose).await,
    Command::Dns(args) => protocols::dns::run(args, mode, no_color, timeout, verbose).await,
    Command::Ping(args) => protocols::ping::run(args, mode, no_color, timeout, count, verbose).await,
    Command::Whois(args) => protocols::whois::run(args, mode, no_color, timeout, verbose).await,
    Command::Jq(args) => protocols::jq::run(args).await,
    Command::Skills { action } => protocols::skills::run(action).await,
  };

  if let Err(e) = result {
    e.exit(mode, no_color);
  }
}
