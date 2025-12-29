use anyhow::Result;
use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericFilePath, ListenerOptions,
};
use shared::{ErrorInfo, Event, Request, Response};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::state::SharedState;

pub type EventSender = broadcast::Sender<Event>;

pub async fn start_server(
    socket_path: &Path,
    state: SharedState,
    event_tx: EventSender,
) -> Result<()> {
    // Remove existing socket if present
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let name = socket_path.to_fs_name::<GenericFilePath>()?;
    let listener = ListenerOptions::new().name(name).create_tokio()?;

    info!("IPC server listening on {:?}", socket_path);

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let state = state.clone();
                let event_tx = event_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state, event_tx).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: Stream,
    state: SharedState,
    event_tx: EventSender,
) -> Result<()> {
    info!("New client connected");

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut event_rx = event_tx.subscribe();

    let mut line = String::new();

    loop {
        tokio::select! {
            // Handle incoming requests
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("Client disconnected");
                        break;
                    }
                    Ok(_) => {
                        let response = process_request(&line, &state).await;
                        let response_json = serde_json::to_string(&response)? + "\n";
                        writer.write_all(response_json.as_bytes()).await?;
                        line.clear();
                    }
                    Err(e) => {
                        error!("Read error: {}", e);
                        break;
                    }
                }
            }

            // Forward events to client
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        let event_json = serde_json::to_string(&event)? + "\n";
                        if let Err(e) = writer.write_all(event_json.as_bytes()).await {
                            warn!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client lagged, missed {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_request(line: &str, state: &SharedState) -> Response {
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            return Response {
                id: 0,
                result: None,
                error: Some(ErrorInfo {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
        }
    };

    match request.method.as_str() {
        "daemon.ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        },

        "session.list" => {
            let s = state.read().await;
            let sessions: Vec<_> = s.sessions.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"sessions": sessions})),
                error: None,
            }
        }

        "group.list" => {
            let s = state.read().await;
            let groups: Vec<_> = s.groups.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"groups": groups})),
                error: None,
            }
        }

        _ => Response {
            id: request.id,
            result: None,
            error: Some(ErrorInfo {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    }
}
