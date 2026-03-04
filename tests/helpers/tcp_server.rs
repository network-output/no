use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const READ_BUFFER_SIZE: usize = 8192;

/// Interval between messages in the multi-message server.
const MESSAGE_INTERVAL_MS: u64 = 50;

/// How long the silent server waits before closing.
const SILENT_CLOSE_DELAY_MS: u64 = 100;

pub struct TcpTestServer {
  pub addr: SocketAddr,
  _handle: std::thread::JoinHandle<()>,
}

/// Start a TCP server that reads one message and echoes it back, then closes.
pub fn start_echo_server() -> TcpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
      tx.send(listener.local_addr().unwrap()).unwrap();

      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; READ_BUFFER_SIZE];
        if let Ok(n) = socket.read(&mut buf).await {
          if n > 0 {
            let _ = socket.write_all(&buf[..n]).await;
          }
        }
        let _ = socket.shutdown().await;
      }
    });
  });

  let addr = rx.recv().unwrap();
  TcpTestServer { addr, _handle: handle }
}

/// Start a TCP server that sends 3 messages (50ms apart), then closes.
pub fn start_multi_message_server() -> TcpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
      tx.send(listener.local_addr().unwrap()).unwrap();

      if let Ok((mut socket, _)) = listener.accept().await {
        for i in 1..=3 {
          let msg = format!("message {i}\n");
          if socket.write_all(msg.as_bytes()).await.is_err() {
            break;
          }
          tokio::time::sleep(std::time::Duration::from_millis(MESSAGE_INTERVAL_MS)).await;
        }
        let _ = socket.shutdown().await;
      }
    });
  });

  let addr = rx.recv().unwrap();
  TcpTestServer { addr, _handle: handle }
}

/// Start a TCP server that accepts a connection, waits 100ms, then closes
/// without sending any data. Tests the "connected then closed" lifecycle.
pub fn start_silent_server() -> TcpTestServer {
  let (tx, rx) = std::sync::mpsc::channel();

  let handle = std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
      let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
      tx.send(listener.local_addr().unwrap()).unwrap();

      if let Ok((mut socket, _)) = listener.accept().await {
        tokio::time::sleep(std::time::Duration::from_millis(SILENT_CLOSE_DELAY_MS)).await;
        let _ = socket.shutdown().await;
      }
    });
  });

  let addr = rx.recv().unwrap();
  TcpTestServer { addr, _handle: handle }
}
