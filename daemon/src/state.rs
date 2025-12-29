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
        for session in sessions {
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
