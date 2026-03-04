# AGENTS.md

Project conventions and architecture reference for `no`, the AI-first networking CLI.

---

## Project Overview

`no` is a networking CLI tool written in Rust. It supports multiple protocols (HTTP, WebSocket, TCP, MQTT, SSE, UDP, DNS, Ping, WHOIS) with a consistent structured output format suited for both human-readable display and machine consumption.

- **Binary name:** `no`
- **Package name:** `no-cli`
- **Crate edition:** 2024
- **Minimum Rust version:** 1.85.0

---

## Project Structure

```
src/
  main.rs              -- Entrypoint: parses CLI, dispatches to protocol handlers
  addr.rs              -- Shared address helpers (parse_listen_addr, client_bind_addr, strip_brackets)
  cli.rs               -- clap-derived argument structs and subcommand enum
  error.rs             -- NetError type, ErrorCode enum, exit code mapping
  output.rs            -- NetResponse, OutputMode, Protocol, ResponseType, print_response
  url.rs               -- URL normalization (auto-infer scheme based on host locality)
  protocols/
    mod.rs             -- Re-exports all protocol modules
    http.rs            -- HTTP request handler (reqwest)
    ws.rs              -- WebSocket listen/send handler (tokio-tungstenite)
    tcp.rs             -- Raw TCP connect/listen handler (tokio)
    jq.rs              -- Standalone jq filter (stdin JSON -> jq expression)
    mqtt.rs            -- MQTT sub/pub handler (rumqttc)
    sse.rs             -- Server-Sent Events handler (eventsource-stream)
    udp.rs             -- UDP datagram send/listen handler (tokio)
    dns.rs             -- DNS lookup handler (hickory-resolver)
    ping.rs            -- ICMP ping handler (surge-ping)
    whois.rs           -- WHOIS lookup handler (raw TCP to port 43)
docs/
  index.html           -- Documentation website (single self-contained file, inline CSS + JS)
  CNAME                -- GitHub Pages custom domain (no-cli.net)
tests/
  helpers/             -- Test infrastructure (servers, CLI helpers)
  http.rs              -- HTTP integration tests (22)
  ws.rs                -- WebSocket integration tests (13)
  tcp.rs               -- TCP integration tests (11)
  mqtt.rs              -- MQTT integration tests (8)
  sse.rs               -- SSE integration tests (10)
  udp.rs               -- UDP integration tests (6)
  dns.rs               -- DNS integration tests (7, 6 network + 1 non-network)
  ping.rs              -- Ping integration tests (6, 5 network + 1 non-network)
  whois.rs             -- WHOIS integration tests (4, 3 network + 1 non-network)
  cli-integration.rs   -- Basic CLI integration tests (13, incl. 5 jq stdin tests)
```

### Module responsibilities

| File | Responsibility |
|---|---|
| `src/addr.rs` | Shared address helpers: `parse_listen_addr()` (parses `:port`, `ip:port`, `[ipv6]:port`), `client_bind_addr()` (matches bind family to target), `strip_brackets()` (strips `[]` for portless protocols) |
| `src/main.rs` | Calls `Cli::parse()`, resolves `OutputMode`, dispatches each `Command` variant to the matching `protocols::<name>::run(...)` |
| `src/cli.rs` | All clap structs and enums: `Cli`, `Command`, per-protocol `Args` and `Action` types |
| `src/error.rs` | `ErrorCode` enum (serializes to `SCREAMING_SNAKE_CASE`), `NetError` struct, `exit()` method |
| `src/output.rs` | `NetResponse` struct, `ResponseType` and `Protocol` enums, `OutputMode::detect()`, `print_response()` |
| `src/url.rs` | `normalize_url(url, scheme)` -- auto-infers `http/https` or `ws/wss` based on host locality |
| `src/protocols/jq.rs` | Standalone `jq` subcommand: reads JSON from stdin, applies a jq filter, prints results |
| `src/protocols/mod.rs` | `pub mod` declarations for each protocol |

---

## How to Add a New Protocol

Follow these steps in order. Use an existing protocol (e.g. `tcp`, `ws`) as a reference.

