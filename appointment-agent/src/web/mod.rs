use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};

use crate::agent::AgentEvent;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Option<Config>>>,
    pub log_tx: broadcast::Sender<AgentEvent>,
    pub runs: Arc<RwLock<Vec<RunInfo>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub id: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub config: Option<Config>,
    pub loaded: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunsResponse {
    pub runs: Vec<RunInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogsResponse {
    pub logs: Vec<AgentEvent>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/config", get(get_config).put(update_config))
        .route("/api/runs", get(get_runs))
        .route("/api/logs", get(get_logs))
        .route("/api/ws", get(websocket_handler))
        .with_state(state)
        .layer(cors)
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    match &*config {
        Some(c) => Json(ConfigResponse {
            config: Some(c.clone()),
            loaded: true,
        }),
        None => Json(ConfigResponse {
            config: None,
            loaded: false,
        }),
    }
}

async fn update_config(
    State(state): State<AppState>,
    Json(new_config): Json<Config>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;
    *config = Some(new_config);
    Json(ConfigResponse {
        config: (*config).clone(),
        loaded: true,
    })
}

async fn get_runs(State(state): State<AppState>) -> impl IntoResponse {
    let runs = state.runs.read().await;
    Json(RunsResponse {
        runs: runs.clone(),
    })
}

async fn get_logs(State(_state): State<AppState>) -> impl IntoResponse {
    // In a real implementation, we'd store logs in memory or retrieve from a buffer
    Json(LogsResponse { logs: vec![] })
}

async fn websocket_handler(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // WebSocket upgrade handling would go here
    // For now, return a simple response
    StatusCode::OK
}

pub async fn run_webserver(
    config: Option<Config>,
    log_tx: broadcast::Sender<AgentEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        config: Arc::new(RwLock::new(config)),
        log_tx,
        runs: Arc::new(RwLock::new(Vec::new())),
    };

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("Web server listening on port 8080");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
