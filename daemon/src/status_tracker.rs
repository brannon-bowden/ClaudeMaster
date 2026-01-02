// Status tracker with velocity-based debouncing
// Provides stable status detection by tracking output velocity and debouncing transitions

use lazy_static::lazy_static;
use regex::Regex;
use shared::SessionStatus;
use std::time::{Duration, Instant};
use tracing::debug;

/// Configuration for status tracking behavior
const VELOCITY_WINDOW_SECS: u64 = 1; // Reset velocity if no output for this long
const WORKING_BYTE_THRESHOLD: usize = 200; // Minimum bytes to trigger working debounce
const WORKING_DEBOUNCE_MS: u64 = 300; // How long to wait before confirming working state
const RUNNING_COOLDOWN_MS: u64 = 2000; // Cooldown before leaving running state

/// Tracks session status with debouncing to prevent flapping
pub struct StatusTracker {
    /// Current confirmed status
    last_status: SessionStatus,
    /// When the current status was last confirmed
    last_change: Instant,
    /// Recent output bytes for velocity calculation
    recent_output_bytes: usize,
    /// When we last received output
    last_output_time: Instant,
    /// Pending transition to working state (started at)
    working_debounce: Option<Instant>,
    /// Pending transition from running state (to_status, started at)
    pending_transition: Option<(SessionStatus, Instant)>,
}

impl StatusTracker {
    /// Create a new tracker with initial status
    pub fn new(initial_status: SessionStatus) -> Self {
        Self {
            last_status: initial_status,
            last_change: Instant::now(),
            recent_output_bytes: 0,
            last_output_time: Instant::now(),
            working_debounce: None,
            pending_transition: None,
        }
    }

    /// Process PTY output and potentially detect a status change
    /// Returns Some(new_status) if status should transition
    pub fn process_output(&mut self, text: &str) -> Option<SessionStatus> {
        // Strip ANSI codes for cleaner pattern matching
        let clean_text = strip_ansi(text);

        // Count printable characters (ignore control sequences)
        let printable_len = clean_text.chars().filter(|c| !c.is_control()).count();

        // Ignore very small outputs (likely just cursor movements)
        if printable_len < 3 {
            return None;
        }

        let now = Instant::now();

        // Reset velocity if there was a gap > 1 second
        if now.duration_since(self.last_output_time) > Duration::from_secs(VELOCITY_WINDOW_SECS) {
            self.recent_output_bytes = 0;
        }
        self.recent_output_bytes += printable_len;
        self.last_output_time = now;

        // Error patterns: immediate transition
        if has_error_pattern(&clean_text) {
            return self.transition_to(SessionStatus::Error);
        }

        // Waiting patterns: immediate transition
        if has_waiting_pattern(&clean_text) {
            self.working_debounce = None;
            return self.transition_to(SessionStatus::Waiting);
        }

        // Running patterns: debounced (need 200+ bytes over 300ms)
        if has_running_pattern(&clean_text) && !in_hook_phase(&clean_text) {
            if self.recent_output_bytes > WORKING_BYTE_THRESHOLD {
                match self.working_debounce {
                    None => {
                        // Start debounce timer
                        self.working_debounce = Some(now);
                        None
                    }
                    Some(started)
                        if now.duration_since(started)
                            > Duration::from_millis(WORKING_DEBOUNCE_MS) =>
                    {
                        // Debounce complete, transition to running
                        self.working_debounce = None;
                        self.transition_to(SessionStatus::Running)
                    }
                    _ => None, // Still waiting for debounce
                }
            } else {
                None // Not enough velocity yet
            }
        } else {
            self.working_debounce = None;
            None
        }
    }

    /// Handle status detection with debouncing (called by session_manager)
    /// This implements the cooldown-based approach:
    /// - Transition TO Running is IMMEDIATE (user should see activity right away)
    /// - Transition FROM Running has a cooldown (prevents flapping during TUI updates)
    pub fn handle_detected_status(
        &mut self,
        current_status: SessionStatus,
        detected_status: SessionStatus,
    ) -> Option<SessionStatus> {
        let now = Instant::now();

        // Same status - just update but DON'T reset pending
        // Important: if we're Running and see Running, keep pending Waiting timer
        if detected_status == current_status {
            self.last_status = current_status;
            return None;
        }

        // IMMEDIATE transition TO Running - don't debounce
        // User should see the "running" indicator as soon as Claude starts working
        if detected_status == SessionStatus::Running {
            debug!(
                "Immediate transition to Running: {:?} -> {:?}",
                current_status, detected_status
            );
            self.pending_transition = None;
            return self.transition_to(detected_status);
        }

        // Transition FROM Running requires cooldown (prevents flapping)
        if current_status == SessionStatus::Running {
            match &self.pending_transition {
                Some((pending_status, first_seen)) if *pending_status == detected_status => {
                    // Same pending status - check if cooldown has passed
                    let elapsed = now.duration_since(*first_seen);

                    if elapsed >= Duration::from_millis(RUNNING_COOLDOWN_MS) {
                        debug!(
                            "Running cooldown complete: Running -> {:?} (after {:?})",
                            detected_status, elapsed
                        );
                        self.pending_transition = None;
                        return self.transition_to(detected_status);
                    }
                    // Otherwise, keep waiting for cooldown
                    None
                }
                _ => {
                    // Start cooldown timer for leaving Running
                    debug!(
                        "Starting Running cooldown: Running -> {:?}",
                        detected_status
                    );
                    self.pending_transition = Some((detected_status, now));
                    None
                }
            }
        } else {
            // Other transitions (not involving Running) - immediate
            debug!(
                "Immediate transition: {:?} -> {:?}",
                current_status, detected_status
            );
            self.transition_to(detected_status)
        }
    }

