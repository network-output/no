use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Delay for the /slow endpoint. Tests should use a --timeout
/// smaller than this to trigger a timeout error (exit code 3).
const SLOW_ENDPOINT_DELAY_SECS: u64 = 2;

/// Interval between streamed messages (WS multi, SSE events).
const STREAM_INTERVAL_MS: u64 = 50;

/// Local test server for HTTP, WebSocket, and SSE.
///
/// Spawns an axum server on a dedicated thread, bound to port 0.
/// The server lives as long as the `TestServer` value is alive.
///
/// # HTTP routes
///
/// | Route              | Behavior                                         |
/// |--------------------|--------------------------------------------------|
/// | `GET /get`         | Echo request headers as JSON `{ method, headers }`|
/// | `POST /post`       | Echo request body as JSON `{ method, body }`     |
/// | `GET /auth`        | 200 with Bearer token present, 401 without       |
/// | `GET /slow`        | Sleep 2s then respond (use --timeout < 2000)     |
/// | `GET /download`    | Return binary body with Content-Length            |
/// | `GET /status/{code}` | Return specified HTTP status code              |
///
/// # WebSocket routes
///
/// | Route         | Behavior                                    |
/// |---------------|---------------------------------------------|
/// | `/ws/echo`    | Echo every text message back                |
/// | `/ws/close`   | Accept, immediately close with reason       |
/// | `/ws/binary`  | Send 4-byte binary frame, then close        |
/// | `/ws/multi`   | Send 3 JSON text messages (50ms apart), close|
///
/// # SSE routes
///
/// | Route           | Behavior                                     |
/// |-----------------|----------------------------------------------|
/// | `/events`       | Send 3 JSON events (50ms apart), then end    |
/// | `/events/auth`  | Same but require Bearer token (401 without)  |
/// | `/events/named` | Events with `event:` name and `id:` fields   |
pub struct TestServer {
  pub addr: SocketAddr,
  _handle: std::thread::JoinHandle<()>,
}

impl TestServer {
  pub fn start() -> Self {
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
      let rt = tokio::runtime::Runtime::new().unwrap();
      rt.block_on(async {
        let app = Router::new()
          .route("/get", get(handle_get))
          .route("/post", post(handle_post))
          .route("/auth", get(handle_auth))
          .route("/slow", get(handle_slow))
          .route("/download", get(handle_download))
          .route("/status/{code}", get(handle_status))
          .route("/ws/echo", get(ws_echo))
          .route("/ws/close", get(ws_close))
          .route("/ws/binary", get(ws_binary))
          .route("/ws/multi", get(ws_multi))
          .route("/events", get(sse_events))
          .route("/events/auth", get(sse_events_auth))
          .route("/events/named", get(sse_named_events));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tx.send(addr).unwrap();
        axum::serve(listener, app).await.unwrap();
      });
    });

    let addr = rx.recv().unwrap();
    Self { addr, _handle: handle }
  }

  pub fn http_url(&self, path: &str) -> String {
    format!("http://{}{}", self.addr, path)
  }

  pub fn ws_url(&self, path: &str) -> String {
    format!("ws://{}{}", self.addr, path)
  }
}

// -- HTTP handlers --

async fn handle_get(headers: HeaderMap) -> impl IntoResponse {
  let header_map: serde_json::Map<String, serde_json::Value> = headers
    .iter()
    .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap_or(""))))
    .collect();
  axum::Json(json!({
    "method": "GET",
    "headers": header_map,
  }))
}

async fn handle_post(body: String) -> impl IntoResponse {
  axum::Json(json!({
    "method": "POST",
    "body": body,
  }))
}

async fn handle_auth(headers: HeaderMap) -> impl IntoResponse {
  if let Some(auth) = headers.get("authorization") {
    let auth_str = auth.to_str().unwrap_or("");
    if let Some(token) = auth_str.strip_prefix("Bearer ") {
      return (
        StatusCode::OK,
        axum::Json(json!({ "authenticated": true, "token": token })),
      );
    }
  }
  (
    StatusCode::UNAUTHORIZED,
    axum::Json(json!({ "authenticated": false, "error": "missing or invalid bearer token" })),
  )
}

