use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use shared::{Event, Group, PtyOutputData, Session, SessionStatus, StatusChangedData};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info};
use uuid::Uuid;

use crate::claude;
use crate::hook_listener::HookEvent;
use crate::hook_manager::HookManager;
use crate::pty::PtyManager;
use crate::state::{save_state, SharedState};
use crate::status_tracker::StatusTracker;

pub struct SessionManager {
    state: SharedState,
    pty_manager: Arc<PtyManager>,
    event_tx: broadcast::Sender<Event>,
    output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
    /// Hook manager for environment variables and hook script
    #[allow(dead_code)]
    hook_manager: Arc<HookManager>,
    /// Status trackers per session (using velocity-based detection)
    status_trackers: Arc<RwLock<HashMap<Uuid, StatusTracker>>>,
}

impl SessionManager {
    pub fn new(
        state: SharedState,
        event_tx: broadcast::Sender<Event>,
        hook_manager: Arc<HookManager>,
    ) -> (Self, mpsc::Receiver<(Uuid, Vec<u8>)>) {
        let (output_tx, output_rx) = mpsc::channel(1000);
        let manager = Self {
            state,
            pty_manager: Arc::new(PtyManager::new()),
            event_tx,
            output_tx,
            hook_manager,
            status_trackers: Arc::new(RwLock::new(HashMap::new())),
        };
        (manager, output_rx)
    }

    /// Get the hook manager for external use (e.g., starting hook listener)
    #[allow(dead_code)]
    pub fn hook_manager(&self) -> Arc<HookManager> {
        self.hook_manager.clone()
    }

