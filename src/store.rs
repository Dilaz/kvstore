//! Core KVStore implementation
//!
//! Provides the main KVStore struct and operations for interacting with Redis.

use crate::error::{KVStoreError, Result};
use crate::REDIS_TOKENS_TABLE;
use redis::{aio::ConnectionManager, AsyncCommands};
use std::sync::Arc;

/// Main KVStore struct that manages Redis connections and operations
///
/// This struct is cheaply cloneable (uses Arc internally) and can be safely
/// shared across threads.
///
/// # Example
///
/// ```rust,no_run
/// use kvstore::KVStore;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let store = KVStore::new("redis://127.0.0.1:6379").await?;
///
///     // Validate a token
///     let is_valid = store.validate_token("my-token").await?;
///
///     // Set a value
///     store.set("my-token", "key1", "value1", None).await?;
///
///     // Get a value
///     let value = store.get("my-token", "key1").await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct KVStore {
    conn: Arc<ConnectionManager>,
}

impl KVStore {
    /// Create a new KVStore instance
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "redis://127.0.0.1:6379")
    ///
    /// # Errors
    ///
    /// Returns an error if connection to Redis fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use kvstore::KVStore;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let store = KVStore::new("redis://127.0.0.1:6379").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(redis_url: &str) -> Result<Self> {
        tracing::info!("Connecting to Redis at {}", redis_url);

        let client = redis::Client::open(redis_url).map_err(|e| {
            tracing::error!("Failed to create Redis client: {}", e);
            e
        })?;

        let conn = ConnectionManager::new(client).await.map_err(|e| {
            tracing::error!("Failed to create connection manager: {}", e);
            e
        })?;

        tracing::info!("Successfully connected to Redis");

        Ok(Self {
            conn: Arc::new(conn),
        })
    }

    /// Create a KVStore from an existing ConnectionManager
    ///
    /// Useful for testing or when you want to manage the connection yourself.
    pub fn from_connection_manager(conn: ConnectionManager) -> Self {
        Self {
            conn: Arc::new(conn),
        }
    }

    /// Get a clone of the underlying connection manager
    pub fn connection_manager(&self) -> ConnectionManager {
        (*self.conn).clone()
    }

    /// Validate if a token exists in the tokens set
    ///
    /// # Arguments
    ///
    /// * `token` - The token to validate
    ///
    /// # Returns
    ///
    /// `true` if the token is valid, `false` otherwise
    pub async fn validate_token(&self, token: &str) -> Result<bool> {
        let mut conn = (*self.conn).clone();
        let exists: bool = conn
            .sismember(REDIS_TOKENS_TABLE, token)
            .await
            .map_err(|e| {
                tracing::error!("Failed to validate token: {}", e);
                e
            })?;
        Ok(exists)
    }

    /// Get a value from the store
    ///
    /// # Arguments
    ///
    /// * `token` - Authentication token (used as namespace prefix)
    /// * `key` - The key to retrieve
    ///
    /// # Returns
    ///
    /// The value if found, or an error if the key doesn't exist
    pub async fn get(&self, token: &str, key: &str) -> Result<String> {
        let namespaced_key = format!("{}:{}", token, key);
        tracing::debug!("GET {}", namespaced_key);

        let mut conn = (*self.conn).clone();
        let value: Option<String> = conn.get(&namespaced_key).await.map_err(|e| {
            tracing::error!("Failed to get key {}: {}", namespaced_key, e);
            e
        })?;

        value.ok_or_else(|| KVStoreError::KeyNotFound(key.to_string()))
    }

    /// Set a value in the store
    ///
    /// # Arguments
    ///
    /// * `token` - Authentication token (used as namespace prefix)
    /// * `key` - The key to set
    /// * `value` - The value to store
    /// * `ttl_seconds` - Optional TTL in seconds
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    pub async fn set(
        &self,
        token: &str,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> Result<()> {
        let namespaced_key = format!("{}:{}", token, key);
        tracing::debug!("SET {} (TTL: {:?})", namespaced_key, ttl_seconds);

        let mut conn = (*self.conn).clone();

        if let Some(ttl) = ttl_seconds {
            conn.set_ex::<_, _, ()>(&namespaced_key, value, ttl as u64)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to set key {} with TTL: {}", namespaced_key, e);
                    e
                })?;
        } else {
            conn.set::<_, _, ()>(&namespaced_key, value)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to set key {}: {}", namespaced_key, e);
                    e
                })?;
        }

        Ok(())
    }

    /// Delete a value from the store
    ///
    /// # Arguments
    ///
    /// * `token` - Authentication token (used as namespace prefix)
    /// * `key` - The key to delete
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    pub async fn delete(&self, token: &str, key: &str) -> Result<()> {
        let namespaced_key = format!("{}:{}", token, key);
        tracing::debug!("DELETE {}", namespaced_key);

        let mut conn = (*self.conn).clone();
        conn.del::<_, ()>(&namespaced_key).await.map_err(|e| {
            tracing::error!("Failed to delete key {}: {}", namespaced_key, e);
            e
        })?;

        Ok(())
    }

    /// List all keys with a given prefix (for a token)
    ///
    /// # Arguments
    ///
    /// * `token` - Authentication token (used as namespace prefix)
    /// * `prefix` - Additional prefix to filter keys (optional, use "" for all keys)
    ///
    /// # Returns
    ///
    /// A vector of keys (without the token namespace)
    pub async fn list(&self, token: &str, prefix: &str) -> Result<Vec<String>> {
        let pattern = if prefix.is_empty() {
            format!("{}:*", token)
        } else {
            format!("{}:{}*", token, prefix)
        };

        tracing::debug!("LIST {}", pattern);

        let mut conn = (*self.conn).clone();
        let keys: Vec<String> = conn.keys(&pattern).await.map_err(|e| {
            tracing::error!("Failed to list keys with pattern {}: {}", pattern, e);
            e
        })?;

        // Remove the token prefix from each key
        let prefix_len = token.len() + 1; // +1 for the colon
        let keys = keys
            .into_iter()
            .filter_map(|k| {
                if k.len() > prefix_len {
                    Some(k[prefix_len..].to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(keys)
    }

    /// Check if the Redis connection is healthy
    ///
    /// # Returns
    ///
    /// `true` if the connection is healthy, `false` otherwise
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = (*self.conn).clone();
        let result: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("Health check failed: {}", e);
                e
            })?;

        Ok(result == "PONG")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance
    // They are designed to work with the test environment

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_new_kvstore() {
        let result = KVStore::new("redis://127.0.0.1:6379").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_set_and_get() {
        let store = KVStore::new("redis://127.0.0.1:6379").await.unwrap();

        // Set a value
        store
            .set("test-token", "test-key", "test-value", None)
            .await
            .unwrap();

        // Get the value
        let value = store.get("test-token", "test-key").await.unwrap();
        assert_eq!(value, "test-value");

        // Clean up
        store.delete("test-token", "test-key").await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_delete() {
        let store = KVStore::new("redis://127.0.0.1:6379").await.unwrap();

        // Set a value
        store
            .set("test-token", "test-key-del", "test-value", None)
            .await
            .unwrap();

        // Delete the value
        store.delete("test-token", "test-key-del").await.unwrap();

        // Verify it's gone
        let result = store.get("test-token", "test-key-del").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_health_check() {
        let store = KVStore::new("redis://127.0.0.1:6379").await.unwrap();
        let healthy = store.health_check().await.unwrap();
        assert!(healthy);
    }
}
