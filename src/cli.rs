//! Command-line interface definition using clap derive macros.
//!
//! The [`Cli`] struct captures all global flags (output mode, timeout, jq filter, etc.)
//! while the [`Command`] enum dispatches to protocol-specific argument types. Each protocol
//! subcommand defines its own `Args` or `Action` type that holds only the options relevant
//! to that protocol.

use clap::{Parser, Subcommand};
use std::time::Duration;

/// Top-level CLI parser with global flags and protocol subcommands.
#[derive(Parser)]
#[command(
  name = "no",
  version,
  about = "The AI-first networking CLI",
  after_help = "\
EXIT CODES:
  0  Success
  1  Connection error (refused, DNS, I/O)
  2  Protocol error (TLS, unexpected response)
  3  Timeout (connection or read)
  4  Invalid input (bad URL, header, method, port)

ENVIRONMENT VARIABLES:
  NO_AUTH_TOKEN   Bearer token fallback for HTTP and SSE
  NO_BASIC_AUTH   Basic auth fallback (USER:PASS) for HTTP and SSE"
)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Command,

  #[arg(long, global = true, help = "Force JSON output")]
  pub json: bool,

  #[arg(long, global = true, help = "Force pretty-printed output")]
  pub pretty: bool,

  #[arg(long, global = true, value_name = "DURATION", value_parser = parse_duration, help = "Request timeout (e.g. 5s, 300ms, 1m)")]
  pub timeout: Option<Duration>,

  #[arg(long, global = true, help = "Disable colored output")]
  pub no_color: bool,

  #[arg(long, short, global = true, help = "Verbose output")]
  pub verbose: bool,

  #[arg(
    long,
    short = 'n',
    global = true,
    value_name = "N",
    help = "Stop after N data messages (streaming protocols)"
  )]
  pub count: Option<usize>,

  #[arg(
    long,
    global = true,
    value_name = "EXPR",
    help = "Filter output with a jq expression"
  )]
  pub jq: Option<String>,
}

/// Protocol subcommand dispatch.
#[derive(Subcommand)]
pub enum Command {
  /// Make HTTP requests.
  #[command(about = "Make HTTP requests")]
  Http(HttpArgs),

  /// WebSocket operations.
  #[command(about = "WebSocket operations")]
  Ws {
    #[command(subcommand)]
    action: WsAction,
  },

  /// Raw TCP connections.
  #[command(about = "Raw TCP connections")]
  Tcp {
    #[command(subcommand)]
    action: TcpAction,
  },

  /// MQTT publish/subscribe messaging.
  #[command(about = "MQTT pub/sub")]
  Mqtt {
    #[command(subcommand)]
    action: MqttAction,
  },

  /// UDP datagram operations.
  #[command(about = "UDP datagrams")]
  Udp {
    #[command(subcommand)]
    action: UdpAction,
  },

  /// Server-Sent Events streaming.
  #[command(about = "Server-Sent Events")]
  Sse(SseArgs),

  /// DNS lookup.
  #[command(about = "DNS lookup")]
  Dns(DnsArgs),

  /// Ping a host.
  #[command(about = "Ping a host")]
  Ping(PingArgs),

  /// WHOIS lookup.
  #[command(about = "WHOIS lookup")]
  Whois(WhoisArgs),

  /// Filter JSON from stdin with a jq expression.
  #[command(about = "Filter JSON from stdin with a jq expression")]
  Jq(JqArgs),
}

// -- Jq --

/// Arguments for standalone jq filtering from stdin.
#[derive(clap::Args)]
pub struct JqArgs {
  #[arg(help = "jq filter expression")]
  pub filter: String,
}

// -- HTTP --

/// Arguments for HTTP requests.
#[derive(clap::Args)]
pub struct HttpArgs {
  #[arg(help = "HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)")]
  pub method: String,

  #[arg(help = "Request URL")]
  pub url: String,

  #[arg(short = 'H', long = "header", value_name = "KEY:VALUE", help = "Add request header")]
  pub headers: Vec<String>,

  #[arg(short = 'b', long = "body", help = "Request body")]
  pub body: Option<String>,

  #[arg(long, help = "Bearer token for authentication")]
  pub bearer: Option<String>,

  #[arg(long, value_name = "USER:PASS", help = "Basic auth credentials")]
  pub basic: Option<String>,

  #[arg(
    short = 'o',
    long = "output",
    value_name = "FILE",
    help = "Save response body to file"
  )]
  pub output: Option<String>,

  #[arg(long, help = "Read request body from stdin")]
  pub stdin: bool,
}

