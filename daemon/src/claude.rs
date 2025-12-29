use regex::Regex;
use shared::SessionStatus;
use std::sync::LazyLock;

/// Patterns for detecting Claude Code's current state
pub struct StatusDetector {
    waiting_patterns: Vec<Regex>,
    running_patterns: Vec<Regex>,
    error_patterns: Vec<Regex>,
    session_id_pattern: Regex,
}

static DETECTOR: LazyLock<StatusDetector> = LazyLock::new(StatusDetector::new);

impl StatusDetector {
    fn new() -> Self {
        Self {
            waiting_patterns: vec![
                Regex::new(r"^>\s*$").unwrap(),                    // Claude's input prompt
                Regex::new(r"╭─+╮\s*$").unwrap(),                  // Response box closed
                Regex::new(r"\?\s*\[Y/n\]").unwrap(),              // Yes/No prompt
                Regex::new(r"\?\s*\[y/N\]").unwrap(),              // No/Yes prompt
                Regex::new(r"Press Enter to continue").unwrap(),  // Pause prompt
                Regex::new(r"Would you like to").unwrap(),        // Action prompt
            ],
            running_patterns: vec![
                Regex::new(r"[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]").unwrap(),           // Spinner characters
                Regex::new(r"Thinking\.\.\.").unwrap(),
                Regex::new(r"Reading .+\.\.\.").unwrap(),
                Regex::new(r"Writing .+\.\.\.").unwrap(),
                Regex::new(r"Searching\.\.\.").unwrap(),
                Regex::new(r"Running .+\.\.\.").unwrap(),
                Regex::new(r"Executing\.\.\.").unwrap(),
            ],
            error_patterns: vec![
                Regex::new(r"Error:").unwrap(),
                Regex::new(r"APIError").unwrap(),
                Regex::new(r"Rate limit").unwrap(),
                Regex::new(r"Connection refused").unwrap(),
                Regex::new(r"ECONNREFUSED").unwrap(),
                Regex::new(r"timed out").unwrap(),
            ],
            // Match session ID from Claude output (appears at startup or in status)
            session_id_pattern: Regex::new(r"session[:\s]+([a-f0-9-]{36})").unwrap(),
        }
    }

    /// Detect status from a chunk of terminal output
    pub fn detect_status(&self, text: &str) -> Option<SessionStatus> {
        // Check for errors first (highest priority)
        for pattern in &self.error_patterns {
            if pattern.is_match(text) {
                return Some(SessionStatus::Error);
            }
        }

        // Check for running indicators
        for pattern in &self.running_patterns {
            if pattern.is_match(text) {
                return Some(SessionStatus::Running);
            }
        }

        // Check for waiting indicators
        for pattern in &self.waiting_patterns {
            if pattern.is_match(text) {
                return Some(SessionStatus::Waiting);
            }
        }

        None
    }

    /// Extract Claude session ID from terminal output
    pub fn extract_session_id(&self, text: &str) -> Option<String> {
        self.session_id_pattern
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }
}

/// Get the global status detector
pub fn detector() -> &'static StatusDetector {
    &DETECTOR
}

/// Convenience function to detect status
pub fn detect_status(text: &str) -> Option<SessionStatus> {
    DETECTOR.detect_status(text)
}

/// Convenience function to extract session ID
pub fn extract_session_id(text: &str) -> Option<String> {
    DETECTOR.extract_session_id(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_waiting() {
        assert_eq!(detect_status("> "), Some(SessionStatus::Waiting));
        assert_eq!(detect_status("? [Y/n]"), Some(SessionStatus::Waiting));
    }

    #[test]
    fn test_detect_running() {
        assert_eq!(detect_status("⠋ Thinking..."), Some(SessionStatus::Running));
        assert_eq!(detect_status("Reading file.rs..."), Some(SessionStatus::Running));
    }

    #[test]
    fn test_detect_error() {
        assert_eq!(detect_status("Error: something went wrong"), Some(SessionStatus::Error));
        assert_eq!(detect_status("APIError: rate limited"), Some(SessionStatus::Error));
    }

    #[test]
    fn test_extract_session_id() {
        let text = "Resuming session: a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        assert_eq!(
            extract_session_id(text),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string())
        );
    }
}
