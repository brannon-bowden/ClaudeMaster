use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info};
use uuid::Uuid;

pub struct PtyInstance {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
}

pub struct PtyManager {
    instances: RwLock<HashMap<Uuid, Arc<Mutex<PtyInstance>>>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
        }
    }

    pub async fn spawn(
        &self,
        session_id: Uuid,
        working_dir: &Path,
        rows: u16,
        cols: u16,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
    ) -> Result<()> {
        self.spawn_with_resume(session_id, working_dir, rows, cols, output_tx, None)
            .await
    }

    pub async fn spawn_with_resume(
        &self,
        session_id: Uuid,
        working_dir: &Path,
        rows: u16,
        cols: u16,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        resume_session_id: Option<&str>,
    ) -> Result<()> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        // Build the claude command with optional --resume flag
        let claude_cmd = if let Some(claude_session_id) = resume_session_id {
            format!("claude --resume {}", claude_session_id)
        } else {
            "claude".to_string()
        };

        // Get home directory - critical for login shell to work
        // When running as a sidecar, HOME may not be set in environment
        let home_dir = env::var("HOME")
            .ok()
            .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                // Last resort fallback for macOS
                if cfg!(target_os = "macos") {
                    format!("/Users/{}", whoami::username())
                } else {
                    format!("/home/{}", whoami::username())
                }
            });

        // Get the user's shell
        // Try SHELL env, then check passwd entry via dirs, or use system default
        let shell = env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "/bin/zsh".to_string()
            } else {
                "/bin/bash".to_string()
            }
        });

        info!(
            "PTY spawn: shell={} cmd='{}' cwd={:?} HOME={}",
            shell, claude_cmd, working_dir, home_dir
        );

        // Use login interactive shell (-li) to source user's full profile
        // -l sources .zprofile/.bash_profile, -i sources .zshrc/.bashrc
        // This ensures PATH includes npm global binaries, homebrew, etc.
        // The -c flag runs the command and exits
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-li"); // Login + Interactive shell - sources all profile files
        cmd.arg("-c"); // Run command
        cmd.arg(&claude_cmd);
        cmd.cwd(working_dir);

        // Always set HOME - critical for login shell to find profile and for claude to work
        cmd.env("HOME", &home_dir);
        // Also set USER if not set
        cmd.env("USER", whoami::username());
        // Set TERM - critical for TUI apps like Claude Code to use correct escape sequences
        // xterm.js emulates xterm-256color
        cmd.env("TERM", "xterm-256color");
        // Also set COLORTERM for apps that check for true color support
        cmd.env("COLORTERM", "truecolor");

        // Force interactive/TUI mode for Ink-based apps like Claude Code
        // Ink checks these to decide whether to use alternate screen buffer
        cmd.env("FORCE_COLOR", "1"); // Force color output
        cmd.env("TERM_PROGRAM", "xterm"); // Identify as xterm-compatible
        cmd.env("LC_ALL", "en_US.UTF-8"); // Ensure UTF-8 locale

        // Remove CI-related environment variables that cause TUI apps to use non-interactive mode
        // The ci-info package checks many of these - we remove the common ones
        cmd.env_remove("CI");
        cmd.env_remove("CONTINUOUS_INTEGRATION");
        cmd.env_remove("BUILD_NUMBER");
        cmd.env_remove("BUILD_ID");
        cmd.env_remove("GITHUB_ACTIONS");
        cmd.env_remove("GITLAB_CI");
        cmd.env_remove("CIRCLECI");
        cmd.env_remove("TRAVIS");
        cmd.env_remove("JENKINS_URL");
        cmd.env_remove("HUDSON_URL");
        cmd.env_remove("BUILDKITE");
        cmd.env_remove("TEAMCITY_VERSION");
        cmd.env_remove("BITBUCKET_COMMIT");
        cmd.env_remove("CODEBUILD_BUILD_ARN");
        cmd.env_remove("DRONE");
        cmd.env_remove("VERCEL");
        cmd.env_remove("NETLIFY");
        cmd.env_remove("RENDER");
        cmd.env_remove("SEMAPHORE");
        cmd.env_remove("APPVEYOR");
        cmd.env_remove("TF_BUILD"); // Azure Pipelines

        info!("PTY spawn: executing spawn_command...");
        let child = pair.slave.spawn_command(cmd)?;
        info!("PTY spawn: process spawned successfully");

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;
        info!(
            "PTY spawn: writer/reader obtained for session {}",
            session_id
        );

        let instance = Arc::new(Mutex::new(PtyInstance {
            pair,
            child,
            writer,
        }));

        {
            let mut instances = self.instances.write().await;
            instances.insert(session_id, instance);
        }

        // Spawn reader task in a dedicated thread since PTY read is blocking I/O
        // Capture the tokio runtime handle before spawning
        let rt_handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut total_bytes = 0usize;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        info!(
                            "PTY reader for {} got EOF after {} bytes",
                            session_id, total_bytes
                        );
                        break;
                    }
                    Ok(n) => {
                        total_bytes += n;
                        let data = buf[..n].to_vec();
                        // Use the captured runtime handle to send asynchronously
                        if rt_handle
                            .block_on(output_tx.send((session_id, data)))
                            .is_err()
                        {
                            info!(
                                "PTY reader for {} channel closed after {} bytes",
                                session_id, total_bytes
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        error!(
                            "PTY read error for {}: {} (after {} bytes)",
                            session_id, e, total_bytes
                        );
                        break;
                    }
                }
            }
            info!(
                "PTY reader for {} exited (total {} bytes read)",
                session_id, total_bytes
            );
        });

        Ok(())
    }

    pub async fn write(&self, session_id: Uuid, data: &[u8]) -> Result<()> {
        let instances = self.instances.read().await;
        if let Some(instance) = instances.get(&session_id) {
            let mut inst = instance.lock().await;
            inst.writer.write_all(data)?;
            inst.writer.flush()?;
        }
        Ok(())
    }

    pub async fn resize(&self, session_id: Uuid, rows: u16, cols: u16) -> Result<()> {
        let instances = self.instances.read().await;
        if let Some(instance) = instances.get(&session_id) {
            let inst = instance.lock().await;
            inst.pair.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }

    pub async fn kill(&self, session_id: Uuid) -> Result<()> {
        let mut instances = self.instances.write().await;
        if let Some(instance) = instances.remove(&session_id) {
            let mut inst = instance.lock().await;
            inst.child.kill()?;
        }
        Ok(())
    }

    pub async fn is_alive(&self, session_id: Uuid) -> bool {
        let instances = self.instances.read().await;
        if let Some(instance) = instances.get(&session_id) {
            let mut inst = instance.lock().await;
            matches!(inst.child.try_wait(), Ok(None))
        } else {
            false
        }
    }
}
