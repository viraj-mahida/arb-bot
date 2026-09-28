//! WebSocket fan-out for the visualizer. Bind failure is logged, not fatal:
//! the trading loop must keep running if the dashboard port is taken.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;

use super::{events_file_path, latest_wallet, recent, subscribe};

pub fn spawn(port: u16) {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/ws", get(websocket))
            .route("/events.jsonl", get(events_file))
            .route("/health", get(health))
            .layer(CorsLayer::permissive());
        let address = format!("127.0.0.1:{port}");
        let listener = match tokio::net::TcpListener::bind(&address).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("[dashboard] could not bind {address}: {error}");
                return;
            }
        };
        println!("[dashboard] Rbot visualizer websocket on ws://{address}/ws");
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[dashboard] server stopped: {error}");
        }
    });
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"ok": true, "bot": "Rbot"}))
}

async fn events_file() -> impl IntoResponse {
    match tokio::fs::read_to_string(events_file_path()).await {
        Ok(body) => (
            axum::http::StatusCode::OK,
            [("content-type", "application/x-ndjson")],
            body,
        )
            .into_response(),
        Err(_) => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

async fn websocket(upgrade: WebSocketUpgrade) -> impl IntoResponse {
    upgrade.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    // Subscribe first so an event published while we replay history is not lost.
    let mut live = match subscribe() {
        Some(receiver) => receiver,
        None => return,
    };
    for line in recent() {
        if socket.send(Message::Text(line.into())).await.is_err() {
            return;
        }
    }
    if let Some(line) = latest_wallet()
        && socket.send(Message::Text(line.into())).await.is_err()
    {
        return;
    }
    loop {
        match live.recv().await {
            Ok(line) => {
                if socket.send(Message::Text(line.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}
