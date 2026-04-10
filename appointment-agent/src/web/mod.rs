/*
 * Version: 0.1.3
 * Description: Dashboard API compatible with Axum 0.8.
 */

use axum::{
    extract::{State, ws::{Message, WebSocket, WebSocketUpgrade}},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<tokio::sync::RwLock<Config>>,
    pub config_path: String,
    pub logs: Arc<DashMap<String, Vec<serde_json::Value>>>,
    pub tx: broadcast::Sender<serde_json::Value>,
}

pub struct WebServer;

impl WebServer {
    pub async fn run(config: Config, config_path: String) -> anyhow::Result<()> {
        let (tx, _rx) = broadcast::channel(100);
        let state = AppState {
            config: Arc::new(tokio::sync::RwLock::new(config)),
            config_path,
            logs: Arc::new(DashMap::new()),
            tx,
        };

        let app = Router::new()
            .route("/api/config", get(get_config).put(update_config))
            .route("/api/runs", get(get_runs))
            .route("/api/logs", get(get_logs))
            .route("/api/ws", get(ws_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
        info!("Web dashboard running on http://0.0.0.0:8080");
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await;
    Json(cfg.clone())
}

async fn update_config(State(state): State<AppState>, Json(new_cfg): Json<Config>) -> impl IntoResponse {
    let mut cfg = state.config.write().await;
    *cfg = new_cfg;
    if let Err(e) = cfg.save(&state.config_path) {
        error!("Failed to save config: {}", e);
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to save config").into_response();
    }
    Json(cfg.clone()).into_response()
}

async fn get_runs(State(state): State<AppState>) -> impl IntoResponse {
    let run_ids: Vec<String> = state.logs.iter().map(|r| r.key().clone()).collect();
    Json(run_ids)
}

async fn get_logs(State(state): State<AppState>) -> impl IntoResponse {
    let all_logs: Vec<serde_json::Value> = state.logs.iter().flat_map(|r| r.value().clone()).collect();
    Json(all_logs)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        let payload = match serde_json::to_string(&msg) {
            Ok(p) => p,
            Err(_) => continue,
        };
        // Axum 0.8 uses into() for Message::Text
        if socket.send(Message::Text(payload.into())).await.is_err() {
            break;
        }
    }
}

pub fn broadcast_log(state: &AppState, log_event: serde_json::Value) {
    if let Some(run_id) = log_event.get("run_id").and_then(|v| v.as_str()) {
        let mut entry = state.logs.entry(run_id.to_string()).or_insert_with(Vec::new);
        entry.push(log_event.clone());
    }
    let _ = state.tx.send(log_event);
}
