use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::group::Group;
use crate::session::{Session, SessionStatus};

/// Request from GUI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Response from daemon to GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: i32,
    pub message: String,
}

/// Event from daemon to GUI (no id, push-based)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event: String,
    pub data: Value,
}

// --- Method Parameters ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionParams {
    pub name: String,
    pub dir: String,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIdParams {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInputParams {
    pub session_id: Uuid,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResizeParams {
    pub session_id: Uuid,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkSessionParams {
    pub session_id: Uuid,
    pub new_name: Option<String>,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupParams {
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveToGroupParams {
    pub session_id: Uuid,
    pub group_id: Option<Uuid>,
}

// --- Event Data ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChangedData {
    pub session_id: Uuid,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyOutputData {
    pub session_id: Uuid,
    pub output: String, // base64 encoded
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyExitData {
    pub session_id: Uuid,
    pub exit_code: Option<i32>,
}

// --- Results ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResult {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupListResult {
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedResult {
    pub session: Session,
}
