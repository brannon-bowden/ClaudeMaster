//! Tauri commands that bridge the frontend to the daemon

use serde_json::json;
use shared::{Group, Session};
use tauri::State;
use uuid::Uuid;

use crate::DaemonState;

/// Connect to the daemon
#[tauri::command]
pub async fn connect_daemon(state: State<'_, DaemonState>) -> Result<(), String> {
    state.client.connect().await
}

/// Check if connected to daemon
#[tauri::command]
pub async fn is_daemon_connected(state: State<'_, DaemonState>) -> Result<bool, String> {
    Ok(state.client.is_connected().await)
}

/// Ping the daemon
#[tauri::command]
pub async fn ping_daemon(state: State<'_, DaemonState>) -> Result<String, String> {
    let result = state.client.call("daemon.ping", json!({})).await?;
    Ok(result.to_string())
}

/// List all sessions
#[tauri::command]
pub async fn list_sessions(state: State<'_, DaemonState>) -> Result<Vec<Session>, String> {
    let result = state.client.call("session.list", json!({})).await?;
    let sessions = result
        .get("sessions")
        .ok_or("Missing sessions field")?
        .clone();
    serde_json::from_value(sessions).map_err(|e| e.to_string())
}

/// Create a new session
#[tauri::command]
pub async fn create_session(
    state: State<'_, DaemonState>,
    name: String,
    dir: String,
    group_id: Option<String>,
) -> Result<Session, String> {
    let group_uuid = group_id
        .map(|id| Uuid::parse_str(&id))
        .transpose()
        .map_err(|e| format!("Invalid group_id: {}", e))?;

    let result = state
        .client
        .call(
            "session.create",
            json!({
                "name": name,
                "dir": dir,
                "group_id": group_uuid,
            }),
        )
        .await?;

    let session = result
        .get("session")
        .ok_or("Missing session field")?
        .clone();
    serde_json::from_value(session).map_err(|e| e.to_string())
}

/// Stop a session
#[tauri::command]
pub async fn stop_session(
    state: State<'_, DaemonState>,
    session_id: String,
) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    let result = state
        .client
        .call("session.stop", json!({ "session_id": uuid }))
        .await?;

    result
        .get("success")
        .and_then(|v| v.as_bool())
        .ok_or("Missing success field".to_string())
}

/// Delete a session
#[tauri::command]
pub async fn delete_session(
    state: State<'_, DaemonState>,
    session_id: String,
) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    let result = state
        .client
        .call("session.delete", json!({ "session_id": uuid }))
        .await?;

    result
        .get("success")
        .and_then(|v| v.as_bool())
        .ok_or("Missing success field".to_string())
}

/// Fork a session
#[tauri::command]
pub async fn fork_session(
    state: State<'_, DaemonState>,
    session_id: String,
    new_name: Option<String>,
    group_id: Option<String>,
) -> Result<Session, String> {
    let session_uuid =
        Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    let group_uuid = group_id
        .map(|id| Uuid::parse_str(&id))
        .transpose()
        .map_err(|e| format!("Invalid group_id: {}", e))?;

    let result = state
        .client
        .call(
            "session.fork",
            json!({
                "session_id": session_uuid,
                "new_name": new_name,
                "group_id": group_uuid,
            }),
        )
        .await?;

    let session = result
        .get("session")
        .ok_or("Missing session field")?
        .clone();
    serde_json::from_value(session).map_err(|e| e.to_string())
}

/// Send input to a session
#[tauri::command]
pub async fn send_input(
    state: State<'_, DaemonState>,
    session_id: String,
    input: String,
) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    let result = state
        .client
        .call(
            "session.input",
            json!({
                "session_id": uuid,
                "input": input,
            }),
        )
        .await?;

    result
        .get("success")
        .and_then(|v| v.as_bool())
        .ok_or("Missing success field".to_string())
}

