# Agent Deck Native Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a native cross-platform desktop app for managing multiple Claude Code sessions with a Rust daemon and Tauri GUI.

**Architecture:** Background daemon manages sessions and PTYs, communicates via Unix sockets (Named Pipes on Windows). Tauri app provides GUI with SolidJS frontend and xterm.js terminals.

**Tech Stack:** Rust (daemon + Tauri), portable-pty, tokio, interprocess, SolidJS, xterm.js, Tailwind CSS

---

## Phase 1: Project Scaffolding

### Task 1: Initialize Rust Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `daemon/Cargo.toml`
- Create: `daemon/src/main.rs`
- Create: `shared/Cargo.toml`
- Create: `shared/src/lib.rs`

**Step 1: Create workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["daemon", "shared", "gui/src-tauri"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/yourusername/agent-deck"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Step 2: Create daemon Cargo.toml**

```toml
[package]
name = "agent-deck-daemon"
version.workspace = true
edition.workspace = true

[[bin]]
name = "agent-deck-daemon"
path = "src/main.rs"

[dependencies]
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
toml.workspace = true
uuid.workspace = true
chrono.workspace = true
thiserror.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

shared = { path = "../shared" }
portable-pty = "0.8"
interprocess = { version = "2", features = ["tokio"] }
directories = "5"
```

**Step 3: Create shared Cargo.toml**

```toml
[package]
name = "shared"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
uuid.workspace = true
chrono.workspace = true
thiserror.workspace = true
```

**Step 4: Create daemon/src/main.rs**

```rust
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Agent Deck daemon starting...");
    Ok(())
}
```

**Step 5: Create shared/src/lib.rs**

```rust
//! Shared types between daemon and GUI

pub mod protocol;
pub mod session;
pub mod group;
```

**Step 6: Add anyhow to daemon**

Add to daemon/Cargo.toml dependencies:
```toml
anyhow = "1"
```

**Step 7: Build to verify setup**

Run: `cargo build`
Expected: Compiles successfully

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: initialize Rust workspace with daemon and shared crates"
```

---

### Task 2: Define Core Types in Shared Crate

**Files:**
- Create: `shared/src/session.rs`
- Create: `shared/src/group.rs`
- Create: `shared/src/protocol.rs`
- Modify: `shared/src/lib.rs`

**Step 1: Create session types**

Create `shared/src/session.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Waiting,
    Idle,
    Error,
    Stopped,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    pub group_id: Option<Uuid>,
    pub working_dir: PathBuf,

    #[serde(default)]
    pub status: SessionStatus,
    #[serde(skip)]
    pub pid: Option<u32>,

    pub claude_session_id: Option<String>,

    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    #[serde(default)]
    pub order: u32,
}

impl Session {
    pub fn new(name: String, working_dir: PathBuf, group_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            group_id,
            working_dir,
            status: SessionStatus::Stopped,
            pid: None,
            claude_session_id: None,
            created_at: now,
            last_activity: now,
            order: 0,
        }
    }
}
```

**Step 2: Create group types**

Create `shared/src/group.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub order: u32,
}

impl Group {
    pub fn new(name: String, parent_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id,
            collapsed: false,
            order: 0,
        }
    }
}
```

**Step 3: Create protocol types**

Create `shared/src/protocol.rs`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::group::Group;
use crate::session::{Session, SessionStatus};

/// Request from GUI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Response from daemon to GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: i32,
    pub message: String,
}

/// Event from daemon to GUI (no id, push-based)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub event: String,
    pub data: Value,
}

// --- Method Parameters ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionParams {
    pub name: String,
    pub dir: String,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIdParams {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInputParams {
    pub session_id: Uuid,
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResizeParams {
    pub session_id: Uuid,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkSessionParams {
    pub session_id: Uuid,
    pub new_name: Option<String>,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupParams {
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveToGroupParams {
    pub session_id: Uuid,
    pub group_id: Option<Uuid>,
}

// --- Event Data ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChangedData {
    pub session_id: Uuid,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyOutputData {
    pub session_id: Uuid,
    pub output: String, // base64 encoded
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyExitData {
    pub session_id: Uuid,
    pub exit_code: Option<i32>,
}

// --- Results ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResult {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupListResult {
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreatedResult {
    pub session: Session,
}
```

