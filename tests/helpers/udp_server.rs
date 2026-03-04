use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// Interval between messages in the multi-message server.
const MESSAGE_INTERVAL_MS: u64 = 50;

pub struct UdpTestServer {
  pub addr: SocketAddr,
  _handle: std::thread::JoinHandle<()>,
}

/// Start a UDP server that receives one datagram and echoes it back, then exits.
pub fn start_echo_server() -> UdpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
      tx.send(socket.local_addr().unwrap()).unwrap();

      let mut buf = vec![0u8; 65535];
      if let Ok((n, peer)) = socket.recv_from(&mut buf).await {
        let _ = socket.send_to(&buf[..n], peer).await;
      }
    });
  });

  let addr = rx.recv().unwrap();
  UdpTestServer { addr, _handle: handle }
}

/// Start a UDP server that receives one datagram, then sends 3 replies at 50ms intervals.
pub fn start_multi_message_server() -> UdpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
      tx.send(socket.local_addr().unwrap()).unwrap();

      let mut buf = vec![0u8; 65535];
      if let Ok((_n, peer)) = socket.recv_from(&mut buf).await {
        for i in 1..=3 {
          let msg = format!("reply {i}");
          if socket.send_to(msg.as_bytes(), peer).await.is_err() {
            break;
          }
          tokio::time::sleep(std::time::Duration::from_millis(MESSAGE_INTERVAL_MS)).await;
        }
      }
    });
  });

  let addr = rx.recv().unwrap();
  UdpTestServer { addr, _handle: handle }
}

/// Start a UDP server that binds but never responds.
pub fn start_silent_server() -> UdpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
      tx.send(socket.local_addr().unwrap()).unwrap();

      // Keep socket alive for 5s then exit
      tokio::time::sleep(std::time::Duration::from_secs(5)).await;
      drop(socket);
    });
  });

  let addr = rx.recv().unwrap();
  UdpTestServer { addr, _handle: handle }
}
