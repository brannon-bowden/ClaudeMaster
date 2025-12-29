mod config;
mod ipc;
mod pty;
mod state;

use anyhow::Result;
use shared::Event;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::{get_socket_path, load_config};
use crate::ipc::start_server;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Agent Deck daemon starting...");

    let config = load_config()?;
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

    start_server(&socket_path, state, event_tx).await?;

    Ok(())
}
