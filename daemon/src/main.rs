mod config;
mod state;

use anyhow::Result;
use tracing::info;

use crate::config::load_config;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Agent Deck daemon starting...");

    let config = load_config()?;
    info!("Config loaded: {:?}", config);

    let state = new_shared_state();
    load_state(&state).await?;

    let s = state.read().await;
    info!(
        "State loaded: {} sessions, {} groups",
        s.sessions.len(),
        s.groups.len()
    );

    Ok(())
}
