# Persistent Daemon via LaunchAgent

**Date:** 2026-01-01
**Status:** Approved

## Problem

The daemon is currently bundled as a Tauri sidecar, which means it starts and stops with the GUI. This defeats the purpose of having a daemon architecture - sessions don't persist when the GUI closes.

## Solution

Run the daemon as a macOS LaunchAgent - a user-level background service that:
- Starts automatically on user login
- Stays running when GUI closes
- Restarts automatically if it crashes
- Sessions survive GUI restarts

## File Locations

```
~/Library/LaunchAgents/com.claudemaster.daemon.plist  # Service definition
~/Library/Application Support/com.claudemaster.claude-master/
  ├── bin/claude-master-daemon    # Daemon binary (extracted from app)
  ├── daemon.sock                 # Unix socket
  ├── sessions.json               # Persisted sessions
  └── groups.json                 # Persisted groups
~/Library/Logs/claude-master-daemon.log               # Daemon logs
```

## LaunchAgent Plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.claudemaster.daemon</string>

    <key>ProgramArguments</key>
    <array>
        <string>/Users/USER/Library/Application Support/com.claudemaster.claude-master/bin/claude-master-daemon</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>/Users/USER/Library/Logs/claude-master-daemon.log</string>

    <key>StandardErrorPath</key>
    <string>/Users/USER/Library/Logs/claude-master-daemon.log</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
</dict>
</plist>
```

## Startup Flow

```
App launches
    ↓
Check if LaunchAgent installed?
    ├─ No → Extract daemon binary, install plist, load service
    └─ Yes → Check if daemon binary is current version?
                ├─ No → Update binary, restart service
                └─ Yes → Just connect to existing daemon
    ↓
Connect to daemon socket (sessions are already running!)
```

## Key Functions

### ensure_daemon_running()
Called on app startup. Extracts/updates daemon binary, installs LaunchAgent if missing, ensures daemon is running.

### is_daemon_running()
Pings daemon socket to check if responsive.

### needs_update()
Compares SHA256 hashes of installed vs bundled binary.

### install_launch_agent()
Generates plist with correct paths, writes to LaunchAgents directory.

### load_launch_agent() / unload_launch_agent()
Wraps `launchctl load/unload` commands.

### restart_daemon()
Unloads, copies new binary, loads. GUI reconnects automatically.

### uninstall_daemon()
Menu option for clean removal - stops service, removes plist, removes app support directory.

## Changes Required

| File | Change |
|------|--------|
| `daemon_launcher.rs` | Complete rewrite for LaunchAgent management |
| `lib.rs` | Update startup to use async launcher |
| `commands.rs` | Add `uninstall_daemon` Tauri command |
| `state.rs` | Remove session status reset (sessions persist now) |
| `tauri.conf.json` | Keep sidecar config (used for binary extraction) |

## Edge Cases

**User deletes app without uninstalling:**
- LaunchAgent fails to start (binary missing)
- launchd logs errors but no harm
- User can manually delete plist if desired

**Binary update while sessions running:**
- Unload gracefully stops daemon
- PTY processes terminate
- New daemon loads, sessions show as Stopped
- User restarts sessions as needed

**First launch:**
- No LaunchAgent exists
- Extract binary, install plist, load service
- Connect and show empty session list