### 1. Create the protocol module

Create `src/protocols/<name>.rs` with a public async entry point:

```rust
use crate::cli::MyArgs;
use crate::error::{ErrorCode, NetError};
use crate::output::{NetResponse, OutputMode, Protocol, ResponseType, print_response};

pub async fn run(args: MyArgs, mode: OutputMode, no_color: bool, timeout: Option<Duration>) -> Result<(), NetError> {
    // implementation
    Ok(())
}

fn map_my_error(e: SomeLibraryError) -> NetError {
    // see Error Handling section
}
```

### 2. Add CLI args to `src/cli.rs`

- Define an `Args` struct (or an `Action` subcommand enum for protocols with sub-operations).
- Add a variant to the `Command` enum:

```rust
#[command(about = "My new protocol")]
MyProto(MyProtoArgs),
// or, for sub-operations:
MyProto {
    #[command(subcommand)]
    action: MyProtoAction,
},
```

### 3. Add a `Protocol` variant to `src/output.rs`

```rust
pub enum Protocol {
    // existing...
    MyProto,
}
```

The `#[serde(rename_all = "lowercase")]` derive handles JSON serialization automatically.

### 4. Implement `map_<name>_error()`

Add a private function in `src/protocols/<name>.rs` that converts the library-specific error type into a `NetError`. Follow the same pattern used by `tcp` and `ws`:

```rust
fn map_my_error(e: SomeLibError) -> NetError {
    match e {
        // connection refused variants -> ErrorCode::ConnectionRefused
        // timeout variants           -> ErrorCode::ConnectionTimeout
        // TLS/protocol variants      -> ErrorCode::ProtocolError
        // IO variants                -> ErrorCode::IoError
        // _                          -> ErrorCode::ProtocolError
    }
}
```

### 5. Wire up in `src/main.rs`

Add a match arm in `main()`:

```rust
Command::MyProto(args) => protocols::my_proto::run(args, mode, no_color, timeout_ms).await,
```

### 6. Re-export in `src/protocols/mod.rs`

```rust
pub mod my_proto;
```

---

## Error Handling

### `ErrorCode` variants and exit codes

| Variant | Exit Code | When to use |
|---|---|---|
| `ConnectionRefused` | 1 | TCP connection actively refused |
| `DnsResolution` | 1 | Hostname could not be resolved |
| `IoError` | 1 | Generic I/O failure (read/write errors, bind failures) |
| `ProtocolError` | 2 | Protocol-level failure, unexpected frames, TLS errors |
| `TlsError` | 2 | TLS handshake or certificate error |
| `ConnectionTimeout` | 3 | Connection or read timed out |
| `InvalidInput` | 4 | Bad user input (invalid URL, header format, method, port) |

### `NetError` construction

```rust
NetError::new(ErrorCode::ConnectionRefused, e.to_string(), Protocol::Http)
```

### `map_<proto>_error` convention

Each protocol module defines a private `fn map_<proto>_error(e: LibError) -> NetError` that matches on library-specific error kinds and returns the appropriate `ErrorCode`. This keeps error conversion isolated and testable. See `map_tcp_error` in `tcp.rs` and `map_ws_error` in `ws.rs` for canonical examples.

### Error output

`NetError::exit()` constructs a `NetResponse` with `type: "error"` and the error `code` and `message` in the `data` field, prints it through the normal `print_response` path, then calls `process::exit(code.exit_code())`.

---

## Output Schema

All protocol handlers emit one or more `NetResponse` structs. The `print_response` function handles both output modes.

### `NetResponse` fields

| Field | Type | Notes |
|---|---|---|
| `type` | string | One of `response`, `message`, `connection`, `error` (lowercase) |
| `protocol` | string | One of `http`, `ws`, `tcp`, `mqtt`, `sse`, `udp`, `dns`, `ping`, `whois` (lowercase) |
| `timestamp` | string | RFC 3339 UTC with millisecond precision, e.g. `2024-01-01T00:00:00.000Z` |
| `data` | object | Protocol-specific payload |
| `metadata` | object | Optional; omitted from JSON when `None` |

### `ResponseType` semantics

