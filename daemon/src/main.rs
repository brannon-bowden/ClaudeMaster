mod claude;
mod config;
mod ipc;
mod pty;
mod session_manager;
mod state;

use anyhow::Result;
use shared::Event;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::{get_socket_path, load_config};
use crate::ipc::{start_server, IpcContext};
use crate::session_manager::SessionManager;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Agent Deck daemon starting...");

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

    // Create session manager
    let (session_manager, output_rx) = SessionManager::new(state.clone(), event_tx.clone());

    // Create IPC context
    let ctx = Arc::new(IpcContext {
        state: state.clone(),
        pty_manager: session_manager.pty_manager(),
        output_tx: session_manager.output_tx(),
        event_tx: event_tx.clone(),
    });

    // Spawn session manager to handle PTY output
    tokio::spawn(async move {
        session_manager.run(output_rx).await;
    });

    // Start IPC server (blocks forever)
    start_server(&socket_path, ctx).await?;

    Ok(())
}
