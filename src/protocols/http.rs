//! HTTP request handler supporting all standard methods, headers, body, auth, and file downloads.

use crate::cli::HttpArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};
use crate::url::{UrlScheme, normalize_url};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;
use std::io::IsTerminal;
use std::str::FromStr;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Execute an HTTP request and print the response as structured output.
///
/// Supports GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS. Handles bearer and basic
/// authentication, custom headers, request bodies (including stdin piping), and file downloads
/// with progress indication.
///
/// # Errors
///
/// Returns [`NetError`] on connection failure, DNS resolution errors, TLS errors, timeout, or
/// invalid input (malformed URL, unreadable stdin).
pub async fn run(
  args: HttpArgs,
  mode: OutputMode,
  no_color: bool,
  timeout: Option<Duration>,
  verbose: bool,
) -> Result<(), NetError> {
  let url = normalize_url(&args.url, UrlScheme::Http);

  let method = reqwest::Method::from_str(&args.method.to_uppercase()).map_err(|_| {
    NetError::new(
      ErrorCode::InvalidInput,
      format!("invalid HTTP method: {}", args.method),
      Protocol::Http,
    )
  })?;

  let mut client_builder = reqwest::Client::builder();
  if let Some(dur) = timeout {
    client_builder = client_builder.timeout(dur);
  }
  let client = client_builder
    .build()
    .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Http))?;

  let mut request = client.request(method, &url);

  // Headers
  let mut headers = HeaderMap::new();
  for h in &args.headers {
    let (key, value) = h.split_once(':').ok_or_else(|| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header format: {h}"),
        Protocol::Http,
      )
    })?;
    let name = HeaderName::from_str(key.trim()).map_err(|_| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header name: {key}"),
        Protocol::Http,
      )
    })?;
    let val = HeaderValue::from_str(value.trim()).map_err(|_| {
      NetError::new(
        ErrorCode::InvalidInput,
        format!("invalid header value: {value}"),
        Protocol::Http,
      )
    })?;
    headers.insert(name, val);
  }
  request = request.headers(headers);

  // Auth
  let bearer = args.bearer.or_else(|| std::env::var("NO_AUTH_TOKEN").ok());
  if let Some(token) = bearer {
    request = request.bearer_auth(token);
  } else {
    let basic = args.basic.or_else(|| std::env::var("NO_BASIC_AUTH").ok());
    if let Some(creds) = basic {
      let (user, pass) = creds.split_once(':').unwrap_or((&creds, ""));
      request = request.basic_auth(user, Some(pass));
    }
  }

  // Body
  if args.stdin {
    let input = read_stdin().await?;
    request = request.body(input);
  } else if let Some(body) = args.body {
    request = request.body(body);
  }

  let response = request.send().await.map_err(map_reqwest_error)?;

  let status = response.status().as_u16();
  let response_headers: serde_json::Map<String, serde_json::Value> = response
    .headers()
    .iter()
    .map(|(k, v)| (k.to_string(), json!(v.to_str().unwrap_or(""))))
    .collect();

  // File download
  if let Some(output_path) = args.output {
    let content_length = response.content_length();
    let is_tty = std::io::stderr().is_terminal();

    let pb = if is_tty && !no_color {
      let pb = if let Some(len) = content_length {
        let pb = ProgressBar::new(len);
        pb.set_style(
          ProgressStyle::default_bar()
            .template("{spinner} [{bar:40}] {bytes}/{total_bytes} ({eta})")
            .expect("valid progress bar template")
            .progress_chars("=> "),
        );
        pb
      } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
          ProgressStyle::default_spinner()
            .template("{spinner} {bytes} downloaded")
            .expect("valid progress bar template"),
        );
        pb
      };
      Some(pb)
    } else {
      None
    };

    let mut file = tokio::fs::File::create(&output_path).await.map_err(|e| {
      NetError::new(
        ErrorCode::IoError,
        format!("cannot create file {output_path}: {e}"),
        Protocol::Http,
      )
    })?;

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
      let chunk =
        chunk.map_err(|e| NetError::new(ErrorCode::IoError, format!("download error: {e}"), Protocol::Http))?;
      file
        .write_all(&chunk)
        .await
        .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Http))?;
      downloaded += chunk.len() as u64;
      if let Some(ref pb) = pb {
        pb.set_position(downloaded);
      }
    }

    if let Some(pb) = pb {
      pb.finish_and_clear();
    }

    let net_response = NetResponse::new(
      ResponseType::Response,
      Protocol::Http,
      json!({
        "status": status,
        "headers": response_headers,
        "file": output_path,
        "bytes": downloaded,
      }),
    );
    print_response(&net_response, mode, no_color);
    return Ok(());
  }

  // Regular response
  let body_text = response
    .text()
    .await
    .map_err(|e| NetError::new(ErrorCode::IoError, e.to_string(), Protocol::Http))?;

  let body_value: serde_json::Value = serde_json::from_str(&body_text).unwrap_or_else(|_| json!(body_text));

  let net_response = NetResponse::new(
    ResponseType::Response,
    Protocol::Http,
    json!({
      "status": status,
      "headers": response_headers,
      "body": body_value,
    }),
  );

  if verbose {
    let metadata = json!({
      "url": url,
      "method": args.method.to_uppercase(),
    });
    print_response(&net_response.with_metadata(metadata), mode, no_color);
  } else {
    print_response(&net_response, mode, no_color);
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

fn map_reqwest_error(e: reqwest::Error) -> NetError {
  if e.is_timeout() {
    NetError::new(ErrorCode::ConnectionTimeout, e.to_string(), Protocol::Http)
  } else if e.is_connect() {
    NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Http)
  } else {
    NetError::new(ErrorCode::ProtocolError, e.to_string(), Protocol::Http)
  }
}
