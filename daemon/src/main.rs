mod claude;
mod claude_resolver;
mod config;
mod hook_listener;
mod hook_manager;
mod ipc;
mod pty;
mod session_manager;
mod state;
mod status_tracker;

use anyhow::Result;
use shared::Event;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::config::{get_socket_path, load_config};
use crate::hook_listener::HookListener;
use crate::hook_manager::HookManager;
use crate::ipc::{start_server, IpcContext};
use crate::session_manager::SessionManager;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with sensible defaults
    // Default to info level if RUST_LOG is not set
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Claude Master daemon starting...");

    let _config = load_config()?;
    info!("Config loaded");

    let state = new_shared_state();
    load_state(&state).await?;

    {
        let s = state.read().await;
        info!(
            "State loaded: {} sessions, {} groups",
            s.sessions.len(),
            s.groups.len()
        );
    }

    let (event_tx, _) = broadcast::channel::<Event>(100);
    let socket_path = get_socket_path()?;

    // Initialize hook manager and ensure hook script is installed
    let hook_manager = Arc::new(HookManager::init()?);
    if let Err(e) = hook_manager.ensure_hook_script() {
        warn!("Failed to install hook script: {}", e);
    } else {
        info!("Hook script installed at {:?}", hook_manager.hooks_dir());
    }

    // Create session manager with hook manager
    let (session_manager, output_rx) =
        SessionManager::new(state.clone(), event_tx.clone(), hook_manager.clone());

    // Create shutdown flag for graceful termination
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    // Create IPC context
    let ctx = Arc::new(IpcContext {
        state: state.clone(),
        pty_manager: session_manager.pty_manager(),
        output_tx: session_manager.output_tx(),
        event_tx: event_tx.clone(),
        shutdown_flag,
        hook_manager: hook_manager.clone(),
    });

    // Start hook listener for authoritative status events
    let (hook_tx, hook_rx) = mpsc::channel(100);
    let hook_listener = HookListener::new(hook_manager.socket_path().clone());
    tokio::spawn(async move {
        if let Err(e) = hook_listener.run(hook_tx).await {
            error!("Hook listener error: {}", e);
        }
    });

    // Spawn session manager to handle PTY output and hook events
    tokio::spawn(async move {
        session_manager.run(output_rx, hook_rx).await;
    });

    // Start IPC server (blocks forever)
    start_server(&socket_path, ctx).await?;

    Ok(())
}
