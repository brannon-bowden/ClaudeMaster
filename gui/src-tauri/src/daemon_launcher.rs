//! Daemon launcher - manages daemon as a LaunchAgent for persistence
//!
//! The daemon runs as a macOS LaunchAgent, which means:
//! - It starts automatically on user login
//! - It stays running when the GUI closes
//! - Sessions persist across GUI restarts
//! - It restarts automatically if it crashes

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tauri::Manager;
use tracing::{info, warn};

const LAUNCHAGENT_LABEL: &str = "com.claudemaster.daemon";
const DAEMON_BINARY_NAME: &str = "claude-master-daemon";

/// Get the path to the LaunchAgent plist
fn get_plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join("Library/LaunchAgents/com.claudemaster.daemon.plist"))
}

/// Get the path where we install the daemon binary
fn get_installed_daemon_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join("Library/Application Support/com.claudemaster.claude-master/bin/claude-master-daemon"))
}

/// Get the log file path
fn get_log_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join("Library/Logs/claude-master-daemon.log"))
}

/// Get the bundled daemon path from the app bundle
fn get_bundled_daemon_path(app: &tauri::AppHandle) -> Result<PathBuf> {
    let resource_path = app
        .path()
        .resource_dir()
        .context("Could not get resource directory")?;

    // Tauri 2.x bundles sidecars in Contents/MacOS without the target triple suffix
    // Check MacOS directory first (where Tauri actually puts it)
    if let Some(contents_dir) = resource_path.parent() {
        let macos_path = contents_dir.join("MacOS").join(DAEMON_BINARY_NAME);
        if macos_path.exists() {
            return Ok(macos_path);
        }
    }

    // Fallback: check Resources directory with target suffix (older Tauri versions)
    #[cfg(target_arch = "aarch64")]
    let sidecar_name = format!("{}-aarch64-apple-darwin", DAEMON_BINARY_NAME);
    #[cfg(target_arch = "x86_64")]
    let sidecar_name = format!("{}-x86_64-apple-darwin", DAEMON_BINARY_NAME);

    let sidecar_path = resource_path.join(&sidecar_name);
    if sidecar_path.exists() {
        return Ok(sidecar_path);
    }

    Err(anyhow::anyhow!(
        "Could not find bundled daemon in MacOS or Resources directory"
    ))
}

/// Calculate SHA256 hash of a file
fn file_hash(path: &Path) -> Result<String> {
    let contents = fs::read(path).context("Failed to read file for hashing")?;
    let hash = Sha256::digest(&contents);
    Ok(format!("{:x}", hash))
}

/// Check if the installed daemon needs to be updated
fn needs_update(installed: &Path, bundled: &Path) -> Result<bool> {
    if !installed.exists() {
        return Ok(true);
    }

    let installed_hash = file_hash(installed)?;
    let bundled_hash = file_hash(bundled)?;

    Ok(installed_hash != bundled_hash)
}

/// Generate the LaunchAgent plist content
fn generate_plist(bin_path: &Path, log_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>{}</string>

    <key>StandardErrorPath</key>
    <string>{}</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
</dict>
</plist>
"#,
        LAUNCHAGENT_LABEL,
        bin_path.display(),
        log_path.display(),
        log_path.display()
    )
}

/// Install the LaunchAgent plist
fn install_launch_agent(plist_path: &Path, bin_path: &Path, log_path: &Path) -> Result<()> {
    let plist_content = generate_plist(bin_path, log_path);

    // Create LaunchAgents directory if needed
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent).context("Failed to create LaunchAgents directory")?;
    }

    // Write plist file
    fs::write(plist_path, plist_content).context("Failed to write LaunchAgent plist")?;

    info!("Installed LaunchAgent at {:?}", plist_path);
    Ok(())
}

