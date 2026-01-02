// Hook manager - installs and configures Claude Code hooks
// Hooks provide authoritative status information via lifecycle events

use anyhow::Result;
use std::path::PathBuf;
use tracing::{debug, info};

/// The hook script content - embedded in the binary
const HOOK_SCRIPT: &str = r#"#!/bin/bash
# Agent Deck Claude Code Hook
# Reports session status changes via Unix socket

SESSION_ID="${AGENT_DECK_SESSION_ID}"
SOCKET_PATH="${AGENT_DECK_SOCKET}"

# Silently exit if not in an Agent Deck session
if [ -z "$SESSION_ID" ] || [ -z "$SOCKET_PATH" ]; then
    exit 0
fi

# Report state to daemon
report_state() {
    local state="$1"
    local event="$2"
    if [ -S "$SOCKET_PATH" ]; then
        echo "{\"session_id\":\"$SESSION_ID\",\"state\":\"$state\",\"event\":\"$event\",\"ts\":$(date +%s)}" \
            | nc -U "$SOCKET_PATH" 2>/dev/null || true
    fi
}

# Handle hook events
case "$1" in
    "PreToolUse")
        # About to run a tool - needs approval
        report_state "waiting" "tool_approval"
        ;;
    "PostToolUse")
        # Tool completed - back to working
        report_state "running" "tool_complete"
        ;;
    "Stop")
        # Claude Code stopped
        report_state "idle" "stopped"
        ;;
    "Notification")
        # Just a notification, no state change needed
        ;;
esac

# Always exit successfully to not block Claude
exit 0
"#;

/// Manages Claude Code hook installation and configuration
pub struct HookManager {
    hooks_dir: PathBuf,
    socket_path: PathBuf,
}

impl HookManager {
    /// Create a new hook manager with the specified paths
    pub fn new(hooks_dir: PathBuf, socket_path: PathBuf) -> Self {
        Self {
            hooks_dir,
            socket_path,
        }
    }

    /// Initialize the hook manager using default paths
    pub fn init() -> Result<Self> {
        let hooks_dir = shared::get_hooks_dir()?;
        let socket_path = shared::get_hook_socket_path()?;
        Ok(Self::new(hooks_dir, socket_path))
    }

    /// Ensure the hook script is installed and up-to-date
    pub fn ensure_hook_script(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.hooks_dir)?;

        let script_path = self.hooks_dir.join("agent-deck-hook.sh");

        // Write or update the script
        std::fs::write(&script_path, HOOK_SCRIPT)?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
        }

        info!("Hook script installed at {:?}", script_path);
        Ok(script_path)
    }

    /// Get environment variables needed for Claude to use our hooks
    /// These should be passed to the PTY when spawning Claude
    pub fn get_env_vars(&self, session_id: &str) -> Vec<(String, String)> {
        vec![
            // Tell Claude where to find hooks
            (
                "CLAUDE_HOOKS_DIR".to_string(),
                self.hooks_dir.to_string_lossy().to_string(),
            ),
            // Our custom vars for the hook script
            ("AGENT_DECK_SESSION_ID".to_string(), session_id.to_string()),
            (
                "AGENT_DECK_SOCKET".to_string(),
                self.socket_path.to_string_lossy().to_string(),
            ),
        ]
    }

    /// Get the hooks directory path
    pub fn hooks_dir(&self) -> &PathBuf {
        &self.hooks_dir
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::init().expect("Failed to initialize HookManager with default paths")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_vars() {
        let hooks_dir = PathBuf::from("/tmp/test-hooks");
        let socket_path = PathBuf::from("/tmp/test.sock");
        let manager = HookManager::new(hooks_dir.clone(), socket_path.clone());

        let vars = manager.get_env_vars("test-session-id");

        assert_eq!(vars.len(), 3);
        assert!(vars.iter().any(|(k, v)| k == "CLAUDE_HOOKS_DIR"
            && v == hooks_dir.to_string_lossy().as_ref()));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "AGENT_DECK_SESSION_ID" && v == "test-session-id"));
        assert!(vars.iter().any(|(k, v)| k == "AGENT_DECK_SOCKET"
            && v == socket_path.to_string_lossy().as_ref()));
    }
}
