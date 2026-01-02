use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::claude_resolver::ClaudeResolver;

pub struct PtyInstance {
    pub pair: PtyPair,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
}

pub struct PtyManager {
    instances: RwLock<HashMap<Uuid, Arc<Mutex<PtyInstance>>>>,
    claude_resolver: ClaudeResolver,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
            claude_resolver: ClaudeResolver::new(),
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

        // Try direct Claude execution first, fall back to shell wrapper if needed
        let cmd = if let Some(claude_path) = self.claude_resolver.claude_path() {
            self.build_direct_command(claude_path, working_dir, resume_session_id)?
        } else {
            warn!("Claude binary not found, falling back to shell wrapper");
            self.build_shell_command(working_dir, resume_session_id)?
        };

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

    /// Build command for direct Claude binary execution (preferred method)
    /// Avoids shell startup noise for cleaner PTY output
    fn build_direct_command(
        &self,
        claude_path: &std::path::PathBuf,
        working_dir: &Path,
        resume_session_id: Option<&str>,
    ) -> Result<CommandBuilder> {
        info!(
            "PTY spawn: direct execution {:?} cwd={:?}",
            claude_path, working_dir
        );

        let mut cmd = CommandBuilder::new(claude_path);
        if let Some(claude_session_id) = resume_session_id {
            cmd.arg("--resume");
            cmd.arg(claude_session_id);
        }
        cmd.cwd(working_dir);

        // Set environment from resolver
        for (key, value) in self.claude_resolver.build_env() {
            cmd.env(&key, &value);
        }

        // Remove CI detection variables
        for var in ClaudeResolver::env_vars_to_remove() {
            cmd.env_remove(var);
        }

        Ok(cmd)
    }

    /// Build command using shell wrapper (fallback method)
    /// Used when Claude binary path cannot be resolved directly
    fn build_shell_command(
        &self,
        working_dir: &Path,
        resume_session_id: Option<&str>,
    ) -> Result<CommandBuilder> {
        let claude_cmd = if let Some(claude_session_id) = resume_session_id {
            format!("claude --resume {}", claude_session_id)
        } else {
            "claude".to_string()
        };

        // Get home directory
        let home_dir = std::env::var("HOME")
            .ok()
            .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    format!("/Users/{}", whoami::username())
                } else {
                    format!("/home/{}", whoami::username())
                }
            });

        // Get the user's shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "/bin/zsh".to_string()
            } else {
                "/bin/bash".to_string()
            }
        });

        info!(
            "PTY spawn (shell): shell={} cmd='{}' cwd={:?} HOME={}",
            shell, claude_cmd, working_dir, home_dir
        );

        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-li"); // Login + Interactive shell
        cmd.arg("-c");
        cmd.arg(&claude_cmd);
        cmd.cwd(working_dir);

        // Set core environment
        cmd.env("HOME", &home_dir);
        cmd.env("USER", whoami::username());
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("FORCE_COLOR", "1");
        cmd.env("TERM_PROGRAM", "xterm");
        cmd.env("LC_ALL", "en_US.UTF-8");

        // Remove CI-related environment variables
        for var in ClaudeResolver::env_vars_to_remove() {
            cmd.env_remove(var);
        }

        Ok(cmd)
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