/// Load the LaunchAgent (start the daemon)
fn load_launch_agent(plist_path: &Path) -> Result<()> {
    let status = Command::new("launchctl")
        .args(["load", "-w", plist_path.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl load")?;

    if !status.success() {
        warn!("launchctl load returned non-zero status: {:?}", status);
    }

    info!("Loaded LaunchAgent");
    Ok(())
}

/// Unload the LaunchAgent (stop the daemon)
fn unload_launch_agent(plist_path: &Path) -> Result<()> {
    let status = Command::new("launchctl")
        .args(["unload", plist_path.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl unload")?;

    if !status.success() {
        // This is often expected if the agent wasn't loaded
        info!("launchctl unload returned non-zero status (may be expected): {:?}", status);
    }

    Ok(())
}

/// Check if the daemon is running by checking launchctl
fn is_launchagent_loaded() -> bool {
    let output = Command::new("launchctl")
        .args(["list", LAUNCHAGENT_LABEL])
        .output();

    match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Copy daemon binary to installation location
fn install_daemon_binary(bundled: &Path, installed: &Path) -> Result<()> {
    // Create parent directory if needed
    if let Some(parent) = installed.parent() {
        fs::create_dir_all(parent).context("Failed to create daemon bin directory")?;
    }

    // Copy the binary
    fs::copy(bundled, installed).context("Failed to copy daemon binary")?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(installed)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(installed, perms)?;
    }

    info!("Installed daemon binary at {:?}", installed);
    Ok(())
}

/// Ensure the daemon is running, installing/updating as needed
pub async fn ensure_daemon_running(app: &tauri::AppHandle) -> Result<()> {
    let plist_path = get_plist_path()?;
    let installed_path = get_installed_daemon_path()?;
    let log_path = get_log_path()?;
    let bundled_path = get_bundled_daemon_path(app)?;

    info!("Checking daemon status...");
    info!("  Bundled daemon: {:?}", bundled_path);
    info!("  Installed daemon: {:?}", installed_path);
    info!("  LaunchAgent plist: {:?}", plist_path);

    // Check if we need to update the daemon binary
    let update_needed = needs_update(&installed_path, &bundled_path)?;

    if update_needed {
        info!("Daemon binary needs update");

        // Stop the daemon if it's running
        if plist_path.exists() && is_launchagent_loaded() {
            info!("Stopping existing daemon for update...");
            unload_launch_agent(&plist_path)?;
            // Give it time to shut down gracefully
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Install the new binary
        install_daemon_binary(&bundled_path, &installed_path)?;
    }

    // Install LaunchAgent if missing
    if !plist_path.exists() {
        info!("Installing LaunchAgent...");
        install_launch_agent(&plist_path, &installed_path, &log_path)?;
    }

    // Load the LaunchAgent if not loaded
    if !is_launchagent_loaded() {
        info!("Loading LaunchAgent...");
        load_launch_agent(&plist_path)?;
        // Give daemon time to start
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    info!("Daemon is running");
    Ok(())
}

/// Uninstall the daemon completely (for clean app removal)
pub fn uninstall_daemon() -> Result<()> {
    let plist_path = get_plist_path()?;
    let app_support = dirs::home_dir()
        .context("Could not find home directory")?
        .join("Library/Application Support/com.claudemaster.claude-master");
    let log_path = get_log_path()?;

    info!("Uninstalling daemon...");

    // Stop and unload service
    if plist_path.exists() {
        let _ = unload_launch_agent(&plist_path);
        fs::remove_file(&plist_path).ok();
        info!("Removed LaunchAgent plist");
    }

    // Remove app support directory (bin, socket, state)
    if app_support.exists() {
        fs::remove_dir_all(&app_support).ok();
        info!("Removed app support directory");
    }

    // Remove log file
    if log_path.exists() {
        fs::remove_file(&log_path).ok();
        info!("Removed log file");
    }

    info!("Daemon uninstalled");
    Ok(())
}

/// Check if daemon is running (for status display)
#[allow(dead_code)]
pub fn is_daemon_running() -> bool {
    is_launchagent_loaded()
}