/// Resize a session's PTY
#[tauri::command]
pub async fn resize_session(
    state: State<'_, DaemonState>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    let result = state
        .client
        .call(
            "session.resize",
            json!({
                "session_id": uuid,
                "rows": rows,
                "cols": cols,
            }),
        )
        .await?;

    result
        .get("success")
        .and_then(|v| v.as_bool())
        .ok_or("Missing success field".to_string())
}

/// List all groups
#[tauri::command]
pub async fn list_groups(state: State<'_, DaemonState>) -> Result<Vec<Group>, String> {
    let result = state.client.call("group.list", json!({})).await?;
    let groups = result.get("groups").ok_or("Missing groups field")?.clone();
    serde_json::from_value(groups).map_err(|e| e.to_string())
}

/// Create a new group
#[tauri::command]
pub async fn create_group(
    state: State<'_, DaemonState>,
    name: String,
    parent_id: Option<String>,
) -> Result<Group, String> {
    let parent_uuid = parent_id
        .map(|id| Uuid::parse_str(&id))
        .transpose()
        .map_err(|e| format!("Invalid parent_id: {}", e))?;

    let result = state
        .client
        .call(
            "group.create",
            json!({
                "name": name,
                "parent_id": parent_uuid,
            }),
        )
        .await?;

    let group = result.get("group").ok_or("Missing group field")?.clone();
    serde_json::from_value(group).map_err(|e| e.to_string())
}

/// Delete a group
#[tauri::command]
pub async fn delete_group(state: State<'_, DaemonState>, group_id: String) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&group_id).map_err(|e| format!("Invalid group_id: {}", e))?;

    // Note: the daemon uses session_id field for group_id in delete
    let result = state
        .client
        .call("group.delete", json!({ "session_id": uuid }))
        .await?;

    result
        .get("success")
        .and_then(|v| v.as_bool())
        .ok_or("Missing success field".to_string())
}

/// Update a session (name and/or group)
/// For group_id: None = don't change, Some("") = remove from group, Some("uuid") = set group
#[tauri::command]
pub async fn update_session(
    state: State<'_, DaemonState>,
    session_id: String,
    name: Option<String>,
    group_id: Option<String>,
) -> Result<Session, String> {
    let session_uuid =
        Uuid::parse_str(&session_id).map_err(|e| format!("Invalid session_id: {}", e))?;

    // Convert: None = don't change, Some("") = remove group, Some("uuid") = set group
    let group_uuid: Option<Option<Uuid>> = match group_id {
        None => None, // Don't change
        Some(ref id) if id.is_empty() => Some(None), // Remove from group
        Some(id) => Some(Some(
            Uuid::parse_str(&id).map_err(|e| format!("Invalid group_id: {}", e))?,
        )),
    };

    let result = state
        .client
        .call(
            "session.update",
            json!({
                "session_id": session_uuid,
                "name": name,
                "group_id": group_uuid,
            }),
        )
        .await?;

    serde_json::from_value(result).map_err(|e| e.to_string())
}

/// Update a group (name and/or parent)
/// For parent_id: None = don't change, Some("") = make root, Some("uuid") = set parent
#[tauri::command]
pub async fn update_group(
    state: State<'_, DaemonState>,
    group_id: String,
    name: Option<String>,
    parent_id: Option<String>,
) -> Result<Group, String> {
    let group_uuid = Uuid::parse_str(&group_id).map_err(|e| format!("Invalid group_id: {}", e))?;

    // Convert: None = don't change, Some("") = make root, Some("uuid") = set parent
    let parent_uuid: Option<Option<Uuid>> = match parent_id {
        None => None, // Don't change
        Some(ref id) if id.is_empty() => Some(None), // Make root (no parent)
        Some(id) => Some(Some(
            Uuid::parse_str(&id).map_err(|e| format!("Invalid parent_id: {}", e))?,
        )),
    };

    let result = state
        .client
        .call(
            "group.update",
            json!({
                "group_id": group_uuid,
                "name": name,
                "parent_id": parent_uuid,
            }),
        )
        .await?;

    serde_json::from_value(result).map_err(|e| e.to_string())
}