    /// Perform a status transition
    fn transition_to(&mut self, new_status: SessionStatus) -> Option<SessionStatus> {
        if self.last_status != new_status {
            debug!("Status transition: {:?} -> {:?}", self.last_status, new_status);
            self.last_status = new_status;
            self.last_change = Instant::now();
            Some(new_status)
        } else {
            None
        }
    }

    /// Get the current tracked status
    pub fn current_status(&self) -> SessionStatus {
        self.last_status
    }
}

/// Strip ANSI escape sequences from text
fn strip_ansi(text: &str) -> String {
    lazy_static! {
        static ref ANSI_RE: Regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07").unwrap();
    }
    ANSI_RE.replace_all(text, "").to_string()
}

/// Check if text contains error patterns
fn has_error_pattern(text: &str) -> bool {
    lazy_static! {
        static ref ERROR_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"(?i)error:").unwrap(),
            Regex::new(r"(?i)fatal:").unwrap(),
            Regex::new(r"(?i)panic:").unwrap(),
            Regex::new(r"(?i)unhandled exception").unwrap(),
        ];
    }
    ERROR_PATTERNS.iter().any(|p| p.is_match(text))
}

/// Check if text contains waiting/prompt patterns
fn has_waiting_pattern(text: &str) -> bool {
    lazy_static! {
        static ref WAITING_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"\?\s*$").unwrap(),            // Ends with ?
            Regex::new(r"(?i)\(y/n\)").unwrap(),       // Yes/no prompt
            Regex::new(r"(?i)\[Y/n\]").unwrap(),       // Yes/no bracket
            Regex::new(r"(?i)Press Enter").unwrap(),   // Enter prompt
            Regex::new(r"(?i)Allow.*Deny").unwrap(),   // Permission dialog
            Regex::new(r"(?i)Type something").unwrap(),
            Regex::new(r">\s*\d+\.").unwrap(),         // Numbered option (> 1.)
            Regex::new(r"(?i)waiting for input").unwrap(),
        ];
    }
    WAITING_PATTERNS.iter().any(|p| p.is_match(text))
}

/// Check if text contains running/working patterns
fn has_running_pattern(text: &str) -> bool {
    lazy_static! {
        static ref RUNNING_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"(?i)esc to interrupt").unwrap(), // Claude's working indicator
            Regex::new(r"[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]").unwrap(),       // Spinner characters
            Regex::new(r"(?i)thinking").unwrap(),
            Regex::new(r"(?i)processing").unwrap(),
            Regex::new(r"(?i)running").unwrap(),
        ];
    }
    RUNNING_PATTERNS.iter().any(|p| p.is_match(text))
}

/// Check if text indicates we're in a hook phase (pre/post tool use)
/// Hook output should not trigger status changes
fn in_hook_phase(text: &str) -> bool {
    lazy_static! {
        static ref HOOK_PATTERNS: Vec<Regex> = vec![
            Regex::new(r"(?i)PreToolUse").unwrap(),
            Regex::new(r"(?i)PostToolUse").unwrap(),
            Regex::new(r"(?i)hook:").unwrap(),
        ];
    }
    HOOK_PATTERNS.iter().any(|p| p.is_match(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let text = "\x1b[32mGreen text\x1b[0m";
        assert_eq!(strip_ansi(text), "Green text");
    }

    #[test]
    fn test_waiting_patterns() {
        assert!(has_waiting_pattern("Do you want to continue? "));
        assert!(has_waiting_pattern("Proceed (y/n)"));
        assert!(has_waiting_pattern("Press Enter to continue"));
        assert!(has_waiting_pattern("> 1. First option"));
        assert!(!has_waiting_pattern("Normal output text"));
    }

    #[test]
    fn test_running_patterns() {
        assert!(has_running_pattern("Press esc to interrupt"));
        assert!(has_running_pattern("⠋ Loading..."));
        assert!(!has_running_pattern("Normal output text"));
    }

    #[test]
    fn test_error_patterns() {
        assert!(has_error_pattern("error: something went wrong"));
        assert!(has_error_pattern("Fatal: unable to connect"));
        assert!(!has_error_pattern("No errors here"));
    }

    #[test]
    fn test_tracker_waiting_detection() {
        let mut tracker = StatusTracker::new(SessionStatus::Running);
        let result = tracker.process_output("Do you want to continue (y/n)? ");
        assert_eq!(result, Some(SessionStatus::Waiting));
    }
}