    pub async fn run(
        self,
        mut output_rx: mpsc::Receiver<(Uuid, Vec<u8>)>,
        mut hook_rx: mpsc::Receiver<HookEvent>,
    ) {
        info!("Session manager started");

        // Spawn background task to check for waiting→idle transitions
        let idle_state = self.state.clone();
        let idle_event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            Self::idle_checker(idle_state, idle_event_tx).await;
        });

        loop {
            tokio::select! {
                // Handle PTY output
                Some((session_id, data)) = output_rx.recv() => {
                    // Convert to string for status detection (lossy is fine for pattern matching)
                    let text = String::from_utf8_lossy(&data);

                    // Debug: log a sample of the text for status detection debugging
                    let sample: String = text.chars().take(100).collect();
                    let printable_sample: String = sample
                        .chars()
                        .map(|c| if c.is_control() && c != '\n' { '.' } else { c })
                        .collect();
                    debug!(
                        "PTY output: {} bytes, sample: {:?}",
                        data.len(),
                        printable_sample
                    );

                    // Detect status changes with debouncing
                    if let Some(detected_status) = claude::detect_status(&text) {
                        self.handle_status_detection(session_id, detected_status)
                            .await;
                    }

                    // Extract Claude session ID if present
                    if let Some(claude_session_id) = claude::extract_session_id(&text) {
                        self.update_claude_session_id(session_id, claude_session_id)
                            .await;
                    }

                    // Forward output as event
                    let output = BASE64.encode(&data);
                    let event = Event {
                        event: "pty:output".to_string(),
                        data: serde_json::to_value(PtyOutputData { session_id, output }).unwrap(),
                    };
                    let _ = self.event_tx.send(event);
                }

                // Handle hook events (authoritative status from Claude hooks)
                Some(hook_event) = hook_rx.recv() => {
                    self.handle_hook_event(hook_event).await;
                }

                // Both channels closed - exit
                else => {
                    info!("Session manager channels closed, shutting down");
                    break;
                }
            }
        }
    }

    async fn update_session_status(&self, session_id: Uuid, new_status: SessionStatus) {
        // First check with read lock to avoid write lock contention
        let needs_update = {
            let s = self.state.read().await;
            s.sessions
                .get(&session_id)
                .map(|session| session.status != new_status)
                .unwrap_or(false)
        };

        if !needs_update {
            return;
        }

        // Only acquire write lock if we actually need to update
        let mut status_changed = false;
        {
            let mut s = self.state.write().await;
            if let Some(session) = s.sessions.get_mut(&session_id) {
                if session.status != new_status {
                    debug!(
                        "Session {} status: {:?} -> {:?}",
                        session_id, session.status, new_status
                    );
                    session.status = new_status;
                    session.last_activity = Utc::now();
                    status_changed = true;
                }
            }
        }

        if status_changed {
            // Emit status change event
            let event = Event {
                event: "session:status_changed".to_string(),
                data: serde_json::to_value(StatusChangedData {
                    session_id,
                    status: new_status,
                })
                .unwrap(),
            };
            let _ = self.event_tx.send(event);
        }
    }

    /// Handle status detection with debouncing to prevent flapping
    ///
    /// Uses StatusTracker for sophisticated velocity-based detection and debouncing:
    /// - Transition TO Running is IMMEDIATE (user should see activity right away)
    /// - Transition FROM Running has a 2 second cooldown (prevent flapping during TUI updates)
    /// - This handles interleaved chunks where some have "esc to interrupt" and some don't
    async fn handle_status_detection(&self, session_id: Uuid, detected_status: SessionStatus) {
        // Get current session status
        let current_status = {
            let s = self.state.read().await;
            s.sessions.get(&session_id).map(|s| s.status)
        };

        let Some(current_status) = current_status else {
            return;
        };

        // Use StatusTracker for debounced transitions
        let mut trackers = self.status_trackers.write().await;
        let tracker = trackers
            .entry(session_id)
            .or_insert_with(|| StatusTracker::new(current_status));

        if let Some(new_status) = tracker.handle_detected_status(current_status, detected_status) {
            drop(trackers); // Release lock before async call
            self.update_session_status(session_id, new_status).await;
        }
    }

    /// Handle hook events from Claude Code lifecycle hooks
    /// These provide authoritative status information
    async fn handle_hook_event(&self, event: HookEvent) {
        // Parse session_id from the hook event
        let session_id = match Uuid::parse_str(&event.session_id) {
            Ok(id) => id,
            Err(_) => {
                debug!("Invalid session ID in hook event: {}", event.session_id);
                return;
            }
        };

        // Map hook event to status
        let new_status = match event.state.as_str() {
            "waiting" => SessionStatus::Waiting,
            "running" => SessionStatus::Running,
            "idle" => SessionStatus::Idle,
            _ => {
                debug!("Unknown hook state: {}", event.state);
                return;
            }
        };

        debug!(
            "Hook event: session={} state={} event={}",
            session_id, event.state, event.event
        );

        // Hook events are authoritative - bypass debouncing
        self.update_session_status(session_id, new_status).await;
    }

    async fn update_claude_session_id(&self, session_id: Uuid, claude_session_id: String) {
        // First check with read lock to avoid write lock contention
        let needs_update = {
            let s = self.state.read().await;
            s.sessions
                .get(&session_id)
                .map(|session| session.claude_session_id.as_ref() != Some(&claude_session_id))
                .unwrap_or(false)
        };

        if !needs_update {
            return;
        }

        // Only acquire write lock if we actually need to update
        let mut s = self.state.write().await;
        if let Some(session) = s.sessions.get_mut(&session_id) {
            if session.claude_session_id.as_ref() != Some(&claude_session_id) {
                debug!(
                    "Session {} claude_session_id: {:?}",
                    session_id, claude_session_id
                );
                session.claude_session_id = Some(claude_session_id);
            }
        }
    }

    /// Background task that checks for waiting→idle transitions
    /// Sessions in "Waiting" status for more than IDLE_TIMEOUT become "Idle"
    async fn idle_checker(state: SharedState, event_tx: broadcast::Sender<Event>) {
        const IDLE_TIMEOUT_SECS: i64 = 60; // 1 minute of inactivity
        const CHECK_INTERVAL_SECS: u64 = 10; // Check every 10 seconds

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(CHECK_INTERVAL_SECS)).await;

            let now = Utc::now();
            let mut sessions_to_idle = Vec::new();

            // Check with read lock first
            {
                let s = state.read().await;
                for (id, session) in s.sessions.iter() {
                    if session.status == SessionStatus::Waiting {
                        let elapsed = now.signed_duration_since(session.last_activity);
                        if elapsed.num_seconds() > IDLE_TIMEOUT_SECS {
                            sessions_to_idle.push(*id);
                        }
                    }
                }
            }

            // Update sessions that need to transition to Idle
            for session_id in sessions_to_idle {
                let mut s = state.write().await;
                if let Some(session) = s.sessions.get_mut(&session_id) {
                    // Double-check it's still waiting (might have changed)
                    if session.status == SessionStatus::Waiting {
                        debug!(
                            "Session {} transitioning to Idle (inactive for >{}s)",
                            session_id, IDLE_TIMEOUT_SECS
                        );
                        session.status = SessionStatus::Idle;

                        // Emit status change event
                        let event = Event {
                            event: "session:status_changed".to_string(),
                            data: serde_json::to_value(StatusChangedData {
                                session_id,
                                status: SessionStatus::Idle,
                            })
                            .unwrap(),
                        };
                        let _ = event_tx.send(event);
                    }
                }
            }
        }
    }

    pub fn pty_manager(&self) -> Arc<PtyManager> {
        self.pty_manager.clone()
    }

    pub fn output_tx(&self) -> mpsc::Sender<(Uuid, Vec<u8>)> {
        self.output_tx.clone()
    }

    #[allow(dead_code)]
    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    #[allow(dead_code)]
    pub fn event_tx(&self) -> broadcast::Sender<Event> {
        self.event_tx.clone()
    }

    pub async fn create_session(
        state: &SharedState,
        _pty_manager: &PtyManager,
        _output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        event_tx: &broadcast::Sender<Event>,
        name: String,
        working_dir: PathBuf,
        group_id: Option<Uuid>,
    ) -> Result<Session> {
        let session = Session::new(name, working_dir.clone(), group_id);
        // Note: Session is created in "stopped" state by default
        // The PTY is NOT spawned here - it will be spawned when the terminal
        // is ready and calls restart_session with proper dimensions

        // Save to state
        {
            let mut s = state.write().await;
            s.sessions.insert(session.id, session.clone());
        }
        save_state(state).await?;

        // Emit event
        let event = Event {
            event: "session:created".to_string(),
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
            event: "session:status_changed".to_string(),
            data: serde_json::to_value(StatusChangedData {
                session_id,
                status: SessionStatus::Stopped,
            })?,
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn fork_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        event_tx: &broadcast::Sender<Event>,
        hook_manager: &HookManager,
        source_session_id: Uuid,
        new_name: Option<String>,
        new_group_id: Option<Uuid>,
        rows: u16,
        cols: u16,
    ) -> Result<Session> {
        // Get source session info
        let (working_dir, claude_session_id, group_id, source_name) = {
            let s = state.read().await;
            let source = s
                .sessions
                .get(&source_session_id)
                .ok_or_else(|| anyhow::anyhow!("Source session not found"))?;

            let claude_id = source.claude_session_id.clone().ok_or_else(|| {
                anyhow::anyhow!("Source session has no Claude session ID - cannot fork")
            })?;

            (
                source.working_dir.clone(),
                claude_id,
                source.group_id,
                source.name.clone(),
            )
        };

        // Create new session with forked name
        let name = new_name.unwrap_or_else(|| format!("{} (Fork)", source_name));

        let mut session = Session::new(name, working_dir.clone(), new_group_id.or(group_id));

        // Get hook environment variables for this session
        let hook_env = hook_manager.get_env_vars(&session.id.to_string());

        // Spawn PTY with --resume flag using provided dimensions
        info!("Spawning forked PTY with size {}x{}", cols, rows);
        pty_manager
            .spawn_with_resume(
                session.id,
                &working_dir,
                rows,
                cols,
                output_tx,
                Some(&claude_session_id),
                hook_env,
            )
            .await?;

        session.status = SessionStatus::Running;
        session.claude_session_id = Some(claude_session_id);
        session.last_activity = Utc::now();

        // Save to state
        {
            let mut s = state.write().await;
            s.sessions.insert(session.id, session.clone());
        }
        save_state(state).await?;

        // Emit event
        let event = Event {
            event: "session:created".to_string(),
            data: serde_json::to_value(&session)?,
        };
        let _ = event_tx.send(event);

        info!(
            "Forked session {} from {} with Claude session {}",
            session.id,
            source_session_id,
            session.claude_session_id.as_ref().unwrap()
        );

        Ok(session)
    }

    pub async fn restart_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        event_tx: &broadcast::Sender<Event>,
        hook_manager: &HookManager,
        session_id: Uuid,
        rows: u16,
        cols: u16,
    ) -> Result<Session> {
        // Get session info
        let working_dir = {
            let s = state.read().await;
            let session = s
                .sessions
                .get(&session_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
            session.working_dir.clone()
        };

        // Stop if running
        if pty_manager.is_alive(session_id).await {
            pty_manager.kill(session_id).await?;
        }

        // Get hook environment variables for this session
        let hook_env = hook_manager.get_env_vars(&session_id.to_string());

        // Spawn new PTY with specified dimensions
        // This is critical - Claude Code checks terminal size at startup
        // to decide whether to use full TUI mode with alternate screen buffer
        info!("Spawning PTY with size {}x{}", cols, rows);
        pty_manager
            .spawn(session_id, &working_dir, rows, cols, output_tx, hook_env)
            .await?;

        // Update session state
        let session = {
            let mut s = state.write().await;
            let session = s
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
            session.status = SessionStatus::Running;
            session.last_activity = Utc::now();
            session.clone()
        };
        save_state(state).await?;

        // Emit status changed event
        let event = Event {
            event: "session:status_changed".to_string(),
            data: serde_json::to_value(StatusChangedData {
                session_id,
                status: SessionStatus::Running,
            })?,
        };
        let _ = event_tx.send(event);

        info!("Restarted session {}", session_id);

        Ok(session)
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
            event: "session:deleted".to_string(),
            data: serde_json::json!({"session_id": session_id}),
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    pub async fn update_session(
        state: &SharedState,
        event_tx: &broadcast::Sender<Event>,
        session_id: Uuid,
        name: Option<String>,
        group_id: Option<Option<Uuid>>, // None = don't change, Some(None) = remove from group, Some(Some(id)) = set group
    ) -> Result<Session> {
        let session = {
            let mut s = state.write().await;
            let session = s
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

            if let Some(new_name) = name {
                session.name = new_name;
            }
            if let Some(new_group_id) = group_id {
                session.group_id = new_group_id;
            }

            session.clone()
        };
        save_state(state).await?;

        let event = Event {
            event: "session:updated".to_string(),
            data: serde_json::to_value(&session)?,
        };
        let _ = event_tx.send(event);

        Ok(session)
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
            event: "group:created".to_string(),
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
            event: "group:deleted".to_string(),
            data: serde_json::json!({"group_id": group_id}),
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    pub async fn update_group(
        state: &SharedState,
        event_tx: &broadcast::Sender<Event>,
        group_id: Uuid,
        name: Option<String>,
        parent_id: Option<Option<Uuid>>,
    ) -> Result<Group> {
        let group = {
            let mut s = state.write().await;
            let group = s
                .groups
                .get_mut(&group_id)
                .ok_or_else(|| anyhow::anyhow!("Group not found"))?;

            if let Some(new_name) = name {
                group.name = new_name;
            }
            if let Some(new_parent_id) = parent_id {
                // Prevent circular references
                if new_parent_id == Some(group_id) {
                    return Err(anyhow::anyhow!("Group cannot be its own parent"));
                }
                group.parent_id = new_parent_id;
            }

            group.clone()
        };
        save_state(state).await?;

        let event = Event {
            event: "group:updated".to_string(),
            data: serde_json::to_value(&group)?,
        };
        let _ = event_tx.send(event);

        Ok(group)
    }
}