// -- WebSocket --

/// WebSocket subcommand: listen for incoming frames or send a message.
#[derive(Subcommand)]
pub enum WsAction {
  #[command(about = "Listen for WebSocket messages")]
  Listen(WsListenArgs),

  #[command(about = "Send a WebSocket message")]
  Send(WsSendArgs),
}

/// Arguments for WebSocket listen mode.
#[derive(clap::Args)]
pub struct WsListenArgs {
  #[arg(help = "WebSocket URL")]
  pub url: String,
}

/// Arguments for WebSocket send mode.
#[derive(clap::Args)]
pub struct WsSendArgs {
  #[arg(help = "WebSocket URL")]
  pub url: String,

  #[arg(short = 'm', long = "message", help = "Message to send")]
  pub message: String,
}

// -- TCP --

/// TCP subcommand: connect to a remote host or listen for incoming connections.
#[derive(Subcommand)]
pub enum TcpAction {
  #[command(about = "Connect to a TCP server")]
  Connect(TcpConnectArgs),

  #[command(about = "Listen on a TCP port")]
  Listen(TcpListenArgs),
}

/// Arguments for TCP client connection.
#[derive(clap::Args)]
pub struct TcpConnectArgs {
  #[arg(help = "Target host:port")]
  pub address: String,

  #[arg(short = 'm', long = "message", help = "Message to send")]
  pub message: Option<String>,

  #[arg(long, help = "Read data from stdin")]
  pub stdin: bool,
}

/// Arguments for TCP server listener.
#[derive(clap::Args)]
pub struct TcpListenArgs {
  #[arg(help = "Address to listen on (e.g. :9090 or 0.0.0.0:9090)")]
  pub address: String,
}

// -- MQTT --

/// MQTT subcommand: subscribe to a topic or publish a message.
#[derive(Subcommand)]
pub enum MqttAction {
  #[command(about = "Subscribe to MQTT topic")]
  Sub(MqttSubArgs),

  #[command(about = "Publish to MQTT topic")]
  Pub(MqttPubArgs),
}

/// Arguments for MQTT topic subscription.
#[derive(clap::Args)]
pub struct MqttSubArgs {
  #[arg(help = "MQTT broker address (e.g. localhost:1883)")]
  pub broker: String,

  #[arg(short = 't', long = "topic", help = "Topic to subscribe to")]
  pub topic: String,
}

/// Arguments for MQTT message publishing.
#[derive(clap::Args)]
pub struct MqttPubArgs {
  #[arg(help = "MQTT broker address (e.g. localhost:1883)")]
  pub broker: String,

  #[arg(short = 't', long = "topic", help = "Topic to publish to")]
  pub topic: String,

  #[arg(short = 'm', long = "message", help = "Message to publish")]
  pub message: String,
}

// -- SSE --

/// Arguments for Server-Sent Events streaming.
#[derive(clap::Args)]
pub struct SseArgs {
  #[arg(help = "SSE endpoint URL")]
  pub url: String,

  #[arg(short = 'H', long = "header", value_name = "KEY:VALUE", help = "Add request header")]
  pub headers: Vec<String>,

  #[arg(long, help = "Bearer token for authentication")]
  pub bearer: Option<String>,

  #[arg(long, value_name = "USER:PASS", help = "Basic auth credentials")]
  pub basic: Option<String>,
}

// -- DNS --

/// Arguments for DNS lookups.
#[derive(clap::Args)]
pub struct DnsArgs {
  #[arg(help = "Domain name or IP address (auto-detects reverse lookup)")]
  pub name: String,

  #[arg(
    default_value = "A",
    help = "Record type (A, AAAA, MX, TXT, CNAME, NS, SOA, SRV, PTR)"
  )]
  pub record_type: String,

  #[arg(long, value_name = "ADDR", help = "DNS server to query (e.g. 8.8.8.8, 1.1.1.1:53)")]
  pub server: Option<String>,
}

// -- UDP --

/// UDP subcommand: send a datagram or listen for incoming datagrams.
#[derive(Subcommand)]
pub enum UdpAction {
  #[command(about = "Send a UDP datagram")]
  Send(UdpSendArgs),

  #[command(about = "Listen for UDP datagrams")]
  Listen(UdpListenArgs),
}

/// Arguments for sending a UDP datagram.
#[derive(clap::Args)]
pub struct UdpSendArgs {
  #[arg(help = "Target host:port")]
  pub address: String,

  #[arg(short = 'm', long = "message", help = "Message to send")]
  pub message: Option<String>,

