// Hook listener - receives status events from Claude Code hooks
// Provides authoritative status information via Unix socket

use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Event sent by Claude hooks
#[derive(Debug, Clone, Deserialize)]
pub struct HookEvent {
    /// The Agent Deck session ID
    pub session_id: String,
    /// State reported by the hook (waiting, running, idle)
    pub state: String,
    /// The hook event type (tool_approval, tool_complete, stopped)
    pub event: String,
    /// Unix timestamp when the event occurred
    pub ts: u64,
}

/// Listens for hook events on a Unix socket
pub struct HookListener {
    socket_path: PathBuf,
}

impl HookListener {
    /// Create a new hook listener
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Start listening for hook events
    /// Events are sent to the provided channel
    pub async fn run(&self, tx: mpsc::Sender<HookEvent>) -> Result<()> {
        // Remove existing socket file if present
        let _ = std::fs::remove_file(&self.socket_path);

        let listener = UnixListener::bind(&self.socket_path)?;
        info!("Hook listener started on {:?}", self.socket_path);

        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    let tx = tx.clone();

                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 1024];
                        match stream.read(&mut buf).await {
                            Ok(0) => {
                                // Connection closed
                            }
                            Ok(n) => {
                                let data = &buf[..n];
                                match serde_json::from_slice::<HookEvent>(data) {
                                    Ok(event) => {
                                        debug!(
                                            "Hook event: session={} state={} event={}",
                                            event.session_id, event.state, event.event
                                        );
                                        if tx.send(event).await.is_err() {
                                            warn!("Hook event channel closed");
                                        }
                                    }
                                    Err(e) => {
                                        // Try to parse as string for debugging
                                        let text = String::from_utf8_lossy(data);
                                        debug!(
                                            "Failed to parse hook event: {} - data: {}",
                                            e, text
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Hook connection read error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Hook listener accept error: {}", e);
                }
            }
        }
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_deserialize() {
        let json = r#"{"session_id":"abc-123","state":"waiting","event":"tool_approval","ts":1704067200}"#;
        let event: HookEvent = serde_json::from_str(json).unwrap();

        assert_eq!(event.session_id, "abc-123");
        assert_eq!(event.state, "waiting");
        assert_eq!(event.event, "tool_approval");
        assert_eq!(event.ts, 1704067200);
    }
}
