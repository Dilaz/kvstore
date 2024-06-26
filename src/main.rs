use axum::{
    extract::{Path, State}, http::{StatusCode, HeaderMap, header, Request}, middleware::{Next, from_fn_with_state}, response::Response, routing::get, Extension, Json, Router
};
use axum_macros::debug_handler;
use redis::{aio::ConnectionManager, ToRedisArgs};
use serde::Deserialize;
use std::net::{SocketAddr, Ipv4Addr};
use tower_http::compression::CompressionLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const REDIS_TOKENS_TABLE: &str = "tokens";
const PORT: u16 = 3000;

#[tokio::main]
async fn main() {
    tracing::info!("Starting!");
    // initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kvstore=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Creating redis client..");

    // Get client url from env
    let client = match redis::Client::open(std::env::var("REDIS_URL").unwrap_or("redis://127.0.0.1:6379".to_string())) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to connect to redis: {}", e);
            return;
        }
    };

    tracing::info!("Creating connection manager..");
    let connection_manager = ConnectionManager::new(client).await;

    if connection_manager.is_err() {
        tracing::error!("Failed to connect to redis: {}", connection_manager.err().unwrap());
        return;
    }

    let connection_manager = connection_manager.unwrap();

    // build our application with a route
    let app = app(&connection_manager);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app(connection_manager: &ConnectionManager) -> Router {
    return Router::new()
    // `GET /` goes to `root`
    .route("/healthz", get(healthcheck))
    .route("/:key",
        get(get_key)
        .post(post_value)
        .delete(delete_key)
        .layer(from_fn_with_state(connection_manager.clone(), get_prefix_by_token))
    )
    .layer(CompressionLayer::new())
    .with_state(connection_manager.clone());
}

// basic handler that responds with a static string
#[debug_handler]
async fn get_key(
    Extension(ext): Extension<String>,
    State(mut conn): State<ConnectionManager>,
    Path(key): Path<String>,
) -> (StatusCode, Json<String>) {
    let key = format!("{}:{}", ext, key);
    tracing::info!("GET {}", key);
    if let Ok(resp) = conn.send_packed_command(redis::cmd("GET").arg(key.to_redis_args())).await {
        match resp {
            redis::Value::Nil => return (StatusCode::NOT_FOUND, Json("Key not found".to_string())),
            redis::Value::Data(str) => {
                let resp = String::from_utf8(str).unwrap();
                return (StatusCode::OK, Json(resp));
            }
            _ => {}
        }
    }
    (StatusCode::NOT_FOUND, Json("Key not found".to_string()))
}

async fn post_value(
    Extension(ext): Extension<String>,
    State(mut conn): State<ConnectionManager>,
    Path(key): Path<String>,
    Json(payload): Json<SetValue>,
) -> (StatusCode, Json<String>) {
    let key = format!("{}:{}", ext, key);
    tracing::info!("SET {}", key);
    match conn.send_packed_command(redis::cmd("SET").arg(key.to_redis_args()).arg(payload.value.to_redis_args())).await {
        Ok(_) => return (StatusCode::OK, Json("Ok".to_string())),
        Err(err) => {
            tracing::error!("Failed to set key: {}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()))
        },

    }
}

async fn delete_key(
    Extension(ext): Extension<String>,
    State(mut conn): State<ConnectionManager>,
    Path(key): Path<String>,
) -> (StatusCode, Json<String>) {
    let key = format!("{}:{}", ext, key);
    tracing::info!("DELETE {}", key);
    match conn.send_packed_command(redis::cmd("DEL").arg(key.to_redis_args())).await {
        Ok(redis::Value::Okay) => return (StatusCode::OK, Json("Ok".to_string())),
        Ok(_) => return (StatusCode::OK, Json("OK".to_string())),
        Err(err) => {
            tracing::error!("Failed to set key: {}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()));
        },
    }
}

async fn healthcheck(State(mut conn): State<ConnectionManager>) -> (StatusCode, Json<String>) {
    match conn.send_packed_command(&redis::cmd("PING")).await {
        Ok(redis::Value::Status(val)) if val == "PONG" => return (StatusCode::OK, Json("Ok".to_string())),
        Ok(value) => { 
            tracing::error!("Status check failed: {:?}", value);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()))
        },
        Err(err) => {
            tracing::error!("Status check failed: {}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()));
        },
    }
}

async fn get_prefix_by_token<B>(
    State(mut conn): State<ConnectionManager>,
    headers: HeaderMap,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(authorize_header) = headers.get(header::AUTHORIZATION) {
        let token: String = authorize_header.to_str().unwrap_or("").split(" ").last().unwrap_or("").to_string();
        match conn.send_packed_command(redis::cmd("SISMEMBER").arg(REDIS_TOKENS_TABLE.to_redis_args()).arg(&token.to_redis_args())).await {
            Ok(redis::Value::Int(n)) if n == 1 => {
                let mut request = request;
                request.extensions_mut().insert(token);
                return Ok(next.run(request).await)
            },
            Ok(_) => return Err(StatusCode::UNAUTHORIZED),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}


#[derive(Deserialize)]
struct SetValue {
    value: String,
}