- `response` -- a final result from a request (HTTP response, MQTT publish confirmation)
- `message` -- a streamed message from a long-lived connection (WS frame, TCP data, SSE event, MQTT publish)
- `connection` -- a lifecycle event (connected, closed, listening, accepted)
- `error` -- an error produced by `NetError::exit()`

### Per-protocol response `data` shapes

**HTTP** (`type: response`):
```json
{ "status": 200, "headers": {...}, "body": <parsed-JSON-or-string>, "bytes": 1234 }
```
Metadata (verbose only): `{ "method": "GET", "url": "...", "time_ms": 42 }`

**WebSocket** (`type: message`):
```json
{ "data": <parsed-JSON-or-string>, "binary": false }          // text frame
{ "binary": true, "length": 4, "hex": "deadbeef" }            // binary frame
```
Connection events: `{ "status": "connected", "url": "..." }`, `{ "status": "closed", "code": 1000, "reason": "..." }`

**TCP** (`type: message`):
```json
{ "data": "..." }                                              // text (UTF-8)
{ "binary": true, "length": 4, "hex": "deadbeef" }            // non-UTF-8
```
Connection events: `{ "status": "connected", "address": "..." }`, `{ "status": "closed" }`, `{ "status": "listening", "address": "..." }`, `{ "status": "accepted", "peer": "..." }`

**MQTT** (`type: response` for pub, `type: message` for sub):
```json
{ "status": "published", "topic": "...", "payload": "..." }   // pub
{ "topic": "...", "payload": "...", "qos": 0 }                // sub message
```
Connection events: `{ "status": "subscribed", "topic": "..." }`

**SSE** (`type: message`):
```json
{ "data": <parsed-JSON-or-string>, "event": "update", "id": "1" }
```
Connection events: `{ "status": "connected", "url": "..." }`

**DNS** (`type: response`):
```json
{ "name": "example.com", "type": "A", "records": [{ "value": "93.184.216.34", "ttl": 3600 }] }
```
MX records add `"priority"`. SRV records add `"priority"`, `"weight"`, `"port"`. SOA records add `"mname"`, `"rname"`, `"serial"`, `"refresh"`, `"retry"`, `"expire"`, `"minimum"`.
Metadata (verbose only): `{ "server": "8.8.8.8", "time_ms": 12 }`

**UDP** (`type: message`):
```json
{ "peer": "127.0.0.1:12345", "data": "..." }
```
Connection events: `{ "status": "sent", "address": "...", "bytes": N }`, `{ "status": "listening", "address": "..." }`

**Ping** (`type: message` per reply, `type: response` for summary):
```json
{ "seq": 0, "host": "example.com", "ip": "93.184.216.34", "ttl": 56, "size": 64, "time_ms": 12.3 }
```
Summary: `{ "host": "example.com", "ip": "93.184.216.34", "transmitted": 4, "received": 4, "loss_pct": 0.0, "min_ms": 11.2, "avg_ms": 12.5, "max_ms": 14.1 }`
Metadata (verbose only): `{ "identifier": 1234, "payload_size": 56 }`

**WHOIS** (`type: response`):
```json
{ "query": "example.com", "server": "whois.verisign-grs.com", "response": "Domain Name: EXAMPLE.COM\r\n..." }
```
Metadata (verbose only): `{ "server": "whois.verisign-grs.com:43", "time_ms": 120 }`

### Output modes

`OutputMode::detect(json_flag, pretty_flag)` resolves the mode:
1. `--json` flag -> `Json` (always machine-readable, one JSON object per line)
2. `--pretty` flag -> `Pretty` (always colored human-readable)
3. stdout is a TTY -> `Pretty`
4. stdout is not a TTY -> `Json`

In `Json` mode, each response is a single-line JSON object followed by a newline.
In `Pretty` mode, a header line `[PROTOCOL] TYPE @ timestamp` is printed, followed by pretty-printed JSON data, and optionally a `metadata:` section. Color is suppressed when `--no-color` is passed or the `NO_COLOR` convention applies.

---

## Testing

### Unit tests

Unit tests live inside `#[cfg(test)]` modules in each source file. Run them with:

