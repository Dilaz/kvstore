//! gRPC server implementation
//!
//! Provides gRPC service for KVStore operations.

use crate::{KVStore, KVStoreError};
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

// Include generated protobuf code
pub mod kv_store {
    tonic::include_proto!("kvstore");
}

pub const KVSTORE_FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("kvstore_descriptor");

pub use kv_store::kv_store_server;
pub use kv_store::kv_store_server::KvStoreServer;

/// gRPC service implementation
pub struct KVStoreService {
    store: KVStore,
}

impl KVStoreService {
    /// Create a new gRPC service
    pub fn new(store: KVStore) -> Self {
        Self { store }
    }

    /// Validate a request token
    async fn validate_request_token(&self, token: &str) -> Result<(), Status> {
        let is_valid = self
            .store
            .validate_token(token)
            .await
            .map_err(|e| Status::internal(format!("Token validation failed: {}", e)))?;

        if !is_valid {
            return Err(Status::unauthenticated("Invalid token"));
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl kv_store::kv_store_server::KvStore for KVStoreService {
    async fn get(
        &self,
        request: Request<kv_store::GetRequest>,
    ) -> Result<Response<kv_store::GetResponse>, Status> {
        let req = request.into_inner();

        tracing::info!(
            "gRPC GET {} (token: {})",
            req.key,
            &req.token[..req
                .token
                .char_indices()
                .nth(8)
                .map_or(req.token.len(), |(i, _)| i)]
        );

        // Validate token
        self.validate_request_token(&req.token).await?;

        // Get the value
        match self.store.get(&req.token, &req.key).await {
            Ok(value) => Ok(Response::new(kv_store::GetResponse { value, found: true })),
            Err(KVStoreError::KeyNotFound(_)) => Ok(Response::new(kv_store::GetResponse {
                value: String::new(),
                found: false,
            })),
            Err(e) => Err(Status::from(e)),
        }
    }

    async fn set(
        &self,
        request: Request<kv_store::SetRequest>,
    ) -> Result<Response<kv_store::SetResponse>, Status> {
        let req = request.into_inner();

        tracing::info!(
            "gRPC SET {} (token: {}, TTL: {:?})",
            req.key,
            &req.token[..req
                .token
                .char_indices()
                .nth(8)
                .map_or(req.token.len(), |(i, _)| i)],
            req.ttl_seconds
        );

        // Validate token
        self.validate_request_token(&req.token).await?;

        // Set the value
        self.store
            .set(&req.token, &req.key, &req.value, req.ttl_seconds)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(kv_store::SetResponse {
            success: true,
            message: "OK".to_string(),
        }))
    }

    async fn delete(
        &self,
        request: Request<kv_store::DeleteRequest>,
    ) -> Result<Response<kv_store::DeleteResponse>, Status> {
        let req = request.into_inner();

        tracing::info!(
            "gRPC DELETE {} (token: {})",
            req.key,
            &req.token[..req
                .token
                .char_indices()
                .nth(8)
                .map_or(req.token.len(), |(i, _)| i)]
        );

        // Validate token
        self.validate_request_token(&req.token).await?;

        // Delete the value
        self.store
            .delete(&req.token, &req.key)
            .await
            .map_err(Status::from)?;

        Ok(Response::new(kv_store::DeleteResponse {
            success: true,
            message: "OK".to_string(),
        }))
    }

    async fn health_check(
        &self,
        _request: Request<kv_store::HealthCheckRequest>,
    ) -> Result<Response<kv_store::HealthCheckResponse>, Status> {
        tracing::debug!("gRPC health check");

        let healthy = self
            .store
            .health_check()
            .await
            .map_err(|e| Status::internal(format!("Health check failed: {}", e)))?;

        Ok(Response::new(kv_store::HealthCheckResponse {
            healthy,
            message: if healthy {
                "OK".to_string()
            } else {
                "Unhealthy".to_string()
            },
        }))
    }

    type ListStream = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<kv_store::ListResponse, Status>> + Send>>;

    async fn list(
        &self,
        request: Request<kv_store::ListRequest>,
    ) -> Result<Response<Self::ListStream>, Status> {
        let req = request.into_inner();

        tracing::info!(
            "gRPC LIST {} (token: {})",
            req.prefix,
            &req.token[..req
                .token
                .char_indices()
                .nth(8)
                .map_or(req.token.len(), |(i, _)| i)]
        );

        // Validate token
        self.validate_request_token(&req.token).await?;

        // List keys - get a stream
        let key_stream = self
            .store
            .list(&req.token, &req.prefix)
            .await
            .map_err(Status::from)?;

        // Map the stream to gRPC responses
        let response_stream = key_stream.map(|key| Ok(kv_store::ListResponse { key }));

        Ok(Response::new(Box::pin(response_stream)))
    }
}

/// Create a gRPC service from a KVStore
pub fn create_service(store: KVStore) -> KvStoreServer<KVStoreService> {
    KvStoreServer::new(KVStoreService::new(store))
}

/// Create a gRPC reflection service for the KVStore API
pub fn create_reflection_service(
) -> std::result::Result<impl Clone, tonic_reflection::server::Error> {
    tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(KVSTORE_FILE_DESCRIPTOR_SET)
        .build_v1()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_service_creation() {
        // This is a basic smoke test
        // Integration tests would require a running Redis instance
    }
}
