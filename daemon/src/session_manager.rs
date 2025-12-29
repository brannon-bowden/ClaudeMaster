use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use shared::{Event, Group, PtyOutputData, Session, SessionStatus, StatusChangedData};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info};
use uuid::Uuid;

use crate::claude;
use crate::pty::PtyManager;
use crate::state::{save_state, SharedState};

pub struct SessionManager {
    state: SharedState,
    pty_manager: Arc<PtyManager>,
    event_tx: broadcast::Sender<Event>,
    output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
}

impl SessionManager {
    pub fn new(state: SharedState, event_tx: broadcast::Sender<Event>) -> (Self, mpsc::Receiver<(Uuid, Vec<u8>)>) {
        let (output_tx, output_rx) = mpsc::channel(1000);
        let manager = Self {
            state,
            pty_manager: Arc::new(PtyManager::new()),
            event_tx,
            output_tx,
        };
        (manager, output_rx)
    }

    pub async fn run(self, mut output_rx: mpsc::Receiver<(Uuid, Vec<u8>)>) {
        info!("Session manager started");

        while let Some((session_id, data)) = output_rx.recv().await {
            // Convert to string for status detection (lossy is fine for pattern matching)
            let text = String::from_utf8_lossy(&data);

            // Detect status changes
            if let Some(new_status) = claude::detect_status(&text) {
                self.update_session_status(session_id, new_status).await;
            }

            // Extract Claude session ID if present
            if let Some(claude_session_id) = claude::extract_session_id(&text) {
                self.update_claude_session_id(session_id, claude_session_id).await;
            }

            // Forward output as event
            let output = BASE64.encode(&data);
            let event = Event {
                event: "pty.output".to_string(),
                data: serde_json::to_value(PtyOutputData {
                    session_id,
                    output,
                })
                .unwrap(),
            };
            let _ = self.event_tx.send(event);
        }
    }

    async fn update_session_status(&self, session_id: Uuid, new_status: SessionStatus) {
        let mut status_changed = false;
        {
            let mut s = self.state.write().await;
            if let Some(session) = s.sessions.get_mut(&session_id) {
                if session.status != new_status {
                    debug!("Session {} status: {:?} -> {:?}", session_id, session.status, new_status);
                    session.status = new_status;
                    session.last_activity = Utc::now();
                    status_changed = true;
                }
            }
        }

        if status_changed {
            // Emit status change event
            let event = Event {
                event: "session.status_changed".to_string(),
                data: serde_json::to_value(StatusChangedData {
                    session_id,
                    status: new_status,
                })
                .unwrap(),
            };
            let _ = self.event_tx.send(event);
        }
    }

    async fn update_claude_session_id(&self, session_id: Uuid, claude_session_id: String) {
        let mut s = self.state.write().await;
        if let Some(session) = s.sessions.get_mut(&session_id) {
            if session.claude_session_id.as_ref() != Some(&claude_session_id) {
                debug!("Session {} claude_session_id: {:?}", session_id, claude_session_id);
                session.claude_session_id = Some(claude_session_id);
            }
        }
    }

    pub fn pty_manager(&self) -> Arc<PtyManager> {
        self.pty_manager.clone()
    }

    pub fn output_tx(&self) -> mpsc::Sender<(Uuid, Vec<u8>)> {
        self.output_tx.clone()
    }

    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    pub fn event_tx(&self) -> broadcast::Sender<Event> {
        self.event_tx.clone()
    }

    pub async fn create_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        event_tx: &broadcast::Sender<Event>,
        name: String,
        working_dir: PathBuf,
        group_id: Option<Uuid>,
    ) -> Result<Session> {
        let mut session = Session::new(name, working_dir.clone(), group_id);

        // Spawn PTY
        pty_manager
            .spawn(session.id, &working_dir, 24, 80, output_tx)
            .await?;

        session.status = SessionStatus::Running;
        session.last_activity = Utc::now();

        // Save to state
        {
            let mut s = state.write().await;
            s.sessions.insert(session.id, session.clone());
        }
        save_state(state).await?;

        // Emit event
        let event = Event {
            event: "session.created".to_string(),
            data: serde_json::to_value(&session)?,
        };
        let _ = event_tx.send(event);

        Ok(session)
    }

    pub async fn stop_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        event_tx: &broadcast::Sender<Event>,
        session_id: Uuid,
    ) -> Result<()> {
        pty_manager.kill(session_id).await?;

        {
            let mut s = state.write().await;
            if let Some(session) = s.sessions.get_mut(&session_id) {
                session.status = SessionStatus::Stopped;
                session.pid = None;
            }
        }
        save_state(state).await?;

        let event = Event {
            event: "session.status_changed".to_string(),
            data: serde_json::to_value(StatusChangedData {
                session_id,
                status: SessionStatus::Stopped,
            })?,
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    pub async fn delete_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        event_tx: &broadcast::Sender<Event>,
        session_id: Uuid,
    ) -> Result<()> {
        // Stop first if running
        if pty_manager.is_alive(session_id).await {
            pty_manager.kill(session_id).await?;
        }

        {
            let mut s = state.write().await;
            s.sessions.remove(&session_id);
        }
        save_state(state).await?;

        let event = Event {
            event: "session.deleted".to_string(),
            data: serde_json::json!({"session_id": session_id}),
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    pub async fn create_group(
        state: &SharedState,
        event_tx: &broadcast::Sender<Event>,
        name: String,
        parent_id: Option<Uuid>,
    ) -> Result<Group> {
        let group = Group::new(name, parent_id);

        {
            let mut s = state.write().await;
            s.groups.insert(group.id, group.clone());
        }
        save_state(state).await?;

        let event = Event {
            event: "group.created".to_string(),
            data: serde_json::to_value(&group)?,
        };
        let _ = event_tx.send(event);

        Ok(group)
    }

    pub async fn delete_group(
        state: &SharedState,
        event_tx: &broadcast::Sender<Event>,
        group_id: Uuid,
    ) -> Result<()> {
        {
            let mut s = state.write().await;
            // Move sessions in this group to root
            for session in s.sessions.values_mut() {
                if session.group_id == Some(group_id) {
                    session.group_id = None;
                }
            }
            // Move child groups to parent of deleted group
            let parent_id = s.groups.get(&group_id).and_then(|g| g.parent_id);
            for group in s.groups.values_mut() {
                if group.parent_id == Some(group_id) {
                    group.parent_id = parent_id;
                }
            }
            s.groups.remove(&group_id);
        }
        save_state(state).await?;

        let event = Event {
            event: "group.deleted".to_string(),
            data: serde_json::json!({"group_id": group_id}),
        };
        let _ = event_tx.send(event);

        Ok(())
    }
}