  #[arg(long, help = "Read data from stdin")]
  pub stdin: bool,

  #[arg(long, value_name = "DURATION", num_args = 0..=1, default_missing_value = "0s", value_parser = parse_duration, help = "Wait for response (optionally with timeout, e.g. 3s)")]
  pub wait: Option<Option<Duration>>,
}

/// Arguments for listening on a UDP port.
#[derive(clap::Args)]
pub struct UdpListenArgs {
  #[arg(help = "Address to listen on (e.g. :9090 or 0.0.0.0:9090)")]
  pub address: String,
}

// -- Ping --

/// Arguments for ICMP ping.
#[derive(clap::Args)]
pub struct PingArgs {
  #[arg(help = "Host or IP address to ping")]
  pub host: String,

  #[arg(
    long,
    value_name = "DURATION",
    value_parser = parse_duration,
    default_value = "1s",
    help = "Interval between pings (e.g. 1s, 500ms)"
  )]
  pub interval: Duration,
}

// -- WHOIS --

/// Arguments for WHOIS lookups.
#[derive(clap::Args)]
pub struct WhoisArgs {
  #[arg(help = "Domain name or IP address to look up")]
  pub query: String,

  #[arg(long, value_name = "HOST", help = "WHOIS server to query (auto-detected by default)")]
  pub server: Option<String>,
}

