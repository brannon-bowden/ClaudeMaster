//! IPC client for connecting to the daemon

use interprocess::local_socket::{
    tokio::{prelude::*, RecvHalf, SendHalf, Stream},
    GenericFilePath,
};
use serde_json::Value;
use shared::{get_socket_path, Request, Response};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Default request timeout in seconds
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// IPC client for communicating with the daemon
pub struct IpcClient {
    reader: Arc<Mutex<Option<BufReader<RecvHalf>>>>,
    writer: Arc<Mutex<Option<SendHalf>>>,
    request_id: AtomicU64,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            reader: Arc::new(Mutex::new(None)),
            writer: Arc::new(Mutex::new(None)),
            request_id: AtomicU64::new(1),
        }
    }

    /// Connect to the daemon socket
    /// This is idempotent - calling it when already connected is a no-op
    pub async fn connect(&self) -> Result<(), String> {
        // Check if already connected
        {
            let writer_guard = self.writer.lock().await;
            if writer_guard.is_some() {
                return Ok(()); // Already connected
            }
        }

        let socket_path = get_socket_path().map_err(|e| e.to_string())?;

        if !socket_path.exists() {
            return Err("Daemon socket not found. Is the daemon running?".to_string());
        }

        let name = socket_path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| e.to_string())?;

        let stream = Stream::connect(name)
            .await
            .map_err(|e| format!("Failed to connect to daemon: {}", e))?;

        let (recv_half, send_half) = stream.split();

        {
            let mut reader_guard = self.reader.lock().await;
            *reader_guard = Some(BufReader::new(recv_half));
        }

        {
            let mut writer_guard = self.writer.lock().await;
            *writer_guard = Some(send_half);
        }

        Ok(())
    }

    /// Check if connected to the daemon
    pub async fn is_connected(&self) -> bool {
        let writer_guard = self.writer.lock().await;
        writer_guard.is_some()
    }

    /// Disconnect from the daemon
    pub async fn disconnect(&self) {
        {
            let mut reader_guard = self.reader.lock().await;
            *reader_guard = None;
        }
        {
            let mut writer_guard = self.writer.lock().await;
            *writer_guard = None;
        }
    }

    /// Send a request and wait for the response with timeout
    /// Auto-reconnects if not connected
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        // Auto-reconnect if not connected
        if !self.is_connected().await {
            self.connect().await?;
        }

        let result = timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            self.call_inner(method, params.clone()),
        )
        .await;

        match result {
            Ok(inner_result) => {
                // If there was a connection error, disconnect and retry once
                if let Err(ref e) = inner_result {
                    if e.contains("Failed to send")
                        || e.contains("Failed to read")
                        || e.contains("Not connected")
                        || e.contains("Connection closed")
                    {
                        self.disconnect().await;
                        // Try to reconnect and retry once
                        if self.connect().await.is_ok() {
                            return timeout(
                                Duration::from_secs(REQUEST_TIMEOUT_SECS),
                                self.call_inner(method, params),
                            )
                            .await
                            .map_err(|_| {
                                format!("Request timed out after {}s", REQUEST_TIMEOUT_SECS)
                            })?;
                        }
                    }
                }
                inner_result
            }
            Err(_) => {
                // Timeout - disconnect and return error
                self.disconnect().await;
                Err(format!("Request timed out after {}s", REQUEST_TIMEOUT_SECS))
            }
        }
    }

    /// Internal call implementation without timeout
    async fn call_inner(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = Request {
            id,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request).map_err(|e| e.to_string())? + "\n";

        // Send request
        {
            let mut writer_guard = self.writer.lock().await;
            let writer = writer_guard
                .as_mut()
                .ok_or_else(|| "Not connected to daemon".to_string())?;

            writer
                .write_all(request_json.as_bytes())
                .await
                .map_err(|e| format!("Failed to send request: {}", e))?;
        }

        // Read response - skip any event messages until we get our response
        loop {
            let mut line = String::new();
            {
                let mut reader_guard = self.reader.lock().await;
                let reader = reader_guard
                    .as_mut()
                    .ok_or_else(|| "Not connected to daemon".to_string())?;

                let bytes_read = reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| format!("Failed to read response: {}", e))?;

                if bytes_read == 0 {
                    return Err("Connection closed by daemon".to_string());
                }
            }

            // Try to parse as Response (has "id" field)
            if let Ok(response) = serde_json::from_str::<Response>(&line) {
                if response.id != id {
                    // Not our response, could be a late response from a previous request
                    continue;
                }

                if let Some(error) = response.error {
                    return Err(error.message);
                }

                return response.result.ok_or_else(|| "Empty response".to_string());
            }

            // If it doesn't parse as a Response, it might be an Event - skip it
            // In a real app, you'd want to queue these events for processing
        }
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}
