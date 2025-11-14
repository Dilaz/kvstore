//! Simple gRPC server example
//!
//! Run with: cargo run --example grpc_server
//!
//! This example demonstrates how to create a gRPC server using the kvstore library.

use kvstore::{create_grpc_server, KVStore};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create KVStore instance
    let store = KVStore::new("redis://127.0.0.1:6379").await?;

    // Create gRPC service
    let service = create_grpc_server(store);

    // Start server
    let addr = "0.0.0.0:50051".parse()?;
    println!("gRPC server listening on {}", addr);
    println!("You can test with a gRPC client like grpcurl or BloomRPC");

    Server::builder().add_service(service).serve(addr).await?;

    Ok(())
}
