use criterion::{criterion_group, criterion_main, Criterion};
use kvstore::{create_grpc_server, create_http_server, KVStore};
use kvstore_client::{
    connect, generated::DeleteRequest, generated::GetRequest, generated::SetRequest,
};
use redis::AsyncCommands;
use reqwest::Client;
use std::hint::black_box;
use std::time::Duration;
use tokio::runtime::Runtime;
use tonic::Request;

async fn setup_store() -> KVStore {
    // For benchmarks, we can use a real Redis instance.
    // Ensure Redis is running at this address.
    KVStore::new("redis://127.0.0.1:6379").await.unwrap()
}

fn library_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("library");
    group.measurement_time(Duration::from_secs(5));

    let runtime = Runtime::new().unwrap();
    let store = runtime.block_on(setup_store());
    let token = "benchmark_token";
    {
        let mut conn = store.connection_manager();
        runtime.block_on(async {
            let _: usize = conn.sadd("tokens", token).await.unwrap();
        });
    }

    let key = "benchmark_key";
    let value = "benchmark_value";

    group.bench_function("set", |b| {
        let store = store.clone();
        b.iter(|| {
            runtime
                .block_on(store.set(black_box(token), black_box(key), black_box(value), None))
                .unwrap();
        })
    });

    group.bench_function("get", |b| {
        let store = store.clone();
        b.iter(|| {
            runtime
                .block_on(store.get(black_box(token), black_box(key)))
                .unwrap();
        })
    });

    group.bench_function("delete", |b| {
        let store = store.clone();
        b.iter(|| {
            runtime
                .block_on(store.delete(black_box(token), black_box(key)))
                .unwrap();
        })
    });

    group.finish();
}

fn server_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let store = rt.block_on(setup_store());
    let token = "benchmark_token_server";
    {
        let mut conn = store.connection_manager();
        rt.block_on(async {
            let _: usize = conn.sadd("tokens", token).await.unwrap();
        });
    }

    // Spawn servers in the background
    let http_port = 3030;
    let grpc_port = 50055;

    let http_store = store.clone();
    rt.spawn(async move {
        let app = create_http_server(http_store);
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", http_port))
            .await
            .unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let grpc_store = store.clone();
    rt.spawn(async move {
        let service = create_grpc_server(grpc_store);
        let addr = format!("127.0.0.1:{}", grpc_port).parse().unwrap();
        tonic::transport::Server::builder()
            .add_service(service)
            .serve(addr)
            .await
            .unwrap();
    });

    std::thread::sleep(Duration::from_millis(500));

    // --- HTTP Benchmarks ---
    let mut http_group = c.benchmark_group("http_server");
    http_group.measurement_time(Duration::from_secs(10));
    let client = Client::new();
    let key = "http_key";
    let value = serde_json::json!({ "value": "http_value" });
    let http_url = format!("http://127.0.0.1:{}", http_port);
    let http_set_url = format!("{}/{}", http_url, key);
    let http_get_url = http_set_url.clone();
    let http_delete_url = http_set_url.clone();

    http_group.bench_function("set", |b| {
        let client = client.clone();
        let url = http_set_url.clone();
        b.iter(|| {
            rt.block_on(async {
                client
                    .post(&url)
                    .bearer_auth(token)
                    .json(&value)
                    .send()
                    .await
                    .unwrap();
            });
        });
    });

    http_group.bench_function("get", |b| {
        let client = client.clone();
        let url = http_get_url.clone();
        b.iter(|| {
            rt.block_on(async {
                client.get(&url).bearer_auth(token).send().await.unwrap();
            });
        });
    });

    http_group.bench_function("delete", |b| {
        let client = client.clone();
        let url = http_delete_url.clone();
        b.iter(|| {
            rt.block_on(async {
                client.delete(&url).bearer_auth(token).send().await.unwrap();
            });
        });
    });

    http_group.finish();

    // --- gRPC Benchmarks ---
    let mut grpc_group = c.benchmark_group("grpc_server");
    grpc_group.measurement_time(Duration::from_secs(10));
    let endpoint = format!("http://127.0.0.1:{}", grpc_port);
    let grpc_key = "grpc_key".to_string();
    let grpc_value = "grpc_value".to_string();

    grpc_group.bench_function("set", |b| {
        let mut client = rt.block_on(async {
            let mut attempts = 0;
            loop {
                match connect(&endpoint).await {
                    Ok(client) => break client,
                    Err(e) => {
                        attempts += 1;
                        if attempts > 20 {
                            panic!("connect gRPC client: {e}");
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
        let key = grpc_key.clone();
        let value = grpc_value.clone();
        let bearer = token.to_string();
        b.iter(|| {
            rt.block_on(async {
                let request = Request::new(SetRequest {
                    token: bearer.clone(),
                    key: key.clone(),
                    value: value.clone(),
                    ttl_seconds: None,
                });
                client.set(request).await.unwrap();
            });
        });
    });

    grpc_group.bench_function("get", |b| {
        let mut client = rt.block_on(async {
            let mut attempts = 0;
            loop {
                match connect(&endpoint).await {
                    Ok(client) => break client,
                    Err(e) => {
                        attempts += 1;
                        if attempts > 20 {
                            panic!("connect gRPC client: {e}");
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
        let key = grpc_key.clone();
        let bearer = token.to_string();
        b.iter(|| {
            rt.block_on(async {
                let request = Request::new(GetRequest {
                    token: bearer.clone(),
                    key: key.clone(),
                });
                client.get(request).await.unwrap();
            });
        });
    });

    grpc_group.bench_function("delete", |b| {
        let mut client = rt.block_on(async {
            let mut attempts = 0;
            loop {
                match connect(&endpoint).await {
                    Ok(client) => break client,
                    Err(e) => {
                        attempts += 1;
                        if attempts > 20 {
                            panic!("connect gRPC client: {e}");
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
        let key = grpc_key.clone();
        let bearer = token.to_string();
        b.iter(|| {
            rt.block_on(async {
                let request = Request::new(DeleteRequest {
                    token: bearer.clone(),
                    key: key.clone(),
                });
                client.delete(request).await.unwrap();
            });
        });
    });

    grpc_group.finish();
}

criterion_group!(benches, library_benchmarks, server_benchmarks);
criterion_main!(benches);
