//! Library usage example
//!
//! Run with: cargo run --example library_usage
//!
//! This example demonstrates how to use KVStore as a library in your own application.

use kvstore::KVStore;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("KVStore Library Usage Example\n");

    // Create a KVStore instance
    let store = KVStore::new("redis://127.0.0.1:6379").await?;
    println!("✓ Connected to Redis");

    // Check health
    let healthy = store.health_check().await?;
    println!("✓ Health check: {}", if healthy { "OK" } else { "Failed" });

    // For this example, we'll use a demo token
    // In production, you would validate tokens against Redis
    let token = "demo-token";

    // First, add the token to Redis (normally done by an admin)
    let mut conn = store.connection_manager();
    redis::cmd("SADD")
        .arg("tokens")
        .arg(token)
        .query_async::<()>(&mut conn)
        .await?;
    println!("✓ Demo token created");

    // Validate the token
    let is_valid = store.validate_token(token).await?;
    println!("✓ Token validation: {}", is_valid);

    // Set a value
    store.set(token, "user:123:name", "Alice", None).await?;
    println!("✓ Set user:123:name = Alice");

    // Set a value with TTL
    store
        .set(token, "session:abc", "active", Some(3600))
        .await?;
    println!("✓ Set session:abc = active (TTL: 3600s)");

    // Get a value
    let name = store.get(token, "user:123:name").await?;
    println!("✓ Get user:123:name = {}", name);

    // Set multiple values
    store
        .set(token, "user:123:email", "alice@example.com", None)
        .await?;
    store.set(token, "user:123:age", "30", None).await?;
    store.set(token, "user:456:name", "Bob", None).await?;
    println!("✓ Set multiple user attributes");

    // List all user:123 keys
    let keys: Vec<String> = store.list(token, "user:123:").await?.collect().await;
    println!("✓ Keys with prefix 'user:123:': {:?}", keys);

    // List all user keys
    let all_user_keys: Vec<String> = store.list(token, "user:").await?.collect().await;
    println!("✓ All user keys: {:?}", all_user_keys);

    // Delete a value
    store.delete(token, "session:abc").await?;
    println!("✓ Deleted session:abc");

    // Try to get deleted value
    match store.get(token, "session:abc").await {
        Ok(_) => println!("✗ Value still exists!"),
        Err(_) => println!("✓ Confirmed session:abc is deleted"),
    }

    // Clean up
    store.delete(token, "user:123:name").await?;
    store.delete(token, "user:123:email").await?;
    store.delete(token, "user:123:age").await?;
    store.delete(token, "user:456:name").await?;
    redis::cmd("SREM")
        .arg("tokens")
        .arg(token)
        .query_async::<()>(&mut conn)
        .await?;
    println!("✓ Cleaned up test data");

    println!("\n✅ All operations completed successfully!");

    Ok(())
}