**Step 4: Update lib.rs**

```rust
//! Shared types between daemon and GUI

pub mod group;
pub mod protocol;
pub mod session;

pub use group::Group;
pub use protocol::*;
pub use session::{Session, SessionStatus};
```

**Step 5: Build to verify**

Run: `cargo build`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add core types (Session, Group, Protocol) to shared crate"
```

---

### Task 3: Set Up Tauri Project

**Files:**
- Create: `gui/package.json`
- Create: `gui/src-tauri/Cargo.toml`
- Create: `gui/src-tauri/tauri.conf.json`
- Create: `gui/src-tauri/src/main.rs`
- Create: `gui/src-tauri/src/lib.rs`
- Create: `gui/index.html`
- Create: `gui/src/main.tsx`
- Create: `gui/src/App.tsx`
- Create: `gui/vite.config.ts`
- Create: `gui/tsconfig.json`
- Create: `gui/tailwind.config.js`
- Create: `gui/postcss.config.js`
- Create: `gui/src/index.css`

**Step 1: Create gui/package.json**

```json
{
  "name": "agent-deck",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "solid-js": "^1.8",
    "xterm": "^5.3",
    "xterm-addon-fit": "^0.8",
    "xterm-addon-webgl": "^0.16"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "autoprefixer": "^10",
    "postcss": "^8",
    "tailwindcss": "^3",
    "typescript": "^5",
    "vite": "^5",
    "vite-plugin-solid": "^2"
  }
}
```

**Step 2: Create gui/src-tauri/Cargo.toml**

```toml
[package]
name = "agent-deck-gui"
version.workspace = true
edition.workspace = true

[lib]
name = "agent_deck_gui_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
shared = { path = "../../shared" }
interprocess = { version = "2", features = ["tokio"] }
```

**Step 3: Create gui/src-tauri/tauri.conf.json**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Agent Deck",
  "version": "0.1.0",
  "identifier": "com.agentdeck.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Agent Deck",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

**Step 4: Create gui/src-tauri/src/main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    agent_deck_gui_lib::run()
}
```

**Step 5: Create gui/src-tauri/src/lib.rs**

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 6: Create gui/src-tauri/build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

**Step 7: Create gui/index.html**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Agent Deck</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 8: Create gui/src/main.tsx**

```tsx
import { render } from "solid-js/web";
import App from "./App";
import "./index.css";

render(() => <App />, document.getElementById("root")!);
```

**Step 9: Create gui/src/App.tsx**

```tsx
function App() {
  return (
    <div class="h-screen bg-gray-900 text-white flex items-center justify-center">
      <h1 class="text-2xl">Agent Deck</h1>
    </div>
  );
}

export default App;
```

**Step 10: Create gui/src/index.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen,
    Ubuntu, Cantarell, "Fira Sans", "Droid Sans", "Helvetica Neue", sans-serif;
}
```

**Step 11: Create gui/vite.config.ts**

```typescript
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "esnext",
  },
});
```

**Step 12: Create gui/tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "preserve",
    "jsxImportSource": "solid-js",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
```

**Step 13: Create gui/tailwind.config.js**

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {},
  },
  plugins: [],
};
```

**Step 14: Create gui/postcss.config.js**

```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

**Step 15: Create placeholder icon**

```bash
mkdir -p gui/src-tauri/icons
# Create minimal placeholder (will be replaced later)
```

**Step 16: Install npm dependencies**

Run: `cd gui && npm install`
Expected: Dependencies installed successfully

**Step 17: Build to verify**

Run: `cargo build`
Expected: Compiles (may have warnings about missing icons)

**Step 18: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri app with SolidJS and Tailwind"
```

---

## Phase 2: Daemon Core

### Task 4: Configuration and State Management

**Files:**
- Create: `daemon/src/config.rs`
- Create: `daemon/src/state.rs`
- Modify: `daemon/src/main.rs`

**Step 1: Create config module**

Create `daemon/src/config.rs`:

```rust
use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub socket_timeout_ms: u64,
    pub output_buffer_kb: usize,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub font_family: String,
    pub font_size: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_timeout_ms: 5000,
            output_buffer_kb: 10,
            log_level: "info".to_string(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_family: "monospace".to_string(),
            font_size: 14,
        }
    }
}

