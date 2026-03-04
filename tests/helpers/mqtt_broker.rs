use rumqttd::{Broker, Config, ConnectionSettings, RouterConfig, ServerSettings};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::LazyLock;

/// Embedded MQTT broker for integration tests.
///
/// Uses `LazyLock` to share a single broker instance across all MQTT tests,
/// avoiding repeated startup cost. The broker binds to a free port on 127.0.0.1.
///
/// Usage:
///   let addr = MQTT_BROKER.addr();   // "127.0.0.1:<port>"
///   no_cmd().args(["mqtt", "pub", "-b", &addr, "-t", "topic", "-m", "msg"])
pub struct MqttTestBroker {
  pub port: u16,
}

impl MqttTestBroker {
  /// Returns the broker address as "127.0.0.1:<port>".
  pub fn addr(&self) -> String {
    format!("127.0.0.1:{}", self.port)
  }

  fn start() -> Self {
    // Find a free port by binding to :0 and immediately closing.
    let port = {
      let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
      listener.local_addr().unwrap().port()
    };

    let listen_addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let mut v4_servers = HashMap::new();
    v4_servers.insert(
      "test".to_string(),
      ServerSettings {
        name: "test".to_string(),
        listen: listen_addr,
        tls: None,
        next_connection_delay_ms: 1,
        connections: ConnectionSettings {
          connection_timeout_ms: 60000,
          max_payload_size: 20480,
          max_inflight_count: 100,
          auth: None,
          external_auth: None,
          dynamic_filters: true,
        },
      },
    );

    // Values from rumqttd's default config (rumqttd.toml).
    // These are generous enough for test workloads.
    let config = Config {
      id: 0,
      router: RouterConfig {
        max_connections: 10010,
        max_outgoing_packet_count: 200,
        max_segment_size: 104857600, // 100 MB
        max_segment_count: 10,
        ..RouterConfig::default()
      },
      v4: Some(v4_servers),
      ..Config::default()
    };

    let mut broker = Broker::new(config);

    std::thread::spawn(move || {
      broker.start().expect("failed to start MQTT broker");
    });

    // Poll until the broker is accepting TCP connections.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
      if std::time::Instant::now() > deadline {
        panic!("MQTT broker failed to start within 5 seconds");
      }
      if std::net::TcpStream::connect_timeout(&listen_addr, std::time::Duration::from_millis(100)).is_ok() {
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(50));
    }

    MqttTestBroker { port }
  }
}

pub static MQTT_BROKER: LazyLock<MqttTestBroker> = LazyLock::new(MqttTestBroker::start);
