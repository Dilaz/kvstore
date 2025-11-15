//! Dual HTTP and gRPC server example
//!
//! Run with: cargo run --example dual_server
//!
//! This example demonstrates how to run both HTTP and gRPC servers concurrently.

use kvstore::{create_grpc_server, create_http_server, KVStore};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create KVStore instance (shared between both servers)
    let store = KVStore::new("redis://127.0.0.1:6379").await?;

    println!("Starting dual server (HTTP + gRPC)...");

    // Start HTTP server
    let http_store = store.clone();
    let http_handle = tokio::spawn(async move {
        let app = create_http_server(http_store);
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
        println!("HTTP server listening on http://0.0.0.0:3000");
        axum::serve(listener, app).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });

    // Start gRPC server
    let grpc_store = store.clone();
    let grpc_handle = tokio::spawn(async move {
        let service = create_grpc_server(grpc_store);
        let addr = "0.0.0.0:50051".parse()?;
        println!("gRPC server listening on {}", addr);
        Server::builder().add_service(service).serve(addr).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    });

    println!("Both servers are running!");
    println!("HTTP: http://localhost:3000");
    println!("gRPC: localhost:50051");

    // Wait for both servers
    match http_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(format!("HTTP server error: {}", e).into()),
        Err(e) => return Err(format!("HTTP server task error: {}", e).into()),
    }

    match grpc_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(format!("gRPC server error: {}", e).into()),
        Err(e) => return Err(format!("gRPC server task error: {}", e).into()),
    }

    Ok(())
}