```
cargo test --lib
```

### Integration tests

Integration tests invoke the compiled binary (`CARGO_BIN_EXE_no`) using `std::process::Command` and parse JSON output. Run them with:

```
cargo test --test '*'
```

Tests that require live network access are annotated with `#[ignore]` and are skipped by default. Run them explicitly with:

```
cargo test -- --ignored
```

Set `NO_NETWORK_TESTS=1` to skip network tests even when `--ignored` is passed.

### Integration test infrastructure

Integration tests use local in-process servers -- no Docker or external services required.

```
tests/
  helpers/
    mod.rs            -- re-exports
    cli.rs            -- no_cmd(), parse_first_json(), parse_all_json()
    server.rs         -- TestServer (axum: HTTP + WS + SSE routes)
    tcp_server.rs     -- TcpTestServer (raw tokio TCP servers)
    udp_server.rs     -- UdpTestServer (raw tokio UDP servers)
    mqtt_broker.rs    -- MqttTestBroker (embedded rumqttd, LazyLock singleton)
  http.rs             -- 17 HTTP tests
  ws.rs               -- 12 WebSocket tests (incl. count + verbose)
  tcp.rs              -- 11 TCP tests (incl. count)
  mqtt.rs             -- 8 MQTT tests
  sse.rs              -- 9 SSE tests (incl. count)
  udp.rs              -- 6 UDP tests
  cli-integration.rs  -- 7 basic CLI tests (incl. help exit codes)
```

**helpers/cli.rs** -- `no_cmd()` returns a `Command` pre-configured with `--json` and env vars stripped (`NO_AUTH_TOKEN`, `NO_BASIC_AUTH`). `parse_first_json()` / `parse_all_json()` extract JSON lines from stdout. `free_port()` finds an available TCP port. `exit_code` module provides named constants (`CONNECTION=1`, `PROTOCOL=2`, `TIMEOUT=3`, `INVALID_INPUT=4`).

**helpers/server.rs** -- `TestServer::start()` spawns an axum server on a dedicated thread bound to port 0. Serves HTTP routes (`/get`, `/post`, `/auth`, `/slow`, `/download`, `/status/{code}`), WebSocket routes (`/ws/echo`, `/ws/close`, `/ws/binary`, `/ws/multi`), and SSE routes (`/events`, `/events/auth`, `/events/named`).

**helpers/tcp_server.rs** -- `start_echo_server()`, `start_multi_message_server()`, `start_silent_server()` each spawn a tokio runtime on a dedicated thread with a TCP listener on port 0.

**helpers/udp_server.rs** -- `start_echo_server()`, `start_multi_message_server()`, `start_silent_server()` each spawn a tokio runtime on a dedicated thread with a UDP socket on port 0.

**helpers/mqtt_broker.rs** -- `MQTT_BROKER` is a `LazyLock` singleton that starts an embedded `rumqttd` broker on a free port. Shared across all MQTT tests to avoid startup overhead.

**Dev-dependencies** used for tests: `axum` (HTTP/WS/SSE server) and `rumqttd` (embedded MQTT broker). Both are listed in `[dev-dependencies]` in `Cargo.toml`.

### Full CI check

```
just check
```

This runs `fmt-check`, `lint`, and `test` in sequence and must pass before merging.

---

## Task Runner

All common tasks are defined in `justfile`. Run `just` with no arguments to list available recipes.

| Command | Description |
|---|---|
| `just build` | Debug build (`cargo build`) |
| `just release` | Release build with LTO and stripping (`cargo build --release`) |
| `just test` | Run all tests (`cargo test`) |
| `just test-unit` | Run unit tests only (`cargo test --lib`) |
| `just test-integration` | Run integration tests only (`cargo test --test '*'`) |
| `just lint` | Run clippy with `-D warnings` |
| `just fmt` | Format code with rustfmt |
| `just fmt-check` | Check formatting without modifying files |
| `just check` | Run `fmt-check`, `lint`, and `test` in sequence |
| `just run [args]` | Run the binary (`cargo run -- [args]`) |

---

## Conventions

### Rust