pub fn get_data_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "agentdeck", "agent-deck")
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let data_dir = proj_dirs.data_dir().to_path_buf();
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

pub fn get_config_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("config.toml"))
}

pub fn get_state_dir() -> Result<PathBuf> {
    let state_dir = get_data_dir()?.join("state");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir)
}

pub fn get_socket_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("daemon.sock"))
}

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}
```

**Step 2: Create state module**

Create `daemon/src/state.rs`:

```rust
use anyhow::Result;
use shared::{Group, Session};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::get_state_dir;

#[derive(Debug, Default)]
pub struct AppState {
    pub sessions: HashMap<Uuid, Session>,
    pub groups: HashMap<Uuid, Group>,
}

pub type SharedState = Arc<RwLock<AppState>>;

pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

fn sessions_path() -> Result<PathBuf> {
    Ok(get_state_dir()?.join("sessions.json"))
}

fn groups_path() -> Result<PathBuf> {
    Ok(get_state_dir()?.join("groups.json"))
}

pub async fn load_state(state: &SharedState) -> Result<()> {
    let mut s = state.write().await;

    // Load sessions
    let sessions_file = sessions_path()?;
    if sessions_file.exists() {
        let content = fs::read_to_string(&sessions_file)?;
        let sessions: Vec<Session> = serde_json::from_str(&content)?;
        for session in sessions {
            s.sessions.insert(session.id, session);
        }
    }

    // Load groups
    let groups_file = groups_path()?;
    if groups_file.exists() {
        let content = fs::read_to_string(&groups_file)?;
        let groups: Vec<Group> = serde_json::from_str(&content)?;
        for group in groups {
            s.groups.insert(group.id, group);
        }
    }

    Ok(())
}

pub async fn save_state(state: &SharedState) -> Result<()> {
    let s = state.read().await;

    // Save sessions
    let sessions: Vec<&Session> = s.sessions.values().collect();
    let sessions_json = serde_json::to_string_pretty(&sessions)?;
    let sessions_file = sessions_path()?;

    // Backup before writing
    if sessions_file.exists() {
        let backup = sessions_file.with_extension("json.bak");
        fs::copy(&sessions_file, backup)?;
    }
    fs::write(&sessions_file, sessions_json)?;

    // Save groups
    let groups: Vec<&Group> = s.groups.values().collect();
    let groups_json = serde_json::to_string_pretty(&groups)?;
    let groups_file = groups_path()?;
    fs::write(&groups_file, groups_json)?;

    Ok(())
}
```

**Step 3: Update main.rs**

```rust
mod config;
mod state;

use anyhow::Result;
use tracing::info;

use crate::config::load_config;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Agent Deck daemon starting...");

    let config = load_config()?;
    info!("Config loaded: {:?}", config);

    let state = new_shared_state();
    load_state(&state).await?;

    let s = state.read().await;
    info!(
        "State loaded: {} sessions, {} groups",
        s.sessions.len(),
        s.groups.len()
    );

    Ok(())
}
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Compiles successfully

**Step 5: Run daemon to test**

Run: `cargo run -p agent-deck-daemon`
Expected: Logs showing config loaded, state loaded

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add config and state management to daemon"
```

---

### Task 5: IPC Server Foundation

**Files:**
- Create: `daemon/src/ipc.rs`
- Modify: `daemon/src/main.rs`

**Step 1: Create IPC module**

Create `daemon/src/ipc.rs`:

```rust
use anyhow::Result;
use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericFilePath, ListenerOptions,
};
use shared::{ErrorInfo, Event, Request, Response};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::state::SharedState;

pub type EventSender = broadcast::Sender<Event>;

