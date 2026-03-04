//! MQTT client handler for publishing messages and subscribing to topics.

use crate::cli::MqttAction;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use rumqttc::{AsyncClient, Event, MqttOptions, NetworkOptions, Packet, QoS};
use serde_json::json;
use std::time::Duration;

/// Dispatch MQTT subscribe or publish operations.
///
/// Connects to an MQTT broker with automatic address parsing (supports `mqtt://host:port` and
/// `host:port` formats). In subscribe mode, streams incoming messages on a topic. In publish mode,
/// sends a single message and confirms delivery.
///
/// # Errors
///
/// Returns [`NetError`] on broker connection failure, invalid broker address, subscription errors,
/// or publish failures.
pub async fn run(
  action: MqttAction,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  match action {
    MqttAction::Sub(args) => subscribe(&args.broker, &args.topic, mode, no_color, timeout, count, verbose).await,
    MqttAction::Pub(args) => publish(&args.broker, &args.topic, &args.message, mode, no_color, timeout).await,
  }
}

fn parse_broker(broker: &str) -> Result<(String, u16), NetError> {
  let url = broker.strip_prefix("mqtt://").unwrap_or(broker);

  // Bracketed IPv6: [::1]:1883 or [::1]
  if let Some(rest) = url.strip_prefix('[') {
    if let Some((host, port_str)) = rest.split_once("]:") {
      let port = port_str
        .parse::<u16>()
        .map_err(|_| NetError::new(ErrorCode::InvalidInput, format!("invalid port: {port_str}"), Protocol::Mqtt))?;
      return Ok((host.to_string(), port));
    }
    let host = rest.trim_end_matches(']');
    return Ok((host.to_string(), 1883));
  }

  // IPv4 or hostname
  if let Some((h, p)) = url.rsplit_once(':') {
    let port = p
      .parse::<u16>()
      .map_err(|_| NetError::new(ErrorCode::InvalidInput, format!("invalid port: {p}"), Protocol::Mqtt))?;
    Ok((h.to_string(), port))
  } else {
    Ok((url.to_string(), 1883))
  }
}

fn map_mqtt_error(e: rumqttc::ConnectionError) -> NetError {
  use rumqttc::ConnectionError;

  match e {
    ConnectionError::NetworkTimeout | ConnectionError::FlushTimeout => {
      NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Mqtt)
    }
    ConnectionError::ConnectionRefused(_) => NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Mqtt),
    ConnectionError::Io(ref io_err) => match io_err.kind() {
      std::io::ErrorKind::ConnectionRefused => {
        NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Mqtt)
      }
      std::io::ErrorKind::TimedOut => NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Mqtt),
      _ => NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Mqtt),
    },
    _ => NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Mqtt),
  }
}

fn build_network_options(timeout: Option<Duration>) -> NetworkOptions {
  let mut net_opts = NetworkOptions::new();
  if let Some(dur) = timeout {
    let secs = dur.as_secs().max(1);
    net_opts.set_connection_timeout(secs);
  }
  net_opts
}

async fn subscribe(
  broker: &str,
  topic: &str,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  count: Option<usize>,
  verbose: bool,
) -> Result<(), NetError> {
  let (host, port) = parse_broker(broker)?;
  let client_id = format!("no-mqtt-{}", std::process::id());

  let mut opts = MqttOptions::new(&client_id, &host, port);
  opts.set_keep_alive(Duration::from_secs(30));

  let (client, mut eventloop) = AsyncClient::new(opts, 10);
  eventloop.set_network_options(build_network_options(timeout));

  client
    .subscribe(topic, QoS::AtLeastOnce)
    .await
    .map_err(|e| NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Mqtt))?;

  let connected = NetResponse::new(
    ResponseType::Connection,
    Protocol::Mqtt,
    json!({ "status": "subscribed", "broker": broker, "topic": topic }),
  );
  if verbose {
    print_response(
      &connected.with_metadata(json!({ "broker": broker, "topic": topic })),
      mode,
      no_color,
    );
  } else {
    print_response(&connected, mode, no_color);
  }

  let mut message_count: usize = 0;

  loop {
    let event = eventloop.poll().await.map_err(map_mqtt_error)?;

    if let Event::Incoming(Packet::Publish(publish)) = event {
      message_count += 1;
      let payload_str = String::from_utf8_lossy(&publish.payload);
      let payload: serde_json::Value =
        serde_json::from_str(&payload_str).unwrap_or_else(|_| json!(payload_str.to_string()));

      let qos_num = match publish.qos {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce => 1,
        QoS::ExactlyOnce => 2,
      };

      let response = NetResponse::new(
        ResponseType::Message,
        Protocol::Mqtt,
        json!({
          "topic": publish.topic,
          "payload": payload,
          "qos": qos_num,
        }),
      );
      if verbose {
        print_response(
          &response.with_metadata(json!({ "broker": broker, "qos": qos_num })),
          mode,
          no_color,
        );
      } else {
        print_response(&response, mode, no_color);
      }

      if count.is_some_and(|n| message_count >= n) {
        client.disconnect().await.ok();
        break;
      }
    }
  }

  Ok(())
}

