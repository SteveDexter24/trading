use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, sync::Arc};
use tokio::net::TcpListener;
use trading_domain::StoragePort;
use trading_storage::{InMemoryStorage, PostgresStorage};

struct ApiState {
    storage: Arc<dyn StoragePort>,
    admin_token: Option<SecretString>,
}

#[derive(Debug, Deserialize)]
struct KillSwitchRequest {
    active: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let storage: Arc<dyn StoragePort> = if let Ok(database_url) = env::var("DATABASE_URL") {
        let postgres = PostgresStorage::connect(&database_url).await?;
        postgres.migrate().await?;
        Arc::new(postgres)
    } else {
        Arc::new(InMemoryStorage::new())
    };
    let state = Arc::new(ApiState {
        storage,
        admin_token: env::var("ADMIN_API_TOKEN").ok().map(SecretString::from),
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/v1/status", get(status))
        .route("/v1/portfolio", get(empty_collection))
        .route("/v1/positions", get(empty_collection))
        .route("/v1/orders", get(orders))
        .route("/v1/fills", get(fills))
        .route("/v1/signals", get(empty_collection))
        .route("/v1/risk-decisions", get(risk_decisions))
        .route("/v1/system/kill-switch", post(set_kill_switch))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!(
        trading_mode = "paper",
        address = "0.0.0.0:8080",
        "read-only API listening"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state.storage.kill_switch_active().await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready" })),
        ),
    }
}

async fn status(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state.storage.kill_switch_active().await {
        Ok(active) => (
            StatusCode::OK,
            Json(json!({
                "trading_mode": "paper",
                "production_submission_enabled": false,
                "kill_switch_active": active,
            })),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        ),
    }
}

async fn empty_collection() -> Json<Value> {
    Json(json!({ "items": [] }))
}

async fn orders(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state.storage.orders().await {
        Ok(orders) => (StatusCode::OK, Json(json!({ "items": orders }))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        ),
    }
}

async fn fills(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state.storage.fills().await {
        Ok(fills) => (StatusCode::OK, Json(json!({ "items": fills }))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        ),
    }
}

async fn risk_decisions(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state.storage.risk_decisions().await {
        Ok(items) => (StatusCode::OK, Json(json!({ "items": items }))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        ),
    }
}

async fn set_kill_switch(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(request): Json<KillSwitchRequest>,
) -> impl IntoResponse {
    let expected = state.admin_token.as_ref().map(ExposeSecret::expose_secret);
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if expected.is_none() || supplied != expected {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "authentication required" })),
        );
    }
    if let Err(error) = state.storage.set_kill_switch(request.active).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        );
    }
    (
        StatusCode::OK,
        Json(json!({ "active": request.active, "trading_mode": "paper" })),
    )
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal");
    }
}