fn parse_duration(s: &str) -> Result<Duration, String> {
  humantime::parse_duration(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use clap::Parser;

  #[test]
  fn http_basic_get() {
    let cli = Cli::try_parse_from(["no", "http", "GET", "https://example.com"]).unwrap();
    let Command::Http(args) = cli.command else {
      panic!("expected Http command")
    };
    assert_eq!(args.method, "GET");
    assert_eq!(args.url, "https://example.com");
  }

  #[test]
  fn http_post_with_options() {
    let cli = Cli::try_parse_from([
      "no",
      "http",
      "POST",
      "https://api.example.com",
      "-H",
      "Content-Type:application/json",
      "-b",
      "{}",
    ])
    .unwrap();
    let Command::Http(args) = cli.command else {
      panic!("expected Http command")
    };
    assert_eq!(args.headers, vec!["Content-Type:application/json"]);
    assert_eq!(args.body.as_deref(), Some("{}"));
    assert!(args.bearer.is_none());
  }

  #[test]
  fn http_with_bearer() {
    let cli = Cli::try_parse_from(["no", "http", "GET", "https://api.example.com", "--bearer", "tok123"]).unwrap();
    let Command::Http(args) = cli.command else {
      panic!("expected Http command")
    };
    assert_eq!(args.bearer.as_deref(), Some("tok123"));
  }

  #[test]
  fn ws_listen() {
    let cli = Cli::try_parse_from(["no", "ws", "listen", "ws://localhost:8080"]).unwrap();
    let Command::Ws { action } = cli.command else {
      panic!("expected Ws command")
    };
    let WsAction::Listen(args) = action else {
      panic!("expected Listen action")
    };
    assert_eq!(args.url, "ws://localhost:8080");
  }

  #[test]
  fn ws_send() {
    let cli = Cli::try_parse_from(["no", "ws", "send", "ws://localhost:8080", "-m", "hello"]).unwrap();
    let Command::Ws { action } = cli.command else {
      panic!("expected Ws command")
    };
    let WsAction::Send(args) = action else {
      panic!("expected Send action")
    };
    assert_eq!(args.message, "hello");
  }

  #[test]
  fn tcp_connect() {
    let cli = Cli::try_parse_from(["no", "tcp", "connect", "localhost:9090", "-m", "hello"]).unwrap();
    let Command::Tcp { action } = cli.command else {
      panic!("expected Tcp command")
    };
    let TcpAction::Connect(args) = action else {
      panic!("expected Connect action")
    };
    assert_eq!(args.address, "localhost:9090");
    assert_eq!(args.message.as_deref(), Some("hello"));
  }

  #[test]
  fn tcp_listen() {
    let cli = Cli::try_parse_from(["no", "tcp", "listen", ":9090"]).unwrap();
    let Command::Tcp { action } = cli.command else {
      panic!("expected Tcp command")
    };
    let TcpAction::Listen(args) = action else {
      panic!("expected Listen action")
    };
    assert_eq!(args.address, ":9090");
  }

  #[test]
  fn mqtt_sub() {
    let cli = Cli::try_parse_from(["no", "mqtt", "sub", "localhost:1883", "-t", "test/topic"]).unwrap();
    let Command::Mqtt { action } = cli.command else {
      panic!("expected Mqtt command")
    };
    let MqttAction::Sub(args) = action else {
      panic!("expected Sub action")
    };
    assert_eq!(args.broker, "localhost:1883");
    assert_eq!(args.topic, "test/topic");
  }

  #[test]
  fn mqtt_pub() {
    let cli = Cli::try_parse_from(["no", "mqtt", "pub", "localhost:1883", "-t", "test/topic", "-m", "hello"]).unwrap();
    let Command::Mqtt { action } = cli.command else {
      panic!("expected Mqtt command")
    };
    let MqttAction::Pub(args) = action else {
      panic!("expected Pub action")
    };
    assert_eq!(args.message, "hello");
  }

  #[test]
  fn sse_basic() {
    let cli = Cli::try_parse_from(["no", "sse", "https://example.com/events"]).unwrap();
    let Command::Sse(args) = cli.command else {
      panic!("expected Sse command")
    };
    assert_eq!(args.url, "https://example.com/events");
  }

  #[test]
  fn global_json_flag() {
    let cli = Cli::try_parse_from(["no", "--json", "http", "GET", "https://example.com"]).unwrap();
    assert!(cli.json);
  }

  #[test]
  fn global_pretty_flag() {
    let cli = Cli::try_parse_from(["no", "--pretty", "http", "GET", "https://example.com"]).unwrap();
    assert!(cli.pretty);
  }

  #[test]
  fn global_timeout() {
    let cli = Cli::try_parse_from(["no", "--timeout", "5s", "http", "GET", "https://example.com"]).unwrap();
    assert_eq!(cli.timeout, Some(Duration::from_secs(5)));
  }

  #[test]
  fn global_timeout_millis() {
    let cli = Cli::try_parse_from(["no", "--timeout", "300ms", "http", "GET", "https://example.com"]).unwrap();
    assert_eq!(cli.timeout, Some(Duration::from_millis(300)));
  }

  #[test]
  fn global_timeout_minutes() {
    let cli = Cli::try_parse_from(["no", "--timeout", "1m", "http", "GET", "https://example.com"]).unwrap();
    assert_eq!(cli.timeout, Some(Duration::from_secs(60)));
  }

  #[test]
  fn global_no_color() {
    let cli = Cli::try_parse_from(["no", "--no-color", "http", "GET", "https://example.com"]).unwrap();
    assert!(cli.no_color);
  }

  #[test]
  fn global_verbose() {
    let cli = Cli::try_parse_from(["no", "-v", "http", "GET", "https://example.com"]).unwrap();
    assert!(cli.verbose);
  }

  #[test]
  fn global_count() {
    let cli = Cli::try_parse_from(["no", "--count", "5", "ws", "listen", "ws://localhost:8080"]).unwrap();
    assert_eq!(cli.count, Some(5));
  }

  #[test]
  fn global_count_short() {
    let cli = Cli::try_parse_from(["no", "-n", "3", "ws", "listen", "ws://localhost:8080"]).unwrap();
    assert_eq!(cli.count, Some(3));
  }

  #[test]
  fn global_jq_filter() {
    let cli = Cli::try_parse_from(["no", "--jq", ".data.status", "http", "GET", "https://example.com"]).unwrap();
    assert_eq!(cli.jq.as_deref(), Some(".data.status"));
  }

  #[test]
  fn jq_subcommand() {
    let cli = Cli::try_parse_from(["no", "jq", ".data"]).unwrap();
    let Command::Jq(args) = cli.command else {
      panic!("expected Jq command")
    };
    assert_eq!(args.filter, ".data");
  }

  #[test]
  fn udp_send() {
    let cli = Cli::try_parse_from(["no", "udp", "send", "127.0.0.1:9090", "-m", "hello"]).unwrap();
    let Command::Udp { action } = cli.command else {
      panic!("expected Udp command")
    };
    let UdpAction::Send(args) = action else {
      panic!("expected Send action")
    };
    assert_eq!(args.address, "127.0.0.1:9090");
    assert_eq!(args.message.as_deref(), Some("hello"));
  }

  #[test]
  fn udp_send_with_wait() {
    let cli = Cli::try_parse_from(["no", "udp", "send", "127.0.0.1:9090", "-m", "hello", "--wait"]).unwrap();
    let Command::Udp { action } = cli.command else {
      panic!("expected Udp command")
    };
    let UdpAction::Send(args) = action else {
      panic!("expected Send action")
    };
    // Bare --wait uses default_missing_value "0s" -> Duration::ZERO
    assert_eq!(args.wait, Some(Some(Duration::ZERO)));
  }

  #[test]
  fn udp_send_with_wait_duration() {
    let cli = Cli::try_parse_from(["no", "udp", "send", "127.0.0.1:9090", "-m", "hello", "--wait", "3s"]).unwrap();
    let Command::Udp { action } = cli.command else {
      panic!("expected Udp command")
    };
    let UdpAction::Send(args) = action else {
      panic!("expected Send action")
    };
    assert_eq!(args.wait, Some(Some(Duration::from_secs(3))));
  }

  #[test]
  fn udp_listen() {
    let cli = Cli::try_parse_from(["no", "udp", "listen", ":9090"]).unwrap();
    let Command::Udp { action } = cli.command else {
      panic!("expected Udp command")
    };
    let UdpAction::Listen(args) = action else {
      panic!("expected Listen action")
    };
    assert_eq!(args.address, ":9090");
  }

  #[test]
  fn dns_basic() {
    let cli = Cli::try_parse_from(["no", "dns", "example.com"]).unwrap();
    let Command::Dns(args) = cli.command else {
      panic!("expected Dns command")
    };
    assert_eq!(args.name, "example.com");
    assert_eq!(args.record_type, "A");
    assert!(args.server.is_none());
  }

  #[test]
  fn dns_record_type() {
    let cli = Cli::try_parse_from(["no", "dns", "example.com", "AAAA"]).unwrap();
    let Command::Dns(args) = cli.command else {
      panic!("expected Dns command")
    };
    assert_eq!(args.name, "example.com");
    assert_eq!(args.record_type, "AAAA");
  }

  #[test]
  fn dns_with_server() {
    let cli = Cli::try_parse_from(["no", "dns", "example.com", "--server", "8.8.8.8"]).unwrap();
    let Command::Dns(args) = cli.command else {
      panic!("expected Dns command")
    };
    assert_eq!(args.server.as_deref(), Some("8.8.8.8"));
  }

  #[test]
  fn parse_duration_seconds() {
    assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
  }

  #[test]
  fn parse_duration_milliseconds() {
    assert_eq!(parse_duration("300ms").unwrap(), Duration::from_millis(300));
  }

  #[test]
  fn parse_duration_invalid() {
    assert!(parse_duration("notaduration").is_err());
  }

  #[test]
  fn ping_basic() {
    let cli = Cli::try_parse_from(["no", "ping", "127.0.0.1"]).unwrap();
    let Command::Ping(args) = cli.command else {
      panic!("expected Ping command")
    };
    assert_eq!(args.host, "127.0.0.1");
    assert_eq!(args.interval, Duration::from_secs(1));
  }

  #[test]
  fn ping_with_interval() {
    let cli = Cli::try_parse_from(["no", "ping", "example.com", "--interval", "500ms"]).unwrap();
    let Command::Ping(args) = cli.command else {
      panic!("expected Ping command")
    };
    assert_eq!(args.host, "example.com");
    assert_eq!(args.interval, Duration::from_millis(500));
  }

  #[test]
  fn ping_with_count() {
    let cli = Cli::try_parse_from(["no", "-n", "2", "ping", "127.0.0.1"]).unwrap();
    let Command::Ping(_) = cli.command else {
      panic!("expected Ping command")
    };
    assert_eq!(cli.count, Some(2));
  }

  #[test]
  fn whois_basic() {
    let cli = Cli::try_parse_from(["no", "whois", "example.com"]).unwrap();
    let Command::Whois(args) = cli.command else {
      panic!("expected Whois command")
    };
    assert_eq!(args.query, "example.com");
    assert!(args.server.is_none());
  }

  #[test]
  fn whois_with_server() {
    let cli = Cli::try_parse_from(["no", "whois", "example.com", "--server", "whois.verisign-grs.com"]).unwrap();
    let Command::Whois(args) = cli.command else {
      panic!("expected Whois command")
    };
    assert_eq!(args.query, "example.com");
    assert_eq!(args.server.as_deref(), Some("whois.verisign-grs.com"));
  }

  #[test]
  fn whois_ip_query() {
    let cli = Cli::try_parse_from(["no", "whois", "8.8.8.8"]).unwrap();
    let Command::Whois(args) = cli.command else {
      panic!("expected Whois command")
    };
    assert_eq!(args.query, "8.8.8.8");
  }

  #[test]
  fn missing_required_args_fails() {
    let result = Cli::try_parse_from(["no", "http"]);
    assert!(result.is_err());
  }
}
