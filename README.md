# API Check

A Rust HTTP server application with request counting, proxy support, metrics collection, and API testing capabilities.

## Features

- 🖥️ **HTTP Dev Server**: Receives and processes HTTP requests
- 📊 **Request Metrics**: Counts requests and measures processing time for each request
- 🔄 **Proxy Mode**: Optional proxy to forward requests to a target server and record response status codes
- 📈 **Real-time Dashboard**: TUI with charts for metrics visualization (requests, latency, status codes)
- 🧪 **API Testing**: Configurable API testing (number of calls, frequency, HTTP method, body/headers)
- ⚙️ **Configuration API**: HTTP endpoints for managing configuration and exporting metrics as JSON
- 🔧 **Flexible Configuration**: Support for configuration via file (TOML/JSON) and environment variables
- 📝 **Logging**: Comprehensive logging with tracing

## Installation

### From Source

```bash
git clone https://github.com/npv2k1/api-check.git
cd api-check
cargo build --release
```

### Quick Start

```bash
# Start the dev server (default mode)
cargo run

# Start with TUI dashboard
cargo run -- tui

# Run API tests
cargo run -- test -t http://example.com -n 20 -f 50
```

## Usage

### Command Line Interface

```bash
# Show help
./api-check --help

# Start the HTTP server (default)
./api-check server

# Start with TUI dashboard (server + realtime metrics)
./api-check tui

# Run API tests
./api-check test --target http://example.com --num-calls 100 --frequency 10 --method GET

# Show current configuration
./api-check config

# Specify host and port
./api-check --host 0.0.0.0 --port 8080 server

# Use a custom config file
./api-check --config myconfig.toml server

# Enable verbose logging
./api-check -v server
```

### TUI Dashboard

Start the interactive dashboard to view real-time metrics:

```bash
./api-check tui
```

#### TUI Commands:
- `h` - Show help
- `t` - Start API test
- `s` - Stop running test
- `c` - Clear all metrics
- `p` - Toggle proxy mode
- `q` - Quit application

### Management API

The server exposes several HTTP endpoints for configuration and metrics:

#### Configuration Endpoints

```bash
# Get current configuration
curl http://localhost:3000/api/config

# Update configuration
curl -X PUT http://localhost:3000/api/config \
  -H "Content-Type: application/json" \
  -d '{"server": {"host": "127.0.0.1", "port": 3000}}'

# Get/Update proxy configuration
curl http://localhost:3000/api/config/proxy
curl -X PUT http://localhost:3000/api/config/proxy \
  -H "Content-Type: application/json" \
  -d '{"enabled": true, "target": "http://example.com"}'

# Get/Update test configuration
curl http://localhost:3000/api/config/test
curl -X PUT http://localhost:3000/api/config/test \
  -H "Content-Type: application/json" \
  -d '{"num_calls": 100, "frequency_ms": 50}'
```

#### Metrics Endpoints

```bash
# Get all metrics
curl http://localhost:3000/api/metrics

# Get metrics summary
curl http://localhost:3000/api/metrics/summary

# Get recent metrics (last 60 seconds by default)
curl http://localhost:3000/api/metrics/recent?seconds=30

# Clear all metrics
curl -X POST http://localhost:3000/api/metrics/clear
```

#### Test Endpoints

```bash
# Run API test with current configuration
curl -X POST http://localhost:3000/api/test/run

# Run API test with custom parameters
curl -X POST http://localhost:3000/api/test/run \
  -H "Content-Type: application/json" \
  -d '{"num_calls": 50, "frequency_ms": 100, "method": "POST", "target_url": "http://example.com/api", "body": "{\"key\":\"value\"}"}'

# Check if test is running
curl http://localhost:3000/api/test/status

# Stop running test
curl -X POST http://localhost:3000/api/test/stop
```

#### Health Check

```bash
curl http://localhost:3000/api/health
```

### Proxy Mode

Enable proxy mode to forward requests to a target server:

```bash
# Enable proxy via API
curl -X PUT http://localhost:3000/api/config/proxy \
  -H "Content-Type: application/json" \
  -d '{"enabled": true, "target": "http://target-server.com"}'

# All non-API requests will now be forwarded to the target
curl http://localhost:3000/any/path  # Forwards to http://target-server.com/any/path
```

## Configuration

### Configuration File

Create a `config.toml` file (see `config.example.toml`):

```toml
[server]
host = "127.0.0.1"
port = 3000

[proxy]
enabled = false
target = "http://localhost:8080"

[test]
num_calls = 10
frequency_ms = 100
method = "GET"
target_url = "http://localhost:3000/test"
```

### Environment Variables

Configuration can also be set via environment variables (prefixed with `API_CHECK_`):

```bash
export API_CHECK_SERVER_HOST=0.0.0.0
export API_CHECK_SERVER_PORT=8080
export API_CHECK_PROXY_ENABLED=true
export API_CHECK_PROXY_TARGET=http://backend:8080
```

## Project Structure

```
api-check/
├── .github/workflows/    # CI/CD workflows
├── src/
│   ├── api/              # Management API endpoints
│   ├── config/           # Configuration management
│   ├── metrics/          # Metrics collection
│   ├── proxy/            # Proxy functionality
│   ├── server/           # HTTP server
│   ├── testing/          # API testing
│   ├── tui/              # Terminal UI
│   ├── lib.rs            # Library root
│   └── main.rs           # CLI application
├── tests/                # Integration tests
├── examples/             # Usage examples
├── config.example.toml   # Sample configuration
└── docs/                 # Documentation
```

## Development

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Running Clippy (Linter)

```bash
cargo clippy -- -D warnings
```

### Formatting Code

```bash
cargo fmt
```

## API Response Examples

### Metrics Summary Response

```json
{
  "total_requests": 150,
  "successful_requests": 145,
  "failed_requests": 5,
  "avg_latency_ms": 25.5,
  "min_latency_ms": 10.2,
  "max_latency_ms": 150.3,
  "proxied_requests": 50,
  "status_distribution": {
    "200": 140,
    "404": 3,
    "500": 2
  },
  "requests_per_second": 2.5
}
```

### Test Run Results

```json
{
  "total_requests": 100,
  "successful": 98,
  "failed": 2,
  "avg_latency_ms": 45.2,
  "min_latency_ms": 20.1,
  "max_latency_ms": 200.5,
  "total_duration_ms": 5500.0,
  "results": [
    {
      "index": 1,
      "success": true,
      "status_code": 200,
      "latency_ms": 25.5,
      "error": null
    }
  ]
}
```

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
