//! # KVStore
//!
//! A lightweight, production-ready Rust library for key-value storage with HTTP and gRPC support,
//! backed by Redis.
//!
//! ## Features
//!
//! - **Multi-Protocol Support**: HTTP REST API and gRPC
//! - **Token-Based Authentication**: Secure, namespace-isolated storage
//! - **Redis Backend**: Reliable and fast key-value operations
//! - **Production Ready**: Comprehensive error handling, logging, and middleware
//! - **Developer Friendly**: Easy to use as a library or standalone service
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use kvstore::{KVStore, create_http_server};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a KVStore instance
//!     let store = KVStore::new("redis://127.0.0.1:6379").await?;
//!
//!     // Start HTTP server
//!     let app = create_http_server(store.clone());
//!     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
//!     axum::serve(listener, app).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod grpc;
pub mod http;
pub mod store;

pub use error::{KVStoreError, Result};
pub use store::KVStore;

// Re-export commonly used types
pub use axum::Router;
pub use redis::aio::ConnectionManager;

/// Creates an HTTP server with all routes configured
///
/// # Example
///
/// ```rust,no_run
/// use kvstore::{KVStore, create_http_server};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let store = KVStore::new("redis://127.0.0.1:6379").await?;
///     let app = create_http_server(store);
///     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
///     axum::serve(listener, app).await?;
///     Ok(())
/// }
/// ```
pub fn create_http_server(store: KVStore) -> Router {
    http::create_router(store)
}

/// Creates a gRPC server
///
/// # Example
///
/// ```rust,no_run
/// use kvstore::{KVStore, create_grpc_server};
/// use tonic::transport::Server;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let store = KVStore::new("redis://127.0.0.1:6379").await?;
///     let addr = "[::1]:50051".parse()?;
///     let service = create_grpc_server(store);
///
///     Server::builder()
///         .add_service(service)
///         .serve(addr)
///         .await?;
///
///     Ok(())
/// }
/// ```
pub fn create_grpc_server(
    store: KVStore,
) -> grpc::kv_store_server::KvStoreServer<grpc::KVStoreService> {
    grpc::create_service(store)
}

/// Default Redis tokens set name
pub const REDIS_TOKENS_TABLE: &str = "tokens";

/// Default HTTP port
pub const DEFAULT_HTTP_PORT: u16 = 3000;

/// Default gRPC port
pub const DEFAULT_GRPC_PORT: u16 = 50051;
