//! Error types for KVStore
//!
//! Provides comprehensive error handling for all KVStore operations.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Result type alias for KVStore operations
pub type Result<T> = std::result::Result<T, KVStoreError>;

/// Comprehensive error types for KVStore operations
#[derive(Debug, Error)]
pub enum KVStoreError {
    /// Redis connection or operation error
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// IO operation error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Authentication failure
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    /// Key not found in storage
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Invalid request or parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),

    /// UTF-8 conversion error
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

impl IntoResponse for KVStoreError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            KVStoreError::Redis(ref e) => {
                tracing::error!("Redis error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
            }
            KVStoreError::Io(ref e) => {
                tracing::error!("IO error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "IO error")
            }
            KVStoreError::Unauthorized(ref msg) => {
                tracing::warn!("Unauthorized: {}", msg);
                (StatusCode::UNAUTHORIZED, "Unauthorized")
            }
            KVStoreError::KeyNotFound(ref key) => {
                tracing::debug!("Key not found: {}", key);
                (StatusCode::NOT_FOUND, "Key not found")
            }
            KVStoreError::InvalidRequest(ref msg) => {
                tracing::warn!("Invalid request: {}", msg);
                (StatusCode::BAD_REQUEST, msg.as_str())
            }
            KVStoreError::Internal(ref msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
            }
            KVStoreError::Utf8(ref e) => {
                tracing::error!("UTF-8 error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Encoding error")
            }
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
        }));

        (status, body).into_response()
    }
}

impl From<KVStoreError> for tonic::Status {
    fn from(error: KVStoreError) -> Self {
        match error {
            KVStoreError::Redis(e) => {
                tonic::Status::internal(format!("Database error: {}", e))
            }
            KVStoreError::Io(e) => tonic::Status::internal(format!("IO error: {}", e)),
            KVStoreError::Unauthorized(msg) => tonic::Status::unauthenticated(msg),
            KVStoreError::KeyNotFound(key) => tonic::Status::not_found(format!("Key not found: {}", key)),
            KVStoreError::InvalidRequest(msg) => tonic::Status::invalid_argument(msg),
            KVStoreError::Internal(msg) => tonic::Status::internal(msg),
            KVStoreError::Utf8(e) => tonic::Status::internal(format!("Encoding error: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = KVStoreError::KeyNotFound("test_key".to_string());
        assert_eq!(error.to_string(), "Key not found: test_key");

        let error = KVStoreError::Unauthorized("Invalid token".to_string());
        assert_eq!(error.to_string(), "Authentication failed: Invalid token");
    }

    #[test]
    fn test_error_into_status_code() {
        let error = KVStoreError::KeyNotFound("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let error = KVStoreError::Unauthorized("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
