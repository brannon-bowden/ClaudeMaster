//! Daemon launcher - spawns the daemon sidecar on app startup

use std::sync::atomic::{AtomicBool, Ordering};
use tauri_plugin_shell::ShellExt;
use tracing::{error, info, warn};

static DAEMON_RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the daemon sidecar if not already running
pub fn start_daemon(app: &tauri::AppHandle) {
    if DAEMON_RUNNING.load(Ordering::Relaxed) {
        info!("Daemon already running");
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match spawn_daemon(&app).await {
            Ok(()) => {
                DAEMON_RUNNING.store(true, Ordering::Relaxed);
                info!("Daemon started successfully");
            }
            Err(e) => {
                error!("Failed to start daemon: {}", e);
            }
        }
    });
}

async fn spawn_daemon(app: &tauri::AppHandle) -> Result<(), String> {
    let shell = app.shell();

    // Spawn the sidecar
    let sidecar = shell
        .sidecar("agent-deck-daemon")
        .map_err(|e| format!("Failed to create sidecar command: {}", e))?;

    let (mut rx, _child) = sidecar
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

    // Log daemon output in background
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_shell::process::CommandEvent;

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    info!("[daemon] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    warn!("[daemon] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Terminated(status) => {
                    DAEMON_RUNNING.store(false, Ordering::Relaxed);
                    info!("Daemon terminated with status: {:?}", status);
                    break;
                }
                CommandEvent::Error(e) => {
                    error!("Daemon error: {}", e);
                }
                _ => {}
            }
        }
    });

    // Give daemon time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    Ok(())
}

/// Check if daemon is running
#[allow(dead_code)]
pub fn is_daemon_running() -> bool {
    DAEMON_RUNNING.load(Ordering::Relaxed)
}