pub async fn start_server(
    socket_path: &Path,
    state: SharedState,
    event_tx: EventSender,
) -> Result<()> {
    // Remove existing socket if present
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let name = socket_path.to_fs_name::<GenericFilePath>()?;
    let listener = ListenerOptions::new().name(name).create_tokio()?;

    info!("IPC server listening on {:?}", socket_path);

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let state = state.clone();
                let event_tx = event_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state, event_tx).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: Stream,
    state: SharedState,
    event_tx: EventSender,
) -> Result<()> {
    info!("New client connected");

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut event_rx = event_tx.subscribe();

    let mut line = String::new();

    loop {
        tokio::select! {
            // Handle incoming requests
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        info!("Client disconnected");
                        break;
                    }
                    Ok(_) => {
                        let response = process_request(&line, &state).await;
                        let response_json = serde_json::to_string(&response)? + "\n";
                        writer.write_all(response_json.as_bytes()).await?;
                        line.clear();
                    }
                    Err(e) => {
                        error!("Read error: {}", e);
                        break;
                    }
                }
            }

            // Forward events to client
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        let event_json = serde_json::to_string(&event)? + "\n";
                        if let Err(e) = writer.write_all(event_json.as_bytes()).await {
                            warn!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client lagged, missed {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_request(line: &str, state: &SharedState) -> Response {
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            return Response {
                id: 0,
                result: None,
                error: Some(ErrorInfo {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
        }
    };

    match request.method.as_str() {
        "daemon.ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        },

        "session.list" => {
            let s = state.read().await;
            let sessions: Vec<_> = s.sessions.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"sessions": sessions})),
                error: None,
            }
        }

        "group.list" => {
            let s = state.read().await;
            let groups: Vec<_> = s.groups.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"groups": groups})),
                error: None,
            }
        }

        _ => Response {
            id: request.id,
            result: None,
            error: Some(ErrorInfo {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    }
}
```

**Step 2: Update main.rs**

```rust
mod config;
mod ipc;
mod state;

use anyhow::Result;
use shared::Event;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::{get_socket_path, load_config};
use crate::ipc::start_server;
use crate::state::{load_state, new_shared_state};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Agent Deck daemon starting...");

    let config = load_config()?;
    info!("Config loaded");

    let state = new_shared_state();
    load_state(&state).await?;

    {
        let s = state.read().await;
        info!(
            "State loaded: {} sessions, {} groups",
            s.sessions.len(),
            s.groups.len()
        );
    }

    let (event_tx, _) = broadcast::channel::<Event>(100);
    let socket_path = get_socket_path()?;

    start_server(&socket_path, state, event_tx).await?;

    Ok(())
}
```

**Step 3: Build to verify**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Test daemon manually**

Run daemon in one terminal:
```bash
cargo run -p agent-deck-daemon
```

Test with netcat in another (macOS/Linux):
```bash
echo '{"id":1,"method":"daemon.ping","params":{}}' | nc -U ~/.local/share/agent-deck/daemon.sock
```

Expected: `{"id":1,"result":{"status":"ok"}}`

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add IPC server with basic ping and list methods"
```

---

## Phase 3: PTY and Session Management

### Task 6: PTY Manager

**Files:**
- Create: `daemon/src/pty.rs`
- Modify: `daemon/Cargo.toml`

**Step 1: Create PTY module**

Create `daemon/src/pty.rs`:

```rust
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
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.cwd(working_dir);

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
```

**Step 2: Build to verify**

Run: `cargo build`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: add PTY manager for cross-platform terminal handling"
```

---

### Task 7: Session Manager with Full IPC Integration

**Files:**
- Create: `daemon/src/session_manager.rs`
- Modify: `daemon/src/ipc.rs`
- Modify: `daemon/src/main.rs`

**Step 1: Create session manager**

Create `daemon/src/session_manager.rs`:

```rust
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use shared::{Event, PtyOutputData, Session, SessionStatus, StatusChangedData};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use uuid::Uuid;

use crate::pty::PtyManager;
use crate::state::{save_state, SharedState};

pub struct SessionManager {
    state: SharedState,
    pty_manager: Arc<PtyManager>,
    event_tx: broadcast::Sender<Event>,
    output_rx: mpsc::Receiver<(Uuid, Vec<u8>)>,
    output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
}

