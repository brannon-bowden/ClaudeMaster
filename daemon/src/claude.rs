use regex::Regex;
use shared::SessionStatus;
use std::sync::LazyLock;
use tracing::debug;

/// Patterns for detecting Claude Code's current state
pub struct StatusDetector {
    running_patterns: Vec<(Regex, &'static str)>,
    error_patterns: Vec<(Regex, &'static str)>,
    /// Patterns that indicate Claude is in a transitional state (running hooks)
    /// These override running detection because hooks run AFTER Claude finishes work
    hook_patterns: Vec<Regex>,
    session_id_pattern: Regex,
    ansi_strip: Regex,
}

static DETECTOR: LazyLock<StatusDetector> = LazyLock::new(StatusDetector::new);

impl StatusDetector {
    fn new() -> Self {
        Self {
            // Running/busy indicators - these mean Claude is actively working
            running_patterns: vec![
                // "esc to interrupt" is THE key indicator that Claude is actively working
                // This appears in Claude Code's status bar during processing
                // Case-insensitive to catch ESC, Esc, esc variations
                (
                    Regex::new(r"(?i)esc to interrupt").unwrap(),
                    "esc_interrupt",
                ),
                (Regex::new(r"(?i)esc to stop").unwrap(), "esc_stop"),
                (Regex::new(r"(?i)press esc").unwrap(), "press_esc"),
                // Spinner characters (braille animation) - these appear during loading
                (Regex::new(r"[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]").unwrap(), "spinner"),
                // Progress/loading indicators
                (Regex::new(r"\.\.\.").unwrap(), "ellipsis"),
                // Thinking indicators with token count (appears in status bar)
                (Regex::new(r"(?i)thinking").unwrap(), "thinking"),
                (Regex::new(r"(?i)\d+\s*tokens").unwrap(), "tokens"),
                (Regex::new(r"(?i)contemplating").unwrap(), "contemplating"),
                (Regex::new(r"(?i)processing").unwrap(), "processing"),
                // Tool use indicators
                (Regex::new(r"(?i)calling tool").unwrap(), "calling_tool"),
                (Regex::new(r"(?i)using tool").unwrap(), "using_tool"),
            ],
            error_patterns: vec![
                (Regex::new(r"Error:").unwrap(), "error"),
                (Regex::new(r"APIError").unwrap(), "api_error"),
                (Regex::new(r"Rate limit").unwrap(), "rate_limit"),
                (Regex::new(r"Connection refused").unwrap(), "conn_refused"),
                (Regex::new(r"ECONNREFUSED").unwrap(), "econnrefused"),
                (Regex::new(r"timed out").unwrap(), "timeout"),
            ],
            // Hook patterns - Claude shows "esc to interrupt" during hook execution,
            // but hooks run AFTER Claude finishes work, so this is a transitional state.
            // We should NOT detect Running when hooks are running.
            hook_patterns: vec![
                Regex::new(r"(?i)running\s+\w*\s*hook").unwrap(), // "running stop hook", "running hook"
                Regex::new(r"(?i)stop\s+hook").unwrap(),          // "stop hook"
                Regex::new(r"(?i)pre-?commit").unwrap(),          // "pre-commit hook"
            ],
            // Match session ID from Claude output (appears at startup or in status)
            session_id_pattern: Regex::new(r"session[:\s]+([a-f0-9-]{36})").unwrap(),
            // Pattern to strip ANSI escape codes for cleaner matching
            // This handles:
            // - Standard CSI sequences: \x1b[0m, \x1b[32m, \x1b[1;34m, etc.
            // - DEC private modes: \x1b[?25l (hide cursor), \x1b[?2004h (bracketed paste)
            // - OSC sequences: \x1b]0;title\x07 (window title)
            // - Other CSI with special chars: \x1b[>c, \x1b[=c, etc.
            ansi_strip: Regex::new(r"\x1b(?:\[[0-9;?]*[a-zA-Z~]|\].*?\x07|\[[=>][0-9;]*[a-zA-Z])")
                .unwrap(),
        }
    }

    /// Strip ANSI escape codes from text for cleaner pattern matching
    fn strip_ansi(&self, text: &str) -> String {
        self.ansi_strip.replace_all(text, "").to_string()
    }

    /// Detect status from a chunk of terminal output
    ///
    /// Detection strategy (simplified agent-deck approach):
    /// 1. Check for error patterns (highest priority)
    /// 2. Check for running/busy indicators (esc to interrupt, spinners)
    /// 3. If NO busy indicators found -> Waiting (the key insight from agent-deck)
    ///
    /// The key insight: We don't need to detect "waiting" patterns.
    /// If Claude is NOT showing busy indicators, it's waiting for input.
    /// The debouncing in session_manager prevents flapping.
    pub fn detect_status(&self, text: &str) -> Option<SessionStatus> {
        // Strip ANSI escape codes for cleaner matching
        let clean_text = self.strip_ansi(text);

        // Skip empty chunks and pure control character chunks
        // Use the raw length (not trimmed) because whitespace can be significant
        // (e.g., "> " is the input prompt with trailing space)
        if clean_text.is_empty()
            || clean_text.len() < 2
            || clean_text.chars().all(|c| c.is_control())
        {
            return None;
        }

        // Log a sample of the cleaned text for debugging
        let sample: String = clean_text.chars().take(200).collect();
        let printable: String = sample
            .chars()
            .map(|c| if c.is_control() && c != '\n' { '.' } else { c })
            .collect();
        debug!("Status check on: {:?}", printable);

        // Check for errors first (highest priority)
        for (pattern, name) in &self.error_patterns {
            if pattern.is_match(&clean_text) {
                debug!("Status detected: Error (pattern: {})", name);
                return Some(SessionStatus::Error);
            }
        }

        // Check if we're in a hook phase (transitional state)
        // Hooks run AFTER Claude finishes work but still show "esc to interrupt"
        // We should NOT detect Running when in hook phase
        let in_hook_phase = self.hook_patterns.iter().any(|p| p.is_match(&clean_text));
        if in_hook_phase {
            debug!("Hook phase detected - skipping running detection");
        }

        // Check for running/busy indicators
        // If we see these AND we're not in a hook phase, Claude is definitely working
        if !in_hook_phase {
            for (pattern, name) in &self.running_patterns {
                if pattern.is_match(&clean_text) {
                    debug!("Status detected: Running (pattern: {})", name);
                    return Some(SessionStatus::Running);
                }
            }
        }

        // No busy indicators found -> Waiting
        // This is the key insight from agent-deck: the ABSENCE of busy indicators
        // means Claude is waiting for input. The debouncing in session_manager
        // (2 second cooldown from Running) prevents flapping.
        debug!(
            "Status detected: Waiting (no busy indicators in: {:?})",
            printable
        );
        Some(SessionStatus::Waiting)
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
#[allow(dead_code)]
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
        // With "absence of busy indicators = Waiting" approach,
        // any non-trivial chunk without running patterns is Waiting

        // Claude's prompt
        assert_eq!(detect_status("> "), Some(SessionStatus::Waiting));
        // Prompt on its own line within larger text
        assert_eq!(
            detect_status("some output\n> "),
            Some(SessionStatus::Waiting)
        );
        // Yes/No prompts
        assert_eq!(detect_status("? [Y/n]"), Some(SessionStatus::Waiting));
        assert_eq!(detect_status("? [y/N]"), Some(SessionStatus::Waiting));
        // With ANSI codes (after stripping, should match)
        assert_eq!(
            detect_status("\x1b[32m> \x1b[0m"),
            Some(SessionStatus::Waiting)
        );
        // Regular content without busy indicators = Waiting
        assert_eq!(detect_status(">\n"), Some(SessionStatus::Waiting));
        // Too short - skip analysis
        assert_eq!(detect_status(">"), None); // Only 1 char after ANSI strip
    }

    #[test]
    fn test_detect_running() {
        // "esc to interrupt" is the primary indicator
        assert_eq!(
            detect_status("esc to interrupt"),
            Some(SessionStatus::Running)
        );
        assert_eq!(
            detect_status("(esc to interrupt)"),
            Some(SessionStatus::Running)
        );
        assert_eq!(
            detect_status("· esc to interrupt"),
            Some(SessionStatus::Running)
        );
        // Spinner characters
        assert_eq!(detect_status("⠋"), Some(SessionStatus::Running));
        assert_eq!(detect_status("⠙"), Some(SessionStatus::Running));
        // Thinking with tokens
        assert_eq!(
            detect_status("thinking 1234 tokens"),
            Some(SessionStatus::Running)
        );
        // With ANSI codes
        assert_eq!(
            detect_status("\x1b[33m⠹\x1b[0m esc to interrupt"),
            Some(SessionStatus::Running)
        );
    }

    #[test]
    fn test_detect_error() {
        assert_eq!(
            detect_status("Error: something went wrong"),
            Some(SessionStatus::Error)
        );
        assert_eq!(
            detect_status("APIError: rate limited"),
            Some(SessionStatus::Error)
        );
    }

    #[test]
    fn test_hook_phase_not_running() {
        // When Claude is running hooks, it still shows "esc to interrupt" but
        // we should detect this as Waiting, not Running, because hooks run
        // AFTER Claude finishes its main work.

        // The exact pattern from the logs
        assert_eq!(
            detect_status("✻ Ruminating… (esc to interrupt · running stop hook)"),
            Some(SessionStatus::Waiting)
        );

        // Other hook variations
        assert_eq!(
            detect_status("(esc to interrupt · stop hook)"),
            Some(SessionStatus::Waiting)
        );
        assert_eq!(
            detect_status("running hook (esc to interrupt)"),
            Some(SessionStatus::Waiting)
        );
        assert_eq!(
            detect_status("pre-commit hook running"),
            Some(SessionStatus::Waiting)
        );

        // But regular "esc to interrupt" without hooks should still be Running
        assert_eq!(
            detect_status("✻ Thinking… (esc to interrupt)"),
            Some(SessionStatus::Running)
        );
    }

    #[test]
    fn test_extract_session_id() {
        let text = "Resuming session: a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        assert_eq!(
            extract_session_id(text),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string())
        );
    }

    #[test]
    fn test_strip_ansi() {
        let detector = StatusDetector::new();
        // Standard color codes
        assert_eq!(detector.strip_ansi("\x1b[32mHello\x1b[0m"), "Hello");
        assert_eq!(detector.strip_ansi("\x1b[1;34mBlue\x1b[0m"), "Blue");
        // DEC private modes (cursor hide/show, bracketed paste)
        assert_eq!(detector.strip_ansi("\x1b[?25lHidden\x1b[?25h"), "Hidden");
        assert_eq!(detector.strip_ansi("\x1b[?2004hText\x1b[?2004l"), "Text");
        // Mixed content
        assert_eq!(
            detector.strip_ansi("\x1b[?25l\x1b[32m> \x1b[0m\x1b[?25h"),
            "> "
        );
    }
}
