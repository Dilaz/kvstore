//! KVStore Server
//!
//! A production-ready key-value storage server with HTTP and gRPC support.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -- --mode=http|grpc|dual
//! ```
//!
//! ## Environment Variables
//!
//! - `REDIS_URL`: Redis connection URL (default: "redis://127.0.0.1:6379")
//! - `HTTP_PORT`: HTTP server port (default: 3000)
//! - `GRPC_PORT`: gRPC server port (default: 50051)
//! - `RUST_LOG`: Logging level (default: "kvstore=info,tower_http=info")

use clap::Parser;
use kvstore::{create_grpc_server, create_http_server, KVStore};
use std::net::{Ipv4Addr, SocketAddr};
use tonic::transport::Server;
use tonic_reflection::server::Builder as ReflectionBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Http,
    Grpc,
    Dual,
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(Mode::Http),
            "grpc" => Ok(Mode::Grpc),
            "dual" => Ok(Mode::Dual),
            _ => Err(format!(
                "Invalid mode: {}. Must be one of: http, grpc, dual",
                s
            )),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "kvstore")]
#[command(about = "A production-ready key-value storage server with HTTP and gRPC support")]
struct Args {
    /// Select which server(s) to start
    #[arg(long, value_name = "MODE", required = true)]
    mode: Mode,
}

async fn run_http(
    store: KVStore,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    tracing::info!("Starting HTTP server on {}", addr);

    let app = create_http_server(store);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn run_grpc(
    store: KVStore,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    tracing::info!("Starting gRPC server on {}", addr);

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("kvstore.KVStore", tonic_health::ServingStatus::Serving)
        .await;

    let service = create_grpc_server(store);
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(kvstore::grpc::KVSTORE_FILE_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(service)
        .serve(addr)
        .await?;

    Ok(())
}

async fn run_dual(
    store: KVStore,
    http_port: u16,
    grpc_port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let http_store = store.clone();
    let http_handle = tokio::spawn(async move { run_http(http_store, http_port).await });

    let grpc_store = store.clone();
    let grpc_handle = tokio::spawn(async move { run_grpc(grpc_store, grpc_port).await });

    tokio::try_join!(async { http_handle.await? }, async { grpc_handle.await? })?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kvstore=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse command line arguments
    let args = Args::parse();
    let mode = args.mode;

    tracing::info!("Starting KVStore server in {:?} mode", mode);

    // Get configuration from environment
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let http_port: u16 = std::env::var("HTTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(kvstore::DEFAULT_HTTP_PORT);
    let grpc_port: u16 = std::env::var("GRPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(kvstore::DEFAULT_GRPC_PORT);

    // Create KVStore instance
    tracing::info!("Connecting to Redis at {}", redis_url);
    let store = KVStore::new(&redis_url).await?;
    tracing::info!("Successfully connected to Redis");

    // Verify health
    if !store.health_check().await? {
        tracing::error!("Redis health check failed");
        return Err("Redis connection unhealthy".into());
    }

    // Start servers based on mode
    match mode {
        Mode::Http => {
            run_http(store, http_port).await?;
        }
        Mode::Grpc => {
            run_grpc(store, grpc_port).await?;
        }
        Mode::Dual => {
            tracing::info!("HTTP: http://localhost:{}", http_port);
            tracing::info!("gRPC: localhost:{}", grpc_port);
            run_dual(store, http_port, grpc_port).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_from_str_http() {
        assert_eq!("http".parse::<Mode>().unwrap(), Mode::Http);
    }

    #[test]
    fn test_mode_from_str_grpc() {
        assert_eq!("grpc".parse::<Mode>().unwrap(), Mode::Grpc);
    }

    #[test]
    fn test_mode_from_str_dual() {
        assert_eq!("dual".parse::<Mode>().unwrap(), Mode::Dual);
    }

    #[test]
    fn test_mode_from_str_invalid() {
        assert!("invalid".parse::<Mode>().is_err());
    }
}