- Edition 2024, minimum version 1.85.0
- Run `cargo clippy -- -D warnings`; all warnings are treated as errors
- `rustfmt` settings: `max_width = 120`, `tab_spaces = 2` (see `rustfmt.toml`)

### Code style

- No emojis in code, comments, or output strings
- kebab-case for file and directory names
- Follow the existing indentation (2 spaces)
- Prefer pure functions and avoid unnecessary mutation

### Protocol modules

- Each protocol module is self-contained: imports its own CLI args type, defines its own error mapper, and calls `print_response` directly
- The `run` function signature always takes `mode: OutputMode`, `no_color: bool`, `timeout: Option<Duration>` as common parameters. Streaming protocols (WS, TCP, MQTT, SSE, UDP) and Ping also take `count: Option<usize>` and `verbose: bool`. HTTP, DNS, and WHOIS take `verbose: bool` only.
- Timeout is always applied using `tokio::time::timeout` or the underlying library's timeout API

---

## Environment Variables

| Variable | Description |
|---|---|
| `NO_AUTH_TOKEN` | Bearer token fallback for HTTP and SSE requests. Used when `--bearer` is not provided on the command line. |
| `NO_BASIC_AUTH` | Basic auth credentials fallback for HTTP and SSE requests, in `USER:PASS` format. Used when `--basic` is not provided and `NO_AUTH_TOKEN` is also unset. |

Both variables are checked in `protocols/http.rs` and `protocols/sse.rs` via `std::env::var`.

---

## URL Normalization

HTTP, WebSocket, and SSE protocols auto-infer the URL scheme when omitted:

- **Local addresses** (`localhost`, `0.0.0.0`, IPv4 loopback/private/link-local, IPv6 loopback/ULA `fc00::/7`/link-local `fe80::/10`) default to `http://` or `ws://`
- **All other hosts** default to `https://` or `wss://`

Examples:
- `no http GET localhost:3000/api` -> `http://localhost:3000/api`
- `no http GET example.com/api` -> `https://example.com/api`
- `no ws listen localhost:8080/ws` -> `ws://localhost:8080/ws`
- `no ws listen api.example.com/ws` -> `wss://api.example.com/ws`

If a scheme is already present, it is preserved as-is. TCP and MQTT are unchanged (TCP uses `host:port`, MQTT handles `mqtt://` internally).

Implementation: `src/url.rs` -- `normalize_url(url, UrlScheme)`.

---

## IPv6 Addressing

All protocols support IPv6. Address format conventions vary by protocol type:

### Protocols with ports (TCP, UDP, MQTT)

Brackets are required to disambiguate IPv6 from port separators:

```
no tcp connect [::1]:9090
no tcp listen [::]:9090              # listen on all IPv6 interfaces
no udp send [::1]:9090 -m "hello"
no udp listen [::]:9090
no mqtt sub [::1]:1883 -t topic
```

Bare `:port` continues to default to `0.0.0.0` (IPv4). Use `[::]:port` for IPv6 listen.

### Portless protocols (Ping, DNS, WHOIS)

Brackets are optional -- bare IPv6 addresses are accepted:

```
no ping ::1
no ping [::1]                        # also valid
no dns example.com --server 2001:4860:4860::8888
no dns example.com --server [2001:4860:4860::8888]
no whois ::1
```

### Implementation

- `src/addr.rs` provides `parse_listen_addr()` (TCP/UDP listen), `client_bind_addr()` (UDP send), and `strip_brackets()` (Ping/DNS/WHOIS)
- `src/protocols/mqtt.rs` `parse_broker()` handles bracketed IPv6 with and without port
- UDP client socket binds to the same address family as the target via `client_bind_addr()`

---

## Global Flags

| Flag | Short | Description |
|---|---|---|
| `--json` | | Force JSON output |
| `--pretty` | | Force pretty-printed output |
| `--timeout DURATION` | | Request/connection timeout (e.g. 5s, 300ms, 1m) |
| `--no-color` | | Disable colored output |
| `--verbose` | `-v` | Verbose output with metadata |
| `--count N` | `-n` | Stop after N data messages (streaming protocols only) |
| `--jq EXPR` | | Filter output with a jq expression |

