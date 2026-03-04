//! Protocol handler implementations for the `no` CLI.
//!
//! Each module exposes a `run()` function as its public entry point. Handlers produce structured
//! [`NetResponse`] output to stdout, formatted according to the active [`OutputMode`].
//!
//! [`NetResponse`]: crate::output::NetResponse
//! [`OutputMode`]: crate::output::OutputMode

pub mod dns;
pub mod http;
pub mod jq;
pub mod mqtt;
pub mod ping;
pub mod sse;
pub mod tcp;
pub mod udp;
pub mod whois;
pub mod ws;
