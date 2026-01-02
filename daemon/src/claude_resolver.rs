// Claude binary resolver - finds the Claude Code binary and builds its environment
// Avoids shell wrapper noise by spawning claude directly

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, info, warn};

/// Resolves the path to the Claude Code binary and provides environment setup
pub struct ClaudeResolver {
    claude_path: Option<PathBuf>,
}

impl ClaudeResolver {
    /// Create a new resolver, immediately attempting to find the claude binary
    pub fn new() -> Self {
        let claude_path = Self::find_claude();
        if let Some(ref path) = claude_path {
            info!("Claude binary found at: {:?}", path);
        } else {
            warn!("Claude binary not found - sessions will fail to start");
        }
        Self { claude_path }
    }

    /// Get the resolved claude binary path
    pub fn claude_path(&self) -> Option<&PathBuf> {
        self.claude_path.as_ref()
    }

    /// Check if claude was found
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        self.claude_path.is_some()
    }

    /// Find the claude binary using multiple strategies
    fn find_claude() -> Option<PathBuf> {
        // Strategy 1: Use the `which` crate (checks PATH)
        if let Ok(path) = which::which("claude") {
            debug!("Found claude via which crate: {:?}", path);
            return Some(path);
        }

        // Strategy 2: Check common installation paths
        let home = dirs::home_dir();
        let common_paths: Vec<PathBuf> = [
            // npm global installations
            home.as_ref().map(|h| h.join(".npm-global/bin/claude")),
            home.as_ref().map(|h| h.join(".nvm/versions/node").join("*").join("bin/claude")),
            // Homebrew on macOS
            Some(PathBuf::from("/opt/homebrew/bin/claude")),
            Some(PathBuf::from("/usr/local/bin/claude")),
            // Local bin
            home.as_ref().map(|h| h.join(".local/bin/claude")),
            // Cargo installs (if distributed via cargo)
            home.as_ref().map(|h| h.join(".cargo/bin/claude")),
        ]
        .into_iter()
        .flatten()
        .collect();

        for path in common_paths {
            // Handle glob patterns (for nvm)
            if path.to_string_lossy().contains('*') {
                if let Some(expanded) = Self::expand_glob(&path) {
                    debug!("Found claude via glob expansion: {:?}", expanded);
                    return Some(expanded);
                }
            } else if path.exists() {
                debug!("Found claude at common path: {:?}", path);
                return Some(path);
            }
        }

        // Strategy 3: Shell-based which (last resort, handles complex shell setups)
        if let Some(path) = Self::shell_which() {
            debug!("Found claude via shell which: {:?}", path);
            return Some(path);
        }

        None
    }

    /// Expand glob pattern to find claude (for nvm-style paths)
    fn expand_glob(pattern: &PathBuf) -> Option<PathBuf> {
        let pattern_str = pattern.to_string_lossy();
        if let Ok(entries) = glob::glob(&pattern_str) {
            for entry in entries.flatten() {
                if entry.exists() {
                    return Some(entry);
                }
            }
        }
        None
    }

    /// Use shell to run `which claude` - handles complex shell configurations
    fn shell_which() -> Option<PathBuf> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "/bin/zsh".to_string()
            } else {
                "/bin/bash".to_string()
            }
        });

        let output = Command::new(&shell)
            .args(["-lc", "which claude"])
            .output()
            .ok()?;

        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&path_str);
            if !path_str.is_empty() && path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Build the environment variables needed for Claude to run properly
    pub fn build_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Get home directory
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    format!("/Users/{}", whoami::username())
                } else {
                    format!("/home/{}", whoami::username())
                }
            });

        // Core environment
        env.insert("HOME".into(), home.clone());
        env.insert("USER".into(), whoami::username());

        // Terminal environment - critical for TUI apps
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());

        // Locale for proper Unicode support
        env.insert("LANG".into(), "en_US.UTF-8".into());
        env.insert("LC_ALL".into(), "en_US.UTF-8".into());

        // Force color/TUI mode
        env.insert("FORCE_COLOR".into(), "1".into());
        env.insert("TERM_PROGRAM".into(), "xterm".into());

        // Inherit PATH from current environment (includes npm, homebrew, etc.)
        if let Ok(path) = std::env::var("PATH") {
            env.insert("PATH".into(), path);
        } else {
            // Construct a reasonable PATH if not available
            let default_path = format!(
                "{}/.local/bin:{}/.npm-global/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
                home, home
            );
            env.insert("PATH".into(), default_path);
        }

        env
    }

    /// Get environment variables that should be explicitly removed
    /// (CI detection variables that cause non-interactive mode)
    pub fn env_vars_to_remove() -> &'static [&'static str] {
        &[
            "CI",
            "CONTINUOUS_INTEGRATION",
            "BUILD_NUMBER",
            "BUILD_ID",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "CIRCLECI",
            "TRAVIS",
            "JENKINS_URL",
            "HUDSON_URL",
            "BUILDKITE",
            "TEAMCITY_VERSION",
            "BITBUCKET_COMMIT",
            "CODEBUILD_BUILD_ARN",
            "DRONE",
            "VERCEL",
            "NETLIFY",
            "RENDER",
            "SEMAPHORE",
            "APPVEYOR",
            "TF_BUILD",
        ]
    }
}

impl Default for ClaudeResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_env_contains_essentials() {
        let resolver = ClaudeResolver::new();
        let env = resolver.build_env();

        assert!(env.contains_key("HOME"));
        assert!(env.contains_key("USER"));
        assert!(env.contains_key("TERM"));
        assert!(env.contains_key("PATH"));
        assert_eq!(env.get("TERM"), Some(&"xterm-256color".to_string()));
    }

    #[test]
    fn test_env_vars_to_remove() {
        let vars = ClaudeResolver::env_vars_to_remove();
        assert!(vars.contains(&"CI"));
        assert!(vars.contains(&"GITHUB_ACTIONS"));
    }
}
