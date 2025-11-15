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

#[derive(Debug)]
enum CliError {
    MissingMode,
    InvalidMode(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::MissingMode => write!(f, "Missing --mode flag"),
            CliError::InvalidMode(mode) => write!(
                f,
                "Invalid mode: {}. Must be one of: http, grpc, dual",
                mode
            ),
        }
    }
}

impl std::error::Error for CliError {}

fn parse_mode(args: &[String]) -> Result<Mode, CliError> {
    for (i, arg) in args.iter().enumerate() {
        if arg == "--mode" {
            if let Some(mode_str) = args.get(i + 1) {
                return match mode_str.as_str() {
                    "http" => Ok(Mode::Http),
                    "grpc" => Ok(Mode::Grpc),
                    "dual" => Ok(Mode::Dual),
                    _ => Err(CliError::InvalidMode(mode_str.clone())),
                };
            } else {
                return Err(CliError::MissingMode);
            }
        } else if arg.starts_with("--mode=") {
            let mode_str = arg.strip_prefix("--mode=").unwrap();
            return match mode_str {
                "http" => Ok(Mode::Http),
                "grpc" => Ok(Mode::Grpc),
                "dual" => Ok(Mode::Dual),
                _ => Err(CliError::InvalidMode(mode_str.to_string())),
            };
        }
    }
    Err(CliError::MissingMode)
}

fn print_usage() {
    eprintln!("Usage: kvstore --mode=http|grpc|dual");
    eprintln!("  --mode=http  Start HTTP server only");
    eprintln!("  --mode=grpc  Start gRPC server only");
    eprintln!("  --mode=dual  Start both HTTP and gRPC servers");
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

    let service = create_grpc_server(store);
    let reflection_service = ReflectionBuilder::configure()
        .register_encoded_file_descriptor_set(kvstore::grpc::KVSTORE_FILE_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
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
    let args: Vec<String> = std::env::args().collect();
    let mode = match parse_mode(&args) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: {}", e);
            print_usage();
            return Err(e.into());
        }
    };

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
    fn test_parse_mode_http() {
        let args = vec![
            "kvstore".to_string(),
            "--mode".to_string(),
            "http".to_string(),
        ];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Http);
    }

    #[test]
    fn test_parse_mode_grpc() {
        let args = vec![
            "kvstore".to_string(),
            "--mode".to_string(),
            "grpc".to_string(),
        ];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Grpc);
    }

    #[test]
    fn test_parse_mode_dual() {
        let args = vec![
            "kvstore".to_string(),
            "--mode".to_string(),
            "dual".to_string(),
        ];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Dual);
    }

    #[test]
    fn test_parse_mode_missing() {
        let args = vec!["kvstore".to_string()];
        assert!(matches!(parse_mode(&args), Err(CliError::MissingMode)));
    }

    #[test]
    fn test_parse_mode_invalid() {
        let args = vec![
            "kvstore".to_string(),
            "--mode".to_string(),
            "invalid".to_string(),
        ];
        assert!(matches!(parse_mode(&args), Err(CliError::InvalidMode(_))));
    }

    #[test]
    fn test_parse_mode_missing_value() {
        let args = vec!["kvstore".to_string(), "--mode".to_string()];
        assert!(matches!(parse_mode(&args), Err(CliError::MissingMode)));
    }

    #[test]
    fn test_parse_mode_equals_format() {
        let args = vec!["kvstore".to_string(), "--mode=http".to_string()];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Http);
    }

    #[test]
    fn test_parse_mode_equals_format_grpc() {
        let args = vec!["kvstore".to_string(), "--mode=grpc".to_string()];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Grpc);
    }

    #[test]
    fn test_parse_mode_equals_format_dual() {
        let args = vec!["kvstore".to_string(), "--mode=dual".to_string()];
        assert_eq!(parse_mode(&args).unwrap(), Mode::Dual);
    }
}
