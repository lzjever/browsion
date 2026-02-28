//! WebSocket broadcast server for real-time browser events.
//!
//! Replaces polling in MonitorPage: browser status, action log entries,
//! and profile changes are pushed to all connected clients.

use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Maximum number of events to buffer per client.
const CHANNEL_CAPACITY: usize = 100;

/// WebSocket event types pushed to clients.
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    /// Browser launched or killed.
    BrowserStatusChanged {
        profile_id: String,
        running: bool,
    },
    /// New action log entry.
    ActionLogEntry {
        id: String,
        ts: u64,
        profile_id: String,
        tool: String,
        duration_ms: u64,
        success: bool,
        error: Option<String>,
    },
    /// Profile added/updated/deleted.
    ProfilesChanged,
    /// Heartbeat (sent every 30s to keep connection alive).
    Heartbeat,
}

/// Shared broadcast sender for WebSocket events.
#[derive(Clone)]
pub struct WsBroadcaster {
    tx: broadcast::Sender<WsEvent>,
}

impl WsBroadcaster {
    /// Create a new broadcaster.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Broadcast an event to all connected clients.
    pub fn broadcast(&self, event: WsEvent) {
        let _ = self.tx.send(event);
    }

    /// Subscribe to events (returns a receiver for a new client).
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.tx.subscribe()
    }
}

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket upgrade handler for `/api/ws`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    // Split into sink and stream
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut rx = state.ws_broadcaster.subscribe();

    // Spawn task to send events to client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break; // Client disconnected
                }
            }
        }
    });

    // Handle incoming messages (client can send ping)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Pong(_) => {}
                Message::Ping(data) => {
                    // Respond to ping with pong
                    // Need to access sender here, but it's in the other task
                    // Axum handles ping/pong automatically
                    let _ = data;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