### `--jq` behavior

Applies a jq expression to each `NetResponse` before printing. Uses `jaq-core` (pure Rust jq implementation) -- no external `jq` binary required.

- The filter receives the full `NetResponse` JSON object as input (with `type`, `protocol`, `timestamp`, `data`, and optional `metadata` fields)
- String results print raw (no quotes), matching `jq -r` behavior; all other types print as JSON
- Error responses (`type: "error"`) bypass the filter and print normally, so `NetError::exit()` works unchanged
- Invalid expressions cause exit code 4 (`InvalidInput`)
- Runtime errors from the filter go to stderr

Examples:

```
no http GET example.com --jq '.data.status'
no ws listen localhost:8080 --jq '.data' --count 5
no http GET example.com --jq '.data.body.items[]'
no sse example.com/events --jq '.data.data' --count 1
```

### `--count` behavior

Only data messages increment the counter (not lifecycle events like `connected`, `closed`, `subscribed`). When the count is reached, the handler exits cleanly. HTTP ignores `--count`.

### `--verbose` behavior

When `-v` is set, all protocols attach metadata to connection and message events:

- **HTTP**: `method`, `url` (already existed)
- **WebSocket**: connection metadata (`url`), message metadata (`message_number`)
- **TCP**: connection metadata (`address`), message metadata (`bytes`)
- **MQTT**: subscribe metadata (`broker`, `topic`), message metadata (`broker`, `qos`)
- **SSE**: connection metadata (`url`), message metadata (`event_type`)
- **DNS**: response metadata (`server`, `time_ms`)
- **Ping**: summary metadata (`identifier`, `payload_size`)
- **WHOIS**: response metadata (`server`, `time_ms`)

### `jq` subcommand

The `jq` subcommand is a standalone jq replacement for filtering arbitrary JSON from stdin. It does not use the protocol output pipeline -- it reads raw JSON from stdin, applies a jq expression, and prints results directly.

```
echo '{"a":1}' | no jq '.a'         -> 1
echo '{"s":"hi"}' | no jq '.s'      -> hi (raw string, no quotes)
echo '[1,2,3]' | no jq '.[]'        -> 1\n2\n3
```

Exit code 4 on invalid JSON input or invalid jq expression.

---

## Documentation

### Website

The documentation site at [no-cli.net](https://no-cli.net) is a single self-contained HTML file at `docs/index.html` with all CSS and JS inlined. It uses a retromodern CRT aesthetic (dark background, phosphor green accents, scanline overlay). GitHub Pages serves the site from the `docs/` directory on the `main` branch.

To preview locally:

```
cd docs && python3 -m http.server 8080
```

When editing the docs site:

- Keep everything in the single `index.html` file (no external CSS/JS dependencies except Google Fonts)
- Maintain the existing design system: background `#0a0a0f`, accent `#00ff88`, JetBrains Mono for headings/code
- Test at both desktop (1280px+) and mobile (375px) breakpoints
- Verify sidebar navigation, copy buttons, and syntax highlighting still work

### Rustdoc

All public items have `///` doc comments. Generate and view with:

```
cargo doc --no-deps --open
```

Doc comment conventions:

- First line: single-sentence summary
- Blank line before extended description
- `# Errors` section for `Result`-returning functions
- Intra-doc links where useful: `[`NetError`]`, `[`Protocol`]`
- No code examples (binary crate)
- 120 char line width

---

## Debug Artifacts

When using Playwright MCP or other browser automation for visual testing, save all generated screenshots and debug files to a dedicated directory rather than the project root. The `.playwright-mcp/` directory and any stray image files are gitignored.

- Save screenshots to a temporary directory or clean them up after verification
- Never commit `.png`, `.jpeg`, or `.log` files generated during testing
- The `.playwright-mcp/` directory is created automatically by the Playwright MCP server and is gitignored

---

### MQTT broker is positional

The MQTT broker address is a positional argument (not a flag):

```
no mqtt pub localhost:1883 -t topic -m message
no mqtt sub localhost:1883 -t topic
```
