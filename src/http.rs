//! HTTP server implementation
//!
//! Provides REST API handlers for KVStore operations.

use crate::{error::Result, KVStore, KVStoreError};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

/// Creates a new HTTP router with all routes configured
///
/// The router includes:
/// - GET /healthz - Health check endpoint
/// - GET /{key} - Get a value
/// - POST /{key} - Set a value
/// - DELETE /{key} - Delete a value
///
/// All endpoints except /healthz require Bearer token authentication.
pub fn create_router(store: KVStore) -> Router {
    Router::new()
        .route("/healthz", get(healthcheck))
        .route(
            "/:key",
            get(get_key)
                .post(post_value)
                .delete(delete_key)
                .layer(from_fn_with_state(store.clone(), auth_middleware)),
        )
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(store)
}

/// Request payload for setting a value
#[derive(Debug, Deserialize, Serialize)]
pub struct SetValueRequest {
    /// The value to store
    pub value: String,
    /// Optional TTL in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<i64>,
}

/// Response for successful operations
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

/// Response for get operations
#[derive(Debug, Serialize)]
pub struct GetResponse {
    pub value: String,
}

/// Health check endpoint
///
/// Returns 200 OK if Redis connection is healthy
#[debug_handler]
async fn healthcheck(State(store): State<KVStore>) -> Result<impl IntoResponse> {
    let healthy = store.health_check().await?;

    if healthy {
        Ok((
            StatusCode::OK,
            Json(SuccessResponse {
                message: "OK".to_string(),
            }),
        ))
    } else {
        Err(KVStoreError::Internal("Health check failed".to_string()))
    }
}

/// Get a value by key
///
/// Requires authentication via Bearer token
#[debug_handler]
async fn get_key(
    Extension(token): Extension<String>,
    State(store): State<KVStore>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse> {
    tracing::info!("GET {} (token: {})", key, &token[..token.len().min(8)]);

    let value = store.get(&token, &key).await?;

    Ok((StatusCode::OK, Json(GetResponse { value })))
}

/// Set a value for a key
///
/// Requires authentication via Bearer token
#[debug_handler]
async fn post_value(
    Extension(token): Extension<String>,
    State(store): State<KVStore>,
    Path(key): Path<String>,
    Json(payload): Json<SetValueRequest>,
) -> Result<impl IntoResponse> {
    tracing::info!(
        "SET {} (token: {}, TTL: {:?})",
        key,
        &token[..token.len().min(8)],
        payload.ttl_seconds
    );

    store
        .set(&token, &key, &payload.value, payload.ttl_seconds)
        .await?;

    Ok((
        StatusCode::OK,
        Json(SuccessResponse {
            message: "OK".to_string(),
        }),
    ))
}

/// Delete a value by key
///
/// Requires authentication via Bearer token
#[debug_handler]
async fn delete_key(
    Extension(token): Extension<String>,
    State(store): State<KVStore>,
    Path(key): Path<String>,
) -> Result<impl IntoResponse> {
    tracing::info!("DELETE {} (token: {})", key, &token[..token.len().min(8)]);

    store.delete(&token, &key).await?;

    Ok((
        StatusCode::OK,
        Json(SuccessResponse {
            message: "OK".to_string(),
        }),
    ))
}

/// Authentication middleware
///
/// Extracts and validates the Bearer token from the Authorization header
async fn auth_middleware(
    State(store): State<KVStore>,
    headers: HeaderMap,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response> {
    // Extract token from Authorization header
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| {
            KVStoreError::Unauthorized("Missing or invalid Authorization header".to_string())
        })?;

    // Validate token
    let is_valid = store.validate_token(token).await?;

    if !is_valid {
        return Err(KVStoreError::Unauthorized("Invalid token".to_string()));
    }

    // Add token to request extensions
    request.extensions_mut().insert(token.to_string());

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    // Helper function to create a test store
    async fn create_test_store() -> KVStore {
        // This requires a running Redis instance
        KVStore::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis")
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_healthcheck() {
        let store = create_test_store().await;
        let app = create_router(store);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_unauthorized_access() {
        let store = create_test_store().await;
        let app = create_router(store);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
