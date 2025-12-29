use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::collections::HashMap;
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
        self.spawn_with_resume(session_id, working_dir, rows, cols, output_tx, None).await
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

        let mut cmd = CommandBuilder::new("claude");
        cmd.cwd(working_dir);

        // Add --resume flag if forking from existing session
        if let Some(claude_session_id) = resume_session_id {
            cmd.arg("--resume");
            cmd.arg(claude_session_id);
        }

        let child = pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let instance = Arc::new(Mutex::new(PtyInstance {
            pair,
            child,
            writer,
        }));

        {
            let mut instances = self.instances.write().await;
            instances.insert(session_id, instance);
        }

        // Spawn reader task
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if output_tx.send((session_id, buf[..n].to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("PTY read error: {}", e);
                        break;
                    }
                }
            }
            info!("PTY reader for {} exited", session_id);
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
