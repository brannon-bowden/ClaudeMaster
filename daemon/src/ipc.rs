use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericFilePath, ListenerOptions,
};
use shared::{
    CreateGroupParams, CreateSessionParams, ErrorInfo, Event, Request, Response,
    SessionIdParams, SessionInputParams, SessionResizeParams,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::pty::PtyManager;
use crate::session_manager::SessionManager;
use crate::state::SharedState;

pub type EventSender = broadcast::Sender<Event>;

pub struct IpcContext {
    pub state: SharedState,
    pub pty_manager: Arc<PtyManager>,
    pub output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
    pub event_tx: EventSender,
}

pub async fn start_server(socket_path: &Path, ctx: Arc<IpcContext>) -> Result<()> {
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
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, ctx).await {
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

async fn handle_connection(stream: Stream, ctx: Arc<IpcContext>) -> Result<()> {
    info!("New client connected");

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut event_rx = ctx.event_tx.subscribe();

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
                        let response = process_request(&line, &ctx).await;
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

async fn process_request(line: &str, ctx: &IpcContext) -> Response {
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
            let s = ctx.state.read().await;
            let sessions: Vec<_> = s.sessions.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"sessions": sessions})),
                error: None,
            }
        }

        "session.create" => {
            let params: CreateSessionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::create_session(
                &ctx.state,
                &ctx.pty_manager,
                ctx.output_tx.clone(),
                &ctx.event_tx,
                params.name,
                PathBuf::from(params.dir),
                params.group_id,
            )
            .await
            {
                Ok(session) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"session": session})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to create session: {}", e),
                    }),
                },
            }
        }

        "session.stop" => {
            let params: SessionIdParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::stop_session(
                &ctx.state,
                &ctx.pty_manager,
                &ctx.event_tx,
                params.session_id,
            )
            .await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to stop session: {}", e),
                    }),
                },
            }
        }

        "session.delete" => {
            let params: SessionIdParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::delete_session(
                &ctx.state,
                &ctx.pty_manager,
                &ctx.event_tx,
                params.session_id,
            )
            .await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to delete session: {}", e),
                    }),
                },
            }
        }

        "session.input" => {
            let params: SessionInputParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            // Try to decode as base64, fall back to raw bytes
            let data = BASE64
                .decode(&params.input)
                .unwrap_or_else(|_| params.input.into_bytes());

            match ctx.pty_manager.write(params.session_id, &data).await {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to write to session: {}", e),
                    }),
                },
            }
        }

        "session.resize" => {
            let params: SessionResizeParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match ctx
                .pty_manager
                .resize(params.session_id, params.rows, params.cols)
                .await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to resize session: {}", e),
                    }),
                },
            }
        }

        "group.list" => {
            let s = ctx.state.read().await;
            let groups: Vec<_> = s.groups.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"groups": groups})),
                error: None,
            }
        }

        "group.create" => {
            let params: CreateGroupParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::create_group(&ctx.state, &ctx.event_tx, params.name, params.parent_id)
                .await
            {
                Ok(group) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"group": group})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to create group: {}", e),
                    }),
                },
            }
        }

        "group.delete" => {
            let params: SessionIdParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            // Reuse session_id field for group_id
            match SessionManager::delete_group(&ctx.state, &ctx.event_tx, params.session_id).await {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to delete group: {}", e),
                    }),
                },
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
