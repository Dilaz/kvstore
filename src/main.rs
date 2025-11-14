//! KVStore Server
//!
//! A production-ready key-value storage server with HTTP and gRPC support.
//!
//! ## Environment Variables
//!
//! - `REDIS_URL`: Redis connection URL (default: "redis://127.0.0.1:6379")
//! - `HTTP_PORT`: HTTP server port (default: 3000)
//! - `GRPC_PORT`: gRPC server port (default: 50051)
//! - `ENABLE_HTTP`: Enable HTTP server (default: true)
//! - `ENABLE_GRPC`: Enable gRPC server (default: true)
//! - `RUST_LOG`: Logging level (default: "kvstore=info,tower_http=info")

use kvstore::{create_grpc_server, create_http_server, KVStore};
use std::net::{Ipv4Addr, SocketAddr};
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    tracing::info!("Starting KVStore server");

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
    let enable_http = std::env::var("ENABLE_HTTP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true);
    let enable_grpc = std::env::var("ENABLE_GRPC")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true);

    // Create KVStore instance
    tracing::info!("Connecting to Redis at {}", redis_url);
    let store = KVStore::new(&redis_url).await?;
    tracing::info!("Successfully connected to Redis");

    // Verify health
    if !store.health_check().await? {
        tracing::error!("Redis health check failed");
        return Err("Redis connection unhealthy".into());
    }

    let mut handles = vec![];

    // Start HTTP server
    if enable_http {
        let store_clone = store.clone();
        let http_handle = tokio::spawn(async move {
            let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, http_port));
            tracing::info!("Starting HTTP server on {}", addr);

            let app = create_http_server(store_clone);
            let listener = tokio::net::TcpListener::bind(addr).await?;

            axum::serve(listener, app).await?;

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(http_handle);
    }

    // Start gRPC server
    if enable_grpc {
        let store_clone = store.clone();
        let grpc_handle = tokio::spawn(async move {
            let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, grpc_port));
            tracing::info!("Starting gRPC server on {}", addr);

            let service = create_grpc_server(store_clone);

            Server::builder()
                .add_service(service)
                .serve(addr)
                .await?;

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });
        handles.push(grpc_handle);
    }

    if handles.is_empty() {
        tracing::error!("No servers enabled. Set ENABLE_HTTP or ENABLE_GRPC to true.");
        return Err("No servers enabled".into());
    }

    // Wait for all servers
    for handle in handles {
        if let Err(e) = handle.await? {
            tracing::error!("Server error: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
