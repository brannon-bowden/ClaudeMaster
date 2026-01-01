use anyhow::Result;
use shared::{Group, Session};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::get_state_dir;

#[derive(Debug, Default)]
pub struct AppState {
    pub sessions: HashMap<Uuid, Session>,
    pub groups: HashMap<Uuid, Group>,
}

pub type SharedState = Arc<RwLock<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

fn sessions_path() -> Result<PathBuf> {
    Ok(get_state_dir()?.join("sessions.json"))
}

fn groups_path() -> Result<PathBuf> {
    Ok(get_state_dir()?.join("groups.json"))
}

pub async fn load_state(state: &SharedState) -> Result<()> {
    let mut s = state.write().await;

    // Load sessions
    let sessions_file = sessions_path()?;
    if sessions_file.exists() {
        let content = fs::read_to_string(&sessions_file)?;
        let sessions: Vec<Session> = serde_json::from_str(&content)?;
        for mut session in sessions {
            // Reset session status to Stopped on daemon restart
            // PTY processes don't survive daemon restarts, so any active session
            // from a previous run needs to be marked as stopped
            match session.status {
                shared::SessionStatus::Running
                | shared::SessionStatus::Waiting
                | shared::SessionStatus::Idle
                | shared::SessionStatus::Error => {
                    // Reset all active/error states to Stopped on daemon restart
                    // PTY processes don't survive daemon restarts
                    session.status = shared::SessionStatus::Stopped;
                    session.pid = None;
                }
                shared::SessionStatus::Stopped => {
                    // Already stopped, no change needed
                }
            }
            s.sessions.insert(session.id, session);
        }
    }

    // Load groups
    let groups_file = groups_path()?;
    if groups_file.exists() {
        let content = fs::read_to_string(&groups_file)?;
        let groups: Vec<Group> = serde_json::from_str(&content)?;
        for group in groups {
            s.groups.insert(group.id, group);
        }
    }

    Ok(())
}

pub async fn save_state(state: &SharedState) -> Result<()> {
    let s = state.read().await;

    // Save sessions
    let sessions: Vec<&Session> = s.sessions.values().collect();
    let sessions_json = serde_json::to_string_pretty(&sessions)?;
    let sessions_file = sessions_path()?;

    // Backup before writing
    if sessions_file.exists() {
        let backup = sessions_file.with_extension("json.bak");
        fs::copy(&sessions_file, backup)?;
    }
    fs::write(&sessions_file, sessions_json)?;

    // Save groups
    let groups: Vec<&Group> = s.groups.values().collect();
    let groups_json = serde_json::to_string_pretty(&groups)?;
    let groups_file = groups_path()?;
    fs::write(&groups_file, groups_json)?;

    Ok(())
}

/// Reorder a session: move to a new group and/or position
/// - `group_id`: Target group (None = root level)
/// - `after_session_id`: Insert after this session (None = insert at beginning)
pub async fn reorder_session(
    state: &SharedState,
    session_id: Uuid,
    group_id: Option<Uuid>,
    after_session_id: Option<Uuid>,
) -> Result<Session> {
    let mut s = state.write().await;

    // Verify session exists
    if !s.sessions.contains_key(&session_id) {
        anyhow::bail!("Session not found: {}", session_id);
    }

    // Verify target group exists if specified
    if let Some(gid) = group_id {
        if !s.groups.contains_key(&gid) {
            anyhow::bail!("Group not found: {}", gid);
        }
    }

    // Update session's group_id
    if let Some(session) = s.sessions.get_mut(&session_id) {
        session.group_id = group_id;
    }

    // Get all sessions in the target group, sorted by current order
    let mut siblings: Vec<Uuid> = s
        .sessions
        .values()
        .filter(|sess| sess.group_id == group_id && sess.id != session_id)
        .map(|sess| (sess.id, sess.order))
        .collect::<Vec<_>>()
        .into_iter()
        .sorted_by_key(|(_, order)| *order)
        .map(|(id, _)| id)
        .collect();

    // Find insertion position
    let insert_pos = if let Some(after_id) = after_session_id {
        siblings
            .iter()
            .position(|id| *id == after_id)
            .map(|p| p + 1)
            .unwrap_or(0)
    } else {
        0
    };

    // Insert the moved session at the correct position
    siblings.insert(insert_pos, session_id);

    // Renumber all siblings
    for (idx, id) in siblings.iter().enumerate() {
        if let Some(sess) = s.sessions.get_mut(id) {
            sess.order = idx as u32;
        }
    }

    let session = s.sessions.get(&session_id).cloned().unwrap();
    Ok(session)
}

/// Reorder a group: move to a new parent and/or position
/// - `parent_id`: Target parent group (None = root level)
/// - `after_group_id`: Insert after this group (None = insert at beginning)
pub async fn reorder_group(
    state: &SharedState,
    group_id: Uuid,
    parent_id: Option<Uuid>,
    after_group_id: Option<Uuid>,
) -> Result<Group> {
    let mut s = state.write().await;

    // Verify group exists
    if !s.groups.contains_key(&group_id) {
        anyhow::bail!("Group not found: {}", group_id);
    }

    // Verify target parent exists if specified
    if let Some(pid) = parent_id {
        if !s.groups.contains_key(&pid) {
            anyhow::bail!("Parent group not found: {}", pid);
        }
        // Check for cycle: can't make a group a child of its own descendant
        if would_create_cycle(&s.groups, group_id, pid) {
            anyhow::bail!("Cannot move group into its own descendant");
        }
    }

    // Update group's parent_id
    if let Some(group) = s.groups.get_mut(&group_id) {
        group.parent_id = parent_id;
    }

    // Get all groups with the same parent, sorted by current order
    let mut siblings: Vec<Uuid> = s
        .groups
        .values()
        .filter(|g| g.parent_id == parent_id && g.id != group_id)
        .map(|g| (g.id, g.order))
        .collect::<Vec<_>>()
        .into_iter()
        .sorted_by_key(|(_, order)| *order)
        .map(|(id, _)| id)
        .collect();

    // Find insertion position
    let insert_pos = if let Some(after_id) = after_group_id {
        siblings
            .iter()
            .position(|id| *id == after_id)
            .map(|p| p + 1)
            .unwrap_or(0)
    } else {
        0
    };

    // Insert the moved group at the correct position
    siblings.insert(insert_pos, group_id);

    // Renumber all siblings
    for (idx, id) in siblings.iter().enumerate() {
        if let Some(g) = s.groups.get_mut(id) {
            g.order = idx as u32;
        }
    }

    let group = s.groups.get(&group_id).cloned().unwrap();
    Ok(group)
}

/// Check if making `group_id` a child of `potential_parent` would create a cycle
fn would_create_cycle(
    groups: &HashMap<Uuid, Group>,
    group_id: Uuid,
    potential_parent: Uuid,
) -> bool {
    let mut current = Some(potential_parent);
    while let Some(pid) = current {
        if pid == group_id {
            return true;
        }
        current = groups.get(&pid).and_then(|g| g.parent_id);
    }
    false
}

// Helper trait for sorting
use itertools::Itertools;
