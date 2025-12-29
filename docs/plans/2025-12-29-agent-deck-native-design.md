# Agent Deck Native - Design Document

**Date:** 2025-12-29
**Status:** Approved
**Goal:** Recreate agent-deck as a native cross-platform GUI application

---

## Overview

A native desktop application for managing multiple Claude Code sessions. Replaces the original Go/tmux-based TUI with a Rust daemon + Tauri GUI architecture.

### Key Decisions

| Aspect | Decision |
|--------|----------|
| Architecture | Background daemon + Tauri GUI |
| Language | Rust (daemon + Tauri backend), SolidJS (frontend) |
| Platforms | macOS, Windows, Linux (native, no WSL required) |
| Sessions | Built-in process manager with PTY, persists across GUI restarts |
| Terminal | Full xterm.js emulation |
| Organization | Nested groups/subgroups with drag-and-drop |
| Claude Features | Status detection, session forking |
| Excluded | MCP management |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         SYSTEM                                       │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │              agent-deck-daemon (Background)                  │    │
│  │                                                              │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │    │
│  │  │SessionManager│  │  PTYManager  │  │ClaudeIntegration │   │    │
│  │  │- spawn/kill  │  │- portable-pty│  │- status detect   │   │    │
│  │  │- track state │  │- I/O buffers │  │- fork sessions   │   │    │
│  │  │- persistence │  │- resize      │  │                  │   │    │
│  │  └──────────────┘  └──────────────┘  └──────────────────┘   │    │
│  │                           │                                  │    │
│  │                    ┌──────┴──────┐                          │    │
│  │                    │  IPC Server │                          │    │
│  │                    │ Unix Socket │ (or Named Pipe on Win)   │    │
│  │                    └──────┬──────┘                          │    │
│  └───────────────────────────┼──────────────────────────────────┘    │
│                              │                                       │
│                              │ JSON-RPC                              │
│                              │                                       │
│  ┌───────────────────────────┼──────────────────────────────────┐    │
│  │              agent-deck (Tauri GUI)                          │    │
│  │                           │                                  │    │
│  │  ┌──────────────┐  ┌──────┴──────┐  ┌──────────────────┐    │    │
│  │  │ Rust Bridge  │◄─┤ IPC Client  │  │  Web Frontend    │    │    │
│  │  │              │  └─────────────┘  │  - SolidJS       │    │    │
│  │  │ Tauri Cmds ◄─┼──────────────────►│  - xterm.js      │    │    │
│  │  │              │                   │  - Sidebar       │    │    │
│  │  └──────────────┘                   └──────────────────┘    │    │
│  └──────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### Cross-Platform IPC

| Platform | IPC Method | Location |
|----------|------------|----------|
| macOS | Unix Socket | `~/Library/Application Support/agent-deck/daemon.sock` |
| Linux | Unix Socket | `~/.local/share/agent-deck/daemon.sock` |
| Windows | Named Pipe | `\\.\pipe\agent-deck-daemon` |

---

## Data Model

### Session

```rust
Session {
    id: UUID,
    name: String,
    group_id: Option<UUID>,         // None = root level
    working_dir: PathBuf,

    // Process state
    status: Running | Waiting | Idle | Error | Stopped,
    pid: Option<u32>,
    pty_id: Option<PtyId>,

    // Claude-specific
    claude_session_id: Option<String>,  // For forking

    // Metadata
    created_at: DateTime,
    last_activity: DateTime,
    order: u32,                     // Sort order within group
}
```

### Group (Nested)

```rust
Group {
    id: UUID,
    name: String,
    parent_id: Option<UUID>,        // None = root level
    collapsed: bool,
    order: u32,                     // Sort order within parent
}
```

### State Persistence

```
~/.agent-deck/
├── daemon.sock                    # IPC socket (runtime)
├── config.toml                    # User preferences
├── state/
│   ├── sessions.json              # Session definitions
│   ├── sessions.json.bak          # Auto-backup
│   └── groups.json                # Group hierarchy
└── logs/
    └── daemon.log                 # Daemon logs
```

---

## IPC Protocol

### Message Format

```json
// Request
{ "id": 1, "method": "session.create", "params": { "name": "my-project", "dir": "/path" } }

// Response
{ "id": 1, "result": { "session_id": "abc-123", "status": "running" } }

// Event (daemon → GUI)
{ "event": "session.status_changed", "data": { "session_id": "abc-123", "status": "waiting" } }

// PTY Output
{ "event": "pty.output", "data": { "session_id": "abc-123", "output": "base64-encoded" } }
```

### Methods

| Method | Description |
|--------|-------------|
| `daemon.ping` | Health check |
| `daemon.shutdown` | Stop daemon gracefully |
| `session.list` | Get all sessions with status |
| `session.create` | Create new Claude Code session |
| `session.attach` | Subscribe to PTY output stream |
| `session.detach` | Unsubscribe from PTY output |
| `session.input` | Send keystrokes to PTY |
| `session.resize` | Resize PTY dimensions |
| `session.stop` | Kill session process |
| `session.restart` | Stop + start session |
| `session.fork` | Fork Claude session with context |
| `session.delete` | Remove session from state |
| `group.list` | Get group hierarchy |
| `group.create` | Create new group |
| `group.delete` | Delete group |
| `group.move` | Move session/group to new parent |

### Events

| Event | Description |
|-------|-------------|
| `session.status_changed` | Status updated |
| `session.created` | New session added |
| `session.deleted` | Session removed |
| `pty.output` | Terminal output chunk |
| `pty.exit` | Process exited |

