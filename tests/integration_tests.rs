//! Integration tests for KVStore
//!
//! These tests require a running Redis instance at redis://127.0.0.1:6379

use kvstore::{create_grpc_server, create_http_server, KVStore};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

mod http_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt; // for `oneshot`

    async fn setup_store() -> KVStore {
        let store = KVStore::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");

        // Add a test token
        let mut conn = store.connection_manager();
        redis::cmd("SADD")
            .arg("tokens")
            .arg("test-token")
            .query_async::<()>(&mut conn)
            .await
            .expect("Failed to add test token");

        store
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_http_healthcheck() {
        let store = setup_store().await;
        let app = create_http_server(store);

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
    async fn test_http_unauthorized() {
        let store = setup_store().await;
        let app = create_http_server(store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/test-key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_http_set_and_get() {
        let store = setup_store().await;
        let app = create_http_server(store.clone());

        // Set a value
        let set_body = json!({"value": "test-value"}).to_string();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/test-key-http")
                    .header("Authorization", "Bearer test-token")
                    .header("Content-Type", "application/json")
                    .body(Body::from(set_body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Get the value
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/test-key-http")
                    .header("Authorization", "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Clean up
        store.delete("test-token", "test-key-http").await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_http_delete() {
        let store = setup_store().await;

        // Set a value first
        store
            .set("test-token", "test-key-del-http", "test-value", None)
            .await
            .unwrap();

        let app = create_http_server(store.clone());

        // Delete the value
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/test-key-del-http")
                    .header("Authorization", "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify it's deleted
        let result = store.get("test-token", "test-key-del-http").await;
        assert!(result.is_err());
    }
}

mod grpc_tests {
    use super::*;
    use kvstore::grpc::kv_store::{
        kv_store_client::KvStoreClient, DeleteRequest, GetRequest, HealthCheckRequest, SetRequest,
    };
    use tonic::transport::Channel;

    async fn setup_grpc_test() -> (KVStore, tokio::task::JoinHandle<()>, u16) {
        let store = KVStore::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");

        // Add a test token
        let mut conn = store.connection_manager();
        redis::cmd("SADD")
            .arg("tokens")
            .arg("grpc-test-token")
            .query_async::<()>(&mut conn)
            .await
            .expect("Failed to add test token");

        // Start gRPC server on a random available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind to port");
        let addr = listener.local_addr().expect("Failed to get local address");
        let port = addr.port();

        let store_clone = store.clone();
        let service = create_grpc_server(store_clone);

        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        // Give the server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        (store, handle, port)
    }

    async fn create_client(port: u16) -> KvStoreClient<Channel> {
        KvStoreClient::connect(format!("http://127.0.0.1:{}", port))
            .await
            .expect("Failed to connect to gRPC server")
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_grpc_health_check() {
        let (_store, _handle, port) = setup_grpc_test().await;
        let mut client = create_client(port).await;

        let response = client.health_check(HealthCheckRequest {}).await.unwrap();

        assert!(response.get_ref().healthy);
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_grpc_set_and_get() {
        let (store, _handle, port) = setup_grpc_test().await;
        let mut client = create_client(port).await;

        // Set a value
        let set_response = client
            .set(SetRequest {
                key: "grpc-test-key".to_string(),
                value: "grpc-test-value".to_string(),
                token: "grpc-test-token".to_string(),
                ttl_seconds: None,
            })
            .await
            .unwrap();

        assert!(set_response.get_ref().success);

        // Get the value
        let get_response = client
            .get(GetRequest {
                key: "grpc-test-key".to_string(),
                token: "grpc-test-token".to_string(),
            })
            .await
            .unwrap();

        assert!(get_response.get_ref().found);
        assert_eq!(get_response.get_ref().value, "grpc-test-value");

        // Clean up
        store
            .delete("grpc-test-token", "grpc-test-key")
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_grpc_delete() {
        let (store, _handle, port) = setup_grpc_test().await;

        // Set a value first
        store
            .set("grpc-test-token", "grpc-test-key-del", "test-value", None)
            .await
            .unwrap();

        let mut client = create_client(port).await;

        // Delete the value
        let delete_response = client
            .delete(DeleteRequest {
                key: "grpc-test-key-del".to_string(),
                token: "grpc-test-token".to_string(),
            })
            .await
            .unwrap();

        assert!(delete_response.get_ref().success);

        // Verify it's deleted
        let result = store.get("grpc-test-token", "grpc-test-key-del").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_grpc_unauthorized() {
        let (_store, _handle, port) = setup_grpc_test().await;
        let mut client = create_client(port).await;

        // Try to get with invalid token
        let result = client
            .get(GetRequest {
                key: "test-key".to_string(),
                token: "invalid-token".to_string(),
            })
            .await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }
}

mod store_tests {
    use super::*;

    async fn setup() -> KVStore {
        let store = KVStore::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");

        // Add a test token
        let mut conn = store.connection_manager();
        redis::cmd("SADD")
            .arg("tokens")
            .arg("store-test-token")
            .query_async::<()>(&mut conn)
            .await
            .expect("Failed to add test token");

        store
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_set_get_delete() {
        let store = setup().await;

        // Set
        store
            .set("store-test-token", "key1", "value1", None)
            .await
            .unwrap();

        // Get
        let value = store.get("store-test-token", "key1").await.unwrap();
        assert_eq!(value, "value1");

        // Delete
        store.delete("store-test-token", "key1").await.unwrap();

        // Verify deletion
        let result = store.get("store-test-token", "key1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_set_with_ttl() {
        let store = setup().await;

        // Set with TTL of 2 seconds
        store
            .set("store-test-token", "ttl-key", "ttl-value", Some(2))
            .await
            .unwrap();

        // Get immediately
        let value = store.get("store-test-token", "ttl-key").await.unwrap();
        assert_eq!(value, "ttl-value");

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Verify expiration
        let result = store.get("store-test-token", "ttl-key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_list_keys() {
        let store = setup().await;

        // Set multiple keys
        store
            .set("store-test-token", "list:key1", "value1", None)
            .await
            .unwrap();
        store
            .set("store-test-token", "list:key2", "value2", None)
            .await
            .unwrap();
        store
            .set("store-test-token", "other:key", "value3", None)
            .await
            .unwrap();

        // List with prefix
        let keys = store.list("store-test-token", "list:").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"list:key1".to_string()));
        assert!(keys.contains(&"list:key2".to_string()));

        // Clean up
        store.delete("store-test-token", "list:key1").await.unwrap();
        store.delete("store-test-token", "list:key2").await.unwrap();
        store.delete("store-test-token", "other:key").await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_validate_token() {
        let store = setup().await;

        // Valid token
        let is_valid = store.validate_token("store-test-token").await.unwrap();
        assert!(is_valid);

        // Invalid token
        let is_valid = store.validate_token("invalid-token-xyz").await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_health_check() {
        let store = setup().await;
        let healthy = store.health_check().await.unwrap();
        assert!(healthy);
    }
}
