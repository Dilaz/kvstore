# KVStore

A lightweight, production-ready Rust library for key-value storage with **HTTP** and **gRPC** support, backed by Redis.

[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Features

- 🚀 **Dual Protocol Support**: HTTP REST API and gRPC
- 🔐 **Token-Based Authentication**: Secure, namespace-isolated storage
- 📦 **Use as Library or Binary**: Import into your project or run as standalone service
- ⚡ **High Performance**: Built on Axum, Tonic, and async Rust
- 🏭 **Production Ready**: Comprehensive error handling, logging, and middleware
- 🐳 **Docker & Kubernetes**: Full deployment support included
- 🧪 **Well Tested**: Unit and integration tests included
- 📚 **Excellent Documentation**: Examples and API docs

## Quick Start

### As a Binary (Standalone Server)

```bash
# Start Redis
docker-compose up -d redis

# Run the server with --mode flag
cargo run --release -- --mode=http   # HTTP server only
cargo run --release -- --mode=grpc   # gRPC server only
cargo run --release -- --mode=dual    # Both HTTP and gRPC servers

# HTTP server runs on port 3000 (default)
# gRPC server runs on port 50051 (default)
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
kvstore = "0.2"
```

Use in your code:

```rust
use kvstore::KVStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a KVStore instance
    let store = KVStore::new("redis://127.0.0.1:6379").await?;

    // Set a value
    store.set("my-token", "user:123", "Alice", None).await?;

    // Get a value
    let value = store.get("my-token", "user:123").await?;
    println!("Value: {}", value);

    // Delete a value
    store.delete("my-token", "user:123").await?;

    Ok(())
}
```

## HTTP API

All endpoints except `/healthz` require Bearer token authentication.

### Health Check

```bash
GET /healthz
```

Returns 200 OK if Redis is healthy.

### Get a Value

```bash
GET /:key
Authorization: Bearer YOUR_TOKEN
```

Returns the value as JSON:

```json
{
  "value": "your-value"
}
```

### Set a Value

```bash
POST /:key
Authorization: Bearer YOUR_TOKEN
Content-Type: application/json

{
  "value": "your-value",
  "ttl_seconds": 3600  // Optional TTL in seconds
}
```

Returns:

```json
{
  "message": "OK"
}
```

### Delete a Value

```bash
DELETE /:key
Authorization: Bearer YOUR_TOKEN
```

Returns:

```json
{
  "message": "OK"
}
```

## gRPC API

The gRPC service is defined in `proto/kvstore.proto` and provides the following methods:

- `Get(GetRequest) -> GetResponse`
- `Set(SetRequest) -> SetResponse`
- `Delete(DeleteRequest) -> DeleteResponse`
- `HealthCheck(HealthCheckRequest) -> HealthCheckResponse`
- `List(ListRequest) -> stream ListResponse` (streaming)

See the [proto file](proto/kvstore.proto) for full definitions.

## Configuration

Configure the server using command-line flags and environment variables:

### Command-Line Flags

- `--mode=http|grpc|dual` - Select which server(s) to start (required)

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection URL |
| `HTTP_PORT` | `3000` | HTTP server port |
| `GRPC_PORT` | `50051` | gRPC server port |
| `RUST_LOG` | `kvstore=info,tower_http=info` | Logging level |

## Authentication

KVStore uses bearer token authentication. Tokens are stored in a Redis set named `tokens`.

### Adding a Token

```bash
redis-cli SADD tokens "your-token-here"
```

### Token Validation

All authenticated requests validate the token against the `tokens` set. Each token acts as a namespace prefix for keys, ensuring isolation between different tokens.

For example, with token `abc123`, a key `user:1` is stored as `abc123:user:1` in Redis.

## Examples

The `examples/` directory contains several usage examples:

### HTTP Server Only

```bash
cargo run --release -- --mode=http
```

### gRPC Server Only

```bash
cargo run --release -- --mode=grpc
```

### Dual HTTP + gRPC Server

```bash
cargo run --release -- --mode=dual
```

Note: The `examples/` directory still contains example code that demonstrates server setup patterns.

### Library Usage

```bash
cargo run --example library_usage
```

## Client Library

A separate `kvstore-client` crate is available for use in your Rust projects:

Add to your `Cargo.toml`:

```toml
[dependencies]
kvstore-client = { path = "../kvstore-client" }
# or from crates.io (when published)
# kvstore-client = "0.2"
```

Use in your code:

```rust
use kvstore_client::{KvStoreClient, connect};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let mut client = connect("http://127.0.0.1:50051").await?;
    
    // Use the client...
    Ok(())
}
```

## Development

### Prerequisites

- Rust 1.75 or later
- Redis 6.0 or later

### Building

```bash
cargo build --release
```

### Running Tests

Tests require a running Redis instance:

```bash
# Start Redis
docker-compose up -d redis

# Run tests (including ignored integration tests)
cargo test -- --ignored

# Run only unit tests
cargo test --lib
```

### Running with Docker

```bash
# Build the image
docker build -t kvstore .

# Run with docker-compose
docker-compose up
```

## Deployment

### Kubernetes

Kubernetes manifests are available in the `k8s/` directory:

```bash
kubectl apply -f k8s/deployment-http.yaml
kubectl apply -f k8s/deployment-grpc.yaml
kubectl apply -f k8s/service-http.yaml
kubectl apply -f k8s/service-grpc.yaml
kubectl apply -f k8s/ingress.yaml
```

### Docker

```bash
docker run -p 3000:3000 -p 50051:50051 \
  -e REDIS_URL=redis://redis:6379 \
  kvstore
```

## Architecture

```
┌─────────────────┐
│   HTTP Client   │
└────────┬────────┘
         │
    ┌────▼─────┐         ┌──────────┐
    │   Axum   │────────▶│  KVStore │────────┐
    └──────────┘         └──────────┘        │
                                              │
┌─────────────────┐                           │
│   gRPC Client   │                           │
└────────┬────────┘                           │
         │                                    │
    ┌────▼─────┐         ┌──────────┘        │
    │  Tonic   │────────▶                     │
    └──────────┘                              │
                                              │
                                         ┌────▼────┐
                                         │  Redis  │
                                         └─────────┘
```

## Library API

### KVStore

The main struct for interacting with Redis:

```rust
impl KVStore {
    // Create a new instance
    pub async fn new(redis_url: &str) -> Result<Self>;

    // Validate a token
    pub async fn validate_token(&self, token: &str) -> Result<bool>;

    // Get a value
    pub async fn get(&self, token: &str, key: &str) -> Result<String>;

    // Set a value (optionally with TTL)
    pub async fn set(&self, token: &str, key: &str, value: &str, ttl_seconds: Option<i64>) -> Result<()>;

    // Delete a value
    pub async fn delete(&self, token: &str, key: &str) -> Result<()>;

    // List keys with a prefix
    pub async fn list(&self, token: &str, prefix: &str) -> Result<Vec<String>>;

    // Health check
    pub async fn health_check(&self) -> Result<bool>;
}
```

### Creating Servers

```rust
// HTTP server
use kvstore::{KVStore, create_http_server};

let store = KVStore::new("redis://127.0.0.1:6379").await?;
let app = create_http_server(store);
// Use with axum::serve()

// gRPC server
use kvstore::{KVStore, create_grpc_server};

let store = KVStore::new("redis://127.0.0.1:6379").await?;
let service = create_grpc_server(store);
// Use with tonic::transport::Server
```

## Performance

Benchmarks captured with `cargo bench --bench benchmarks` on a local WSL2 dev machine (Redis 7.2 running on localhost). Values show Criterion's reported median latency per operation and the derived throughput (`1_000_000 / latency_µs`).

| Scenario | Median latency | Approx throughput |
|----------|----------------|-------------------|
| Library `set` | 172 µs | ~5.8k ops/s |
| Library `get` | 167 µs | ~6.0k ops/s |
| Library `delete` | 161 µs | ~6.2k ops/s |
| HTTP `set` | 390 µs | ~2.6k req/s |
| HTTP `get` | 359 µs | ~2.8k req/s |
| HTTP `delete` | 363 µs | ~2.8k req/s |
| gRPC `set` | 419 µs | ~2.4k req/s |
| gRPC `get` | 430 µs | ~2.3k req/s |
| gRPC `delete` | 420 µs | ~2.4k req/s |

*Hardware, Redis configuration, and network path heavily influence these numbers; run the same command in your environment to obtain comparable measurements.*

KVStore builds on:
- **Axum** for the HTTP API
- **Tonic** for gRPC
- **Tokio** for the async runtime
- **Redis** for storage

## License

MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