async fn publish(
  broker: &str,
  topic: &str,
  message: &str,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
) -> Result<(), NetError> {
  let (host, port) = parse_broker(broker)?;
  let client_id = format!("no-mqtt-{}", std::process::id());

  let mut opts = MqttOptions::new(&client_id, &host, port);
  opts.set_keep_alive(Duration::from_secs(30));

  let (client, mut eventloop) = AsyncClient::new(opts, 10);
  eventloop.set_network_options(build_network_options(timeout));

  let timeout_dur = timeout.unwrap_or(Duration::from_secs(5));

  // Wait for ConnAck before publishing
  let connack_result = tokio::time::timeout(timeout_dur, async {
    loop {
      let event = eventloop.poll().await.map_err(map_mqtt_error)?;
      if let Event::Incoming(Packet::ConnAck(_)) = event {
        return Ok::<(), NetError>(());
      }
    }
  })
  .await;

  match connack_result {
    Err(_) => {
      return Err(NetError::new(
        ErrorCode::ConnectionTimeout,
        "MQTT connection timed out waiting for ConnAck",
        Protocol::Mqtt,
      ));
    }
    Ok(Err(e)) => return Err(e),
    Ok(Ok(())) => {}
  }

  client
    .publish(topic, QoS::AtLeastOnce, false, message.as_bytes())
    .await
    .map_err(|e| NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Mqtt))?;

  // Wait for PubAck to confirm delivery
  let puback_result = tokio::time::timeout(timeout_dur, async {
    loop {
      let event = eventloop.poll().await.map_err(map_mqtt_error)?;
      if let Event::Incoming(Packet::PubAck(_)) = event {
        return Ok::<(), NetError>(());
      }
    }
  })
  .await;

  match puback_result {
    Err(_) => {
      return Err(NetError::new(
        ErrorCode::ConnectionTimeout,
        "MQTT publish timed out waiting for PubAck",
        Protocol::Mqtt,
      ));
    }
    Ok(Err(e)) => return Err(e),
    Ok(Ok(())) => {}
  }

  let response = NetResponse::new(
    ResponseType::Response,
    Protocol::Mqtt,
    json!({
      "status": "published",
      "broker": broker,
      "topic": topic,
      "payload": message,
    }),
  );
  print_response(&response, mode, no_color);

  client.disconnect().await.ok();

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_broker_with_scheme_and_port() {
    let (host, port) = parse_broker("mqtt://broker.example.com:1884").unwrap();
    assert_eq!(host, "broker.example.com");
    assert_eq!(port, 1884);
  }

  #[test]
  fn parse_broker_without_scheme() {
    let (host, port) = parse_broker("broker.example.com:1884").unwrap();
    assert_eq!(host, "broker.example.com");
    assert_eq!(port, 1884);
  }

  #[test]
  fn parse_broker_default_port() {
    let (host, port) = parse_broker("broker.example.com").unwrap();
    assert_eq!(host, "broker.example.com");
    assert_eq!(port, 1883);
  }

  #[test]
  fn parse_broker_with_scheme_default_port() {
    let (host, port) = parse_broker("mqtt://localhost").unwrap();
    assert_eq!(host, "localhost");
    assert_eq!(port, 1883);
  }

  #[test]
  fn parse_broker_localhost_with_port() {
    let (host, port) = parse_broker("localhost:1883").unwrap();
    assert_eq!(host, "localhost");
    assert_eq!(port, 1883);
  }

  #[test]
  fn parse_broker_invalid_port() {
    let result = parse_broker("localhost:notaport");
    assert!(result.is_err());
  }

  #[test]
  fn parse_broker_ipv6_with_port() {
    let (host, port) = parse_broker("[::1]:1883").unwrap();
    assert_eq!(host, "::1");
    assert_eq!(port, 1883);
  }

  #[test]
  fn parse_broker_ipv6_default_port() {
    let (host, port) = parse_broker("[::1]").unwrap();
    assert_eq!(host, "::1");
    assert_eq!(port, 1883);
  }

  #[test]
  fn parse_broker_ipv6_with_scheme() {
    let (host, port) = parse_broker("mqtt://[::1]:1884").unwrap();
    assert_eq!(host, "::1");
    assert_eq!(port, 1884);
  }

  #[test]
  fn parse_broker_ipv6_scheme_default_port() {
    let (host, port) = parse_broker("mqtt://[::1]").unwrap();
    assert_eq!(host, "::1");
    assert_eq!(port, 1883);
  }

  #[test]
  fn map_mqtt_error_network_timeout() {
    let err = map_mqtt_error(rumqttc::ConnectionError::NetworkTimeout);
    assert!(matches!(err.code, ErrorCode::ConnectionTimeout));
  }

  #[test]
  fn map_mqtt_error_flush_timeout() {
    let err = map_mqtt_error(rumqttc::ConnectionError::FlushTimeout);
    assert!(matches!(err.code, ErrorCode::ConnectionTimeout));
  }

  #[test]
  fn map_mqtt_error_io_connection_refused() {
    let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
    let err = map_mqtt_error(rumqttc::ConnectionError::Io(io_err));
    assert!(matches!(err.code, ErrorCode::ConnectionRefused));
  }

  #[test]
  fn map_mqtt_error_io_timed_out() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
    let err = map_mqtt_error(rumqttc::ConnectionError::Io(io_err));
    assert!(matches!(err.code, ErrorCode::ConnectionTimeout));
  }
}