impl SessionManager {
    pub fn new(state: SharedState, event_tx: broadcast::Sender<Event>) -> Self {
        let (output_tx, output_rx) = mpsc::channel(1000);
        Self {
            state,
            pty_manager: Arc::new(PtyManager::new()),
            event_tx,
            output_rx,
            output_tx,
        }
    }

    pub async fn run(mut self) {
        info!("Session manager started");

        while let Some((session_id, data)) = self.output_rx.recv().await {
            let output = BASE64.encode(&data);
            let event = Event {
                event: "pty.output".to_string(),
                data: serde_json::to_value(PtyOutputData {
                    session_id,
                    output,
                })
                .unwrap(),
            };
            let _ = self.event_tx.send(event);
        }
    }

    pub fn pty_manager(&self) -> Arc<PtyManager> {
        self.pty_manager.clone()
    }

    pub fn output_tx(&self) -> mpsc::Sender<(Uuid, Vec<u8>)> {
        self.output_tx.clone()
    }

    pub async fn create_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        output_tx: mpsc::Sender<(Uuid, Vec<u8>)>,
        event_tx: &broadcast::Sender<Event>,
        name: String,
        working_dir: PathBuf,
        group_id: Option<Uuid>,
    ) -> Result<Session> {
        let mut session = Session::new(name, working_dir.clone(), group_id);

        // Spawn PTY
        pty_manager
            .spawn(session.id, &working_dir, 24, 80, output_tx)
            .await?;

        session.status = SessionStatus::Running;
        session.last_activity = Utc::now();

        // Save to state
        {
            let mut s = state.write().await;
            s.sessions.insert(session.id, session.clone());
        }
        save_state(state).await?;

        // Emit event
        let event = Event {
            event: "session.created".to_string(),
            data: serde_json::to_value(&session)?,
        };
        let _ = event_tx.send(event);

        Ok(session)
    }

    pub async fn stop_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        event_tx: &broadcast::Sender<Event>,
        session_id: Uuid,
    ) -> Result<()> {
        pty_manager.kill(session_id).await?;

        {
            let mut s = state.write().await;
            if let Some(session) = s.sessions.get_mut(&session_id) {
                session.status = SessionStatus::Stopped;
                session.pid = None;
            }
        }
        save_state(state).await?;

        let event = Event {
            event: "session.status_changed".to_string(),
            data: serde_json::to_value(StatusChangedData {
                session_id,
                status: SessionStatus::Stopped,
            })?,
        };
        let _ = event_tx.send(event);

        Ok(())
    }

    pub async fn delete_session(
        state: &SharedState,
        pty_manager: &PtyManager,
        event_tx: &broadcast::Sender<Event>,
        session_id: Uuid,
    ) -> Result<()> {
        // Stop first if running
        if pty_manager.is_alive(session_id).await {
            pty_manager.kill(session_id).await?;
        }

        {
            let mut s = state.write().await;
            s.sessions.remove(&session_id);
        }
        save_state(state).await?;

        let event = Event {
            event: "session.deleted".to_string(),
            data: serde_json::json!({"session_id": session_id}),
        };
        let _ = event_tx.send(event);

        Ok(())
    }
}
```

**Step 2: Update ipc.rs with session methods**

Add to `daemon/src/ipc.rs`, update the `process_request` function:

```rust
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use shared::{CreateGroupParams, CreateSessionParams, SessionIdParams, SessionInputParams, SessionResizeParams};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::pty::PtyManager;
use crate::session_manager::SessionManager;

// Update process_request signature and add handlers:

