use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Waiting,
    Idle,
    Error,
    #[default]
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub group_id: Option<Uuid>,
    pub working_dir: PathBuf,

    #[serde(default)]
    pub status: SessionStatus,
    #[serde(skip)]
    pub pid: Option<u32>,

    pub claude_session_id: Option<String>,

    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    #[serde(default)]
    pub order: u32,
}

impl Session {
    pub fn new(name: String, working_dir: PathBuf, group_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            group_id,
            working_dir,
            status: SessionStatus::Stopped,
            pid: None,
            claude_session_id: None,
            created_at: now,
            last_activity: now,
            order: 0,
        }
    }
}
