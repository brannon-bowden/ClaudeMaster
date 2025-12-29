//! Event listener for streaming events from daemon to frontend

use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericFilePath,
};
use serde::Serialize;
use shared::{get_socket_path, Event};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info, warn};

/// Connection state payload for frontend
#[derive(Clone, Serialize)]
pub struct ConnectionState {
    pub connected: bool,
    pub error: Option<String>,
}

/// Emit connection state to frontend
fn emit_connection_state(app: &AppHandle, connected: bool, error: Option<String>) {
    let state = ConnectionState { connected, error };
    if let Err(e) = app.emit("daemon:connection_state", &state) {
        error!("Failed to emit connection state: {}", e);
    }
}

/// Start the event listener in a background task
/// This creates a separate connection to the daemon for receiving events
pub fn start_event_listener(app: AppHandle) {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Use Tauri's async runtime to spawn the task
    tauri::async_runtime::spawn(async move {
        let mut reconnect_attempts = 0u32;
        let max_backoff = 30; // Maximum 30 seconds between attempts

        while running_clone.load(Ordering::Relaxed) {
            match run_event_loop(&app).await {
                Ok(()) => {
                    info!("Event loop ended normally");
                    emit_connection_state(&app, false, None);
                    reconnect_attempts = 0;
                }
                Err(e) => {
                    warn!("Event loop error: {}, reconnecting...", e);
                    emit_connection_state(&app, false, Some(e.clone()));

                    // Exponential backoff with cap
                    let delay = std::cmp::min(2u64.pow(reconnect_attempts), max_backoff);
                    reconnect_attempts = reconnect_attempts.saturating_add(1);

                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                }
            }
        }
    });
}

async fn run_event_loop(app: &AppHandle) -> Result<(), String> {
    let socket_path = get_socket_path().map_err(|e| e.to_string())?;

    // Wait for socket to exist with timeout
    let mut wait_attempts = 0;
    while !socket_path.exists() {
        wait_attempts += 1;
        if wait_attempts > 60 {
            // 30 seconds max wait
            return Err("Daemon socket not found after 30s".to_string());
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    let name = socket_path
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| e.to_string())?;

    let stream = Stream::connect(name)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    info!("Event listener connected to daemon");
    emit_connection_state(app, true, None);

    let (recv_half, _send_half) = stream.split();
    let mut reader = BufReader::new(recv_half);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // Connection closed
                return Err("Connection closed".to_string());
            }
            Ok(_) => {
                // Try to parse as Event
                if let Ok(event) = serde_json::from_str::<Event>(&line) {
                    // Emit to frontend
                    if let Err(e) = app.emit(&event.event, &event.data) {
                        error!("Failed to emit event: {}", e);
                    }
                }
                // Ignore responses (they have "id" field) - those are handled by the command connection
            }
            Err(e) => {
                return Err(format!("Read error: {}", e));
            }
        }
    }
}