async fn process_request(
    line: &str,
    state: &SharedState,
    pty_manager: &Arc<PtyManager>,
    output_tx: &mpsc::Sender<(Uuid, Vec<u8>)>,
    event_tx: &EventSender,
) -> Response {
    let request: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            return Response {
                id: 0,
                result: None,
                error: Some(ErrorInfo {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
        }
    };

    match request.method.as_str() {
        "daemon.ping" => Response {
            id: request.id,
            result: Some(serde_json::json!({"status": "ok"})),
            error: None,
        },

        "session.list" => {
            let s = state.read().await;
            let sessions: Vec<_> = s.sessions.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"sessions": sessions})),
                error: None,
            }
        }

        "session.create" => {
            let params: CreateSessionParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::create_session(
                state,
                pty_manager,
                output_tx.clone(),
                event_tx,
                params.name,
                PathBuf::from(params.dir),
                params.group_id,
            )
            .await
            {
                Ok(session) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"session": session})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to create session: {}", e),
                    }),
                },
            }
        }

        "session.stop" => {
            let params: SessionIdParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::stop_session(state, pty_manager, event_tx, params.session_id).await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to stop session: {}", e),
                    }),
                },
            }
        }

        "session.delete" => {
            let params: SessionIdParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match SessionManager::delete_session(state, pty_manager, event_tx, params.session_id)
                .await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to delete session: {}", e),
                    }),
                },
            }
        }

        "session.input" => {
            let params: SessionInputParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            let data = BASE64.decode(&params.input).unwrap_or_else(|_| params.input.into_bytes());

            match pty_manager.write(params.session_id, &data).await {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to write to session: {}", e),
                    }),
                },
            }
        }

        "session.resize" => {
            let params: SessionResizeParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            match pty_manager
                .resize(params.session_id, params.rows, params.cols)
                .await
            {
                Ok(()) => Response {
                    id: request.id,
                    result: Some(serde_json::json!({"success": true})),
                    error: None,
                },
                Err(e) => Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to resize session: {}", e),
                    }),
                },
            }
        }

        "group.list" => {
            let s = state.read().await;
            let groups: Vec<_> = s.groups.values().cloned().collect();
            Response {
                id: request.id,
                result: Some(serde_json::json!({"groups": groups})),
                error: None,
            }
        }

        "group.create" => {
            let params: CreateGroupParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response {
                        id: request.id,
                        result: None,
                        error: Some(ErrorInfo {
                            code: -32602,
                            message: format!("Invalid params: {}", e),
                        }),
                    };
                }
            };

            let group = shared::Group::new(params.name, params.parent_id);
            {
                let mut s = state.write().await;
                s.groups.insert(group.id, group.clone());
            }
            if let Err(e) = crate::state::save_state(state).await {
                return Response {
                    id: request.id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -32000,
                        message: format!("Failed to save state: {}", e),
                    }),
                };
            }

            Response {
                id: request.id,
                result: Some(serde_json::json!({"group": group})),
                error: None,
            }
        }

        _ => Response {
            id: request.id,
            result: None,
            error: Some(ErrorInfo {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    }
}
```

**Note:** The IPC module needs significant refactoring to pass the pty_manager and output_tx. This is a simplified reference - the actual implementation will require updating the `handle_connection` and `start_server` functions to accept these additional parameters.

**Step 3: Add base64 dependency**

Add to `daemon/Cargo.toml`:
```toml
base64 = "0.22"
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Compiles (may need adjustments)

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add session manager with PTY integration"
```

---

## Remaining Tasks (Summary)

The following tasks continue the implementation:

### Phase 4: Claude Integration
- **Task 8:** Status detection via regex patterns on PTY output
- **Task 9:** Session forking (detect session ID, launch with --resume)

### Phase 5: GUI Frontend
- **Task 10:** Tauri bridge (IPC client connecting to daemon)
- **Task 11:** Sidebar component with nested groups
- **Task 12:** Terminal panel with xterm.js
- **Task 13:** Session actions (create, stop, delete, fork)
- **Task 14:** Settings modal
- **Task 15:** Keyboard shortcuts

### Phase 6: Polish
- **Task 16:** Error handling and reconnection
- **Task 17:** Cross-platform testing
- **Task 18:** Build and packaging scripts

---

## Testing Strategy

Each task includes verification steps:
1. Unit tests for core logic (state, protocol parsing)
2. Integration tests for IPC communication
3. Manual testing for PTY and UI

Run tests with:
```bash
cargo test
cd gui && npm test
```

---

## Notes

- Use `cargo clippy` for linting
- Run `cargo fmt` before each commit
- Frontend uses SolidJS - remember reactive primitives
- xterm.js requires WebGL addon for performance with many terminals
