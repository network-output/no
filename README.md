# no (network-output)

A fast, structured networking CLI for HTTP, WebSocket, TCP, UDP, MQTT, SSE, DNS, Ping, and WHOIS.

**Docs:** [network-output.com](https://network-output.com)

## Installation

### Cargo

```sh
cargo install network-output
```

### Homebrew

```sh
brew install network-output/tap/network-output
```

### npm

```sh
npx @network-output/no
```

For a permanent install:

```sh
npm install -g @network-output/no
```

### GitHub Releases

Download prebuilt binaries from [GitHub Releases](https://github.com/network-output/no/releases).

### Build from source

```sh
cargo install --path .
```

## Usage

### HTTP

```sh
no http GET https://httpbin.org/get
no http POST https://api.example.com/data -b '{"key":"value"}' -H "Content-Type:application/json"
no http GET https://api.example.com --bearer $TOKEN
no http GET https://api.example.com --basic user:pass
no http GET https://example.com/file.tar.gz -o file.tar.gz
no http POST https://api.example.com/upload --stdin
```

### WebSocket

```sh
no ws listen ws://localhost:8080
no ws send ws://localhost:8080 -m "hello"
```

### TCP

```sh
no tcp connect localhost:9090 -m "hello"
no tcp connect [::1]:9090 -m "hello"     # IPv6
no tcp connect localhost:9090 --stdin
no tcp listen :9090
no tcp listen [::]:9090                   # IPv6 listen
```

### MQTT

```sh
no mqtt sub localhost:1883 -t "sensor/temp"
no mqtt sub [::1]:1883 -t "sensor/temp"                        # IPv6
no mqtt pub localhost:1883 -t "sensor/temp" -m '{"value":22.5}'
```

### UDP

```sh
no udp send 127.0.0.1:9090 -m "hello"
no udp send [::1]:9090 -m "hello"          # IPv6
no udp send 127.0.0.1:9090 -m "ping" --wait 3s
no udp listen :9090
no udp listen [::]:9090                     # IPv6 listen
```

### SSE

```sh
no sse https://example.com/events
no sse https://example.com/events --bearer $TOKEN
no sse https://example.com/events -H "X-Custom:value"
```

### DNS

```sh
no dns example.com
no dns example.com AAAA
no dns google.com MX
no dns 8.8.8.8                    # auto-detects reverse (PTR) lookup
no dns example.com --server 1.1.1.1
no dns example.com --server 2001:4860:4860::8888   # IPv6 DNS server
```

### Ping

```sh
no ping 127.0.0.1
no ping ::1                                  # IPv6
no ping example.com -n 2
no ping 127.0.0.1 --interval 500ms
no --jq '.data.time_ms' ping 127.0.0.1 -n 3
```

### WHOIS

```sh
no whois example.com
no whois 8.8.8.8
no whois example.com --server whois.verisign-grs.com
no --jq '.data.response' whois example.com
```

### jq

Filter JSON from stdin using jq expressions:

```sh
echo '{"a":1,"b":2}' | no jq '.a'
no http GET https://httpbin.org/get | no jq '.data.body'
```

The `--jq` global flag applies a jq filter to any command's output:

```sh
no --jq '.data.status' http GET https://httpbin.org/get
no --jq '.data.payload' mqtt sub localhost:1883 -t "sensor/temp"
```

## Global Flags

| Flag             | Description                                       |
| ---------------- | ------------------------------------------------- |
| `--json`         | Force JSON output                                 |
| `--pretty`       | Force pretty output                               |
| `--timeout <DURATION>` | Request timeout (e.g. 5s, 300ms, 1m)        |
| `--no-color`     | Disable colors                                    |
| `-v, --verbose`  | Verbose output                                    |
| `-n, --count <N>`| Stop after N data messages (streaming protocols)  |
| `--jq <EXPR>`   | Filter output with a jq expression                |

## Output Format

By default, output is pretty-printed when connected to a terminal and newline-delimited JSON when piped. Use `--json` or `--pretty` to override.

JSON output follows a consistent envelope:

```json
{"type":"response","protocol":"http","timestamp":"2024-01-01T00:00:00.000Z","data":{"status":200,"headers":{},"body":{}}}
```

| Field       | Description                                             |
| ----------- | ------------------------------------------------------- |
| `type`      | `response`, `message`, `connection`, or `error`         |
| `protocol`  | `http`, `ws`, `tcp`, `udp`, `mqtt`, `sse`, `dns`, `ping`, or `whois` |
| `timestamp` | ISO 8601 timestamp with millisecond precision (UTC)     |
| `data`      | Protocol-specific payload                               |

## Exit Codes

| Code | Meaning             |
| ---- | ------------------- |
| `0`  | Success             |
| `1`  | Connection / IO error |
| `2`  | Protocol error      |
| `3`  | Timeout             |
| `4`  | Invalid input       |

## Environment Variables

| Variable         | Description                                  |
| ---------------- | -------------------------------------------- |
| `NO_AUTH_TOKEN`  | Fallback bearer token for authenticated requests |
| `NO_BASIC_AUTH`  | Fallback basic auth credentials as `user:pass`   |

## Documentation

Full documentation is available at [network-output.com](https://network-output.com), covering all protocols, output format, global flags, exit codes, environment variables, and URL normalization rules.

API documentation can be generated with:

```sh
cargo doc --no-deps --open
```

## Development

```sh
just check    # fmt + clippy + test
just build    # debug build
just release  # release build
```

See [AGENTS.md](AGENTS.md) for development conventions.
