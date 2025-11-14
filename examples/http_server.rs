//! Simple HTTP server example
//!
//! Run with: cargo run --example http_server
//!
//! This example demonstrates how to create a simple HTTP server using the kvstore library.

use kvstore::{create_http_server, KVStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create KVStore instance
    let store = KVStore::new("redis://127.0.0.1:6379").await?;

    // Create HTTP server
    let app = create_http_server(store);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("HTTP server listening on http://0.0.0.0:3000");
    println!("Try:");
    println!("  curl -H 'Authorization: Bearer YOUR_TOKEN' http://localhost:3000/healthz");

    axum::serve(listener, app).await?;

    Ok(())
}
