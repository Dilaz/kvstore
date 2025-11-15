//! # KVStore Client
//!
//! A client library for interacting with KVStore gRPC service.
//!
//! ## Usage
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! kvstore-client = { path = "../kvstore-client" }
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use kvstore_client::KvStoreClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = KvStoreClient::connect("http://127.0.0.1:50051").await?;
//!
//!     // Use the client...
//!     Ok(())
//! }
//! ```

use tonic::transport::Channel;

pub mod generated {
    tonic::include_proto!("kvstore");
}

pub use generated::kv_store_client::KvStoreClient;

/// Connect to a KVStore gRPC server
///
/// # Arguments
///
/// * `endpoint` - The server endpoint (e.g., "http://127.0.0.1:50051")
///
/// # Returns
///
/// A `KvStoreClient` instance ready to make requests
///
/// # Example
///
/// ```rust,no_run
/// use kvstore_client::connect;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut client = connect("http://127.0.0.1:50051").await?;
///     Ok(())
/// }
/// ```
pub async fn connect(
    endpoint: impl AsRef<str>,
) -> Result<KvStoreClient<Channel>, tonic::transport::Error> {
    KvStoreClient::connect(endpoint.as_ref().to_string()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_invalid_uri() {
        let result = connect("invalid://uri").await;
        assert!(result.is_err());
    }
}