async fn handle_slow() -> impl IntoResponse {
  tokio::time::sleep(std::time::Duration::from_secs(SLOW_ENDPOINT_DELAY_SECS)).await;
  axum::Json(json!({ "status": "done" }))
}

async fn handle_download() -> Response {
  let content = b"test download content here";
  Response::builder()
    .header("content-type", "application/octet-stream")
    .header("content-length", content.len().to_string())
    .body(Body::from(content.to_vec()))
    .unwrap()
}

async fn handle_status(Path(code): Path<u16>) -> impl IntoResponse {
  let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
  (status, axum::Json(json!({ "status": code })))
}

// -- WebSocket handlers --

async fn ws_echo(ws: WebSocketUpgrade) -> Response {
  ws.on_upgrade(handle_ws_echo)
}

async fn handle_ws_echo(mut socket: WebSocket) {
  while let Some(Ok(msg)) = socket.next().await {
    match msg {
      Message::Text(text) => {
        if socket.send(Message::Text(text)).await.is_err() {
          break;
        }
      }
      Message::Close(_) => break,
      _ => {}
    }
  }
}

async fn ws_close(ws: WebSocketUpgrade) -> Response {
  ws.on_upgrade(handle_ws_close)
}

async fn handle_ws_close(mut socket: WebSocket) {
  let _ = socket
    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
      code: 1000,
      reason: "server closing".into(),
    })))
    .await;
}

async fn ws_binary(ws: WebSocketUpgrade) -> Response {
  ws.on_upgrade(handle_ws_binary)
}

async fn handle_ws_binary(mut socket: WebSocket) {
  let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
  let _ = socket.send(Message::Binary(data.into())).await;
  let _ = socket
    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
      code: 1000,
      reason: "done".into(),
    })))
    .await;
}

async fn ws_multi(ws: WebSocketUpgrade) -> Response {
  ws.on_upgrade(handle_ws_multi)
}

async fn handle_ws_multi(mut socket: WebSocket) {
  for i in 1..=3 {
    let msg = json!({ "seq": i, "data": format!("message {i}") }).to_string();
    if socket.send(Message::Text(msg.into())).await.is_err() {
      return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(STREAM_INTERVAL_MS)).await;
  }
  let _ = socket
    .send(Message::Close(Some(axum::extract::ws::CloseFrame {
      code: 1000,
      reason: "done".into(),
    })))
    .await;
}

// -- SSE handlers --

async fn sse_events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
  let stream = futures_util::stream::iter(1..=3).then(|i| async move {
    tokio::time::sleep(std::time::Duration::from_millis(STREAM_INTERVAL_MS)).await;
    Ok(Event::default().data(json!({ "seq": i }).to_string()))
  });
  Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn sse_events_auth(
  headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
  if let Some(auth) = headers.get("authorization") {
    let auth_str = auth.to_str().unwrap_or("");
    if auth_str.starts_with("Bearer ") {
      let stream = futures_util::stream::iter(1..=3).then(|i| async move {
        tokio::time::sleep(std::time::Duration::from_millis(STREAM_INTERVAL_MS)).await;
        Ok(Event::default().data(json!({ "seq": i, "auth": true }).to_string()))
      });
      return Ok(Sse::new(stream).keep_alive(KeepAlive::default()));
    }
  }
  Err((StatusCode::UNAUTHORIZED, "unauthorized".to_string()))
}

async fn sse_named_events() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
  let stream = futures_util::stream::iter(1..=3).then(|i| async move {
    tokio::time::sleep(std::time::Duration::from_millis(STREAM_INTERVAL_MS)).await;
    Ok(
      Event::default()
        .event(format!("update-{i}"))
        .id(i.to_string())
        .data(json!({ "seq": i }).to_string()),
    )
  });
  Sse::new(stream).keep_alive(KeepAlive::default())
}