---

## Claude Code Integration

### Status Detection

```rust
StatusDetector {
    waiting_patterns: [
        r"^> $",                    // Claude's input prompt
        r"╭─+╮\s*$",               // Response box closed
        r"\? \[Y/n\]",             // Yes/No prompt
    ],

    running_patterns: [
        r"⠋|⠙|⠹|⠸|⠼|⠴|⠦|⠧|⠇|⠏",  // Spinner
        r"Thinking\.\.\.",
        r"Reading .+\.\.\.",
        r"Writing .+\.\.\.",
    ],

    error_patterns: [
        r"Error:",
        r"APIError",
        r"Rate limit",
    ],
}
```

### Session Forking

1. Detect current `session_id` from Claude's output
2. Create new session in same working directory
3. Launch Claude with `claude --resume <session_id>`
4. New session inherits full conversation history

---

## UI Design

### Layout

```
┌──────────────────────────────────────────────────────────────────┐
│  Agent Deck                                    [−] [□] [×]       │
├────────────────────┬─────────────────────────────────────────────┤
│ ┌────────────────┐ │                                             │
│ │ 🔍 Search...   │ │  my-project                        ● Running│
│ └────────────────┘ │  ~/projects/my-project                      │
│                    │ ┌─────────────────────────────────────────┐ │
│ ▼ Work (3)         │ │                                         │ │
│   ▼ Client A       │ │  [xterm.js terminal]                    │ │
│     ● my-project   │ │                                         │ │
│     ◐ api-backend  │ │                                         │ │
│   ▶ Client B       │ │                                         │ │
│                    │ │                                         │ │
│ ▼ Personal (2)     │ │                                         │ │
│   ○ blog           │ │                                         │ │
│   ○ dotfiles       │ └─────────────────────────────────────────┘ │
│                    │ ┌─────────────────────────────────────────┐ │
│ ─────────────────  │ │ [Fork] [Restart] [Stop]                 │ │
│ + New Session      │ └─────────────────────────────────────────┘ │
│ + New Group        │                                             │
├────────────────────┴─────────────────────────────────────────────┤
│ 5 sessions │ 1 running │ 1 waiting │ 3 idle          [Settings] │
└──────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Responsibility |
|-----------|----------------|
| `Sidebar` | Session tree, groups, search, actions |
| `SessionItem` | Session row with status indicator |
| `GroupItem` | Collapsible group with nesting |
| `TerminalPanel` | xterm.js + session header + toolbar |
| `SettingsModal` | Preferences, theme |
| `StatusBar` | Session counts, settings button |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+N` | New session |
| `Ctrl+G` | New group |
| `Ctrl+F` | Focus search |
| `Ctrl+↑/↓` | Navigate sessions |
| `Ctrl+Enter` | Attach to selected |
| `Ctrl+Shift+F` | Fork current session |
| `Ctrl+W` | Stop current session |

---

## Error Handling

### Daemon Lifecycle

| Scenario | Behavior |
|----------|----------|
| GUI starts, no daemon | GUI spawns daemon, waits for socket |
| GUI starts, daemon exists | GUI connects to existing socket |
| GUI closes | Daemon keeps running |
| Daemon crashes | GUI shows reconnect prompt |
| System reboot | State file preserves definitions, sessions marked Stopped |

### Session Recovery

After reboot, sessions are preserved in `sessions.json` but processes are gone:
- All sessions marked `Stopped`
- User can restart individually or "Restart All"
- Optional: `auto_start: true` flag for automatic restart

### PTY Edge Cases

| Edge Case | Handling |
|-----------|----------|
| Process exits | Update status to Stopped, emit event |
| GUI disconnects | Buffer last 10KB per session |
| Long output | Ring buffer, discard oldest |
| Resize | Send SIGWINCH, update PTY dimensions |

---

## Project Structure

```
agent-deck/
├── Cargo.toml                 # Workspace root
├── daemon/                    # agent-deck-daemon
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── ipc/
│       ├── session/
│       ├── pty/
│       ├── claude/
│       └── config/
│
├── gui/                       # Tauri app
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   └── bridge.rs
│   ├── tauri.conf.json
│   └── ui/
│       ├── package.json
│       ├── src/
│       │   ├── App.tsx
│       │   ├── components/
│       │   ├── stores/
│       │   └── lib/
│       └── index.html
│
├── shared/                    # Shared types
│   ├── Cargo.toml
│   └── src/lib.rs
│
└── scripts/
    ├── build.sh
    └── release.sh
```

## Dependencies

### Rust (Daemon + GUI)

| Crate | Purpose |
|-------|---------|
| `portable-pty` | Cross-platform PTY |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `toml` | Config parsing |
| `interprocess` | Cross-platform IPC |
| `tauri` | GUI framework |

### Frontend (npm)

| Package | Purpose |
|---------|---------|
| `solid-js` | Reactive UI |
| `xterm` | Terminal emulator |
| `xterm-addon-fit` | Auto-resize |
| `xterm-addon-webgl` | GPU rendering |
| `tailwindcss` | Styling |

## Build Outputs

| Platform | Daemon | GUI |
|----------|--------|-----|
| macOS | `agent-deck-daemon` | `Agent Deck.app` |
| Linux | `agent-deck-daemon` | `agent-deck.AppImage` |
| Windows | `agent-deck-daemon.exe` | `Agent Deck.msi` |
