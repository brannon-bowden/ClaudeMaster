//! Shared path utilities for daemon and GUI

use anyhow::Result;
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

/// Get the application data directory
pub fn get_data_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "claudemaster", "claude-master")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let data_dir = proj_dirs.data_dir().to_path_buf();
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Get the path to the config file
pub fn get_config_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("config.toml"))
}

/// Get the state directory
pub fn get_state_dir() -> Result<PathBuf> {
    let state_dir = get_data_dir()?.join("state");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir)
}

/// Get the daemon socket path
/// On Unix: returns a path to a Unix socket file
/// On Windows: returns a path that will be used as a named pipe identifier
pub fn get_socket_path() -> Result<PathBuf> {
    #[cfg(unix)]
    {
        Ok(get_data_dir()?.join("daemon.sock"))
    }
    #[cfg(windows)]
    {
        // On Windows, we use the data dir path to create a unique named pipe
        // The actual pipe path will be \\.\pipe\claude-master-daemon
        // But we return a file path that the interprocess crate can convert
        Ok(get_data_dir()?.join("daemon.sock"))
    }
}

/// Get the logs directory
pub fn get_logs_dir() -> Result<PathBuf> {
    let logs_dir = get_data_dir()?.join("logs");
    fs::create_dir_all(&logs_dir)?;
    Ok(logs_dir)
}
