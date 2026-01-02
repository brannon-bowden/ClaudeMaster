# Auto-Update Support Design

## Overview

Add automatic update support to Claude Master using Tauri's updater plugin with GitHub Releases as the update source.

## User Experience

1. App checks for updates on launch (after 3 second delay)
2. If update available, modal dialog appears with version info and release notes
3. User can click "Update Now" or "Later"
4. "Update Now" downloads with progress bar, then restarts app
5. "Later" dismisses until next launch

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Claude Master App                     │
├─────────────────────────────────────────────────────────┤
│  Frontend (SolidJS)                                      │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │ UpdateModal.tsx │◄───│ update-checker.ts (listener)│ │
│  └─────────────────┘    └─────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  Backend (Rust/Tauri)                                    │
│  ┌─────────────────────────────────────────────────────┐│
│  │ tauri-plugin-updater                                 ││
│  │ - Checks GitHub Releases on app launch               ││
│  │ - Verifies signature with embedded public key        ││
│  │ - Downloads update in background                     ││
│  │ - Applies update on restart                          ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  GitHub Releases                                         │
│  └── vX.Y.Z/                                            │
│      ├── claude-master-linux-x64.tar.gz                 │
│      ├── claude-master-macos-arm64.tar.gz               │
│      ├── claude-master-macos-x64.tar.gz                 │
│      ├── claude-master-windows-x64.zip                  │
│      └── latest.json  ← Update manifest (auto-generated)│
└─────────────────────────────────────────────────────────┘
```

## Implementation

### Backend Dependencies

```toml
# gui/src-tauri/Cargo.toml
tauri-plugin-updater = "2"
```

### Tauri Configuration

```json
// tauri.conf.json - add to root level
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/brannon-bowden/ClaudeMaster/releases/latest/download/latest.json"
      ],
      "pubkey": "<GENERATED_PUBLIC_KEY>"
    }
  }
}
```

### Rust Plugin Registration

```rust
// main.rs
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Frontend Dependencies

```bash
npm install @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

### Update Checker Service

```typescript
// src/services/update-checker.ts
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  body: string;
  date: string;
}

export async function checkForUpdates(): Promise<UpdateInfo | null> {
  const update = await check();
  if (update) {
    return {
      version: update.version,
      currentVersion: update.currentVersion,
      body: update.body ?? '',
      date: update.date ?? '',
    };
  }
  return null;
}

export async function downloadAndInstall(
  onProgress: (percent: number) => void
): Promise<void> {
  const update = await check();
  if (!update) return;

  await update.downloadAndInstall((event) => {
    if (event.event === 'Progress') {
      const percent = (event.data.chunkLength / event.data.contentLength) * 100;
      onProgress(percent);
    }
  });

  await relaunch();
}
```

### Update Modal Component

```tsx
// src/components/UpdateModal.tsx
import { createSignal, Show } from 'solid-js';

interface Props {
  version: string;
  currentVersion: string;
  releaseNotes: string;
  onUpdate: () => void;
  onDismiss: () => void;
}

export function UpdateModal(props: Props) {
  const [downloading, setDownloading] = createSignal(false);
  const [progress, setProgress] = createSignal(0);

  return (
    <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-zinc-800 rounded-lg p-6 max-w-md w-full mx-4 shadow-xl">
        <h2 class="text-xl font-semibold text-white mb-2">
          Update Available
        </h2>
        <p class="text-zinc-400 mb-4">
          Version {props.version} is available (you have {props.currentVersion})
        </p>

        <Show when={props.releaseNotes}>
          <div class="bg-zinc-900 rounded p-3 mb-4 max-h-40 overflow-y-auto">
            <p class="text-sm text-zinc-300 whitespace-pre-wrap">
              {props.releaseNotes}
            </p>
          </div>
        </Show>

        <Show when={downloading()}>
          <div class="mb-4">
            <div class="h-2 bg-zinc-700 rounded-full overflow-hidden">
              <div
                class="h-full bg-blue-500 transition-all"
                style={{ width: `${progress()}%` }}
              />
            </div>
            <p class="text-sm text-zinc-400 mt-1">
              Downloading... {progress().toFixed(0)}%
            </p>
          </div>
        </Show>

        <div class="flex gap-3 justify-end">
          <button
            class="px-4 py-2 text-zinc-400 hover:text-white"
            onClick={props.onDismiss}
            disabled={downloading()}
          >
            Later
          </button>
          <button
            class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded"
            onClick={() => {
              setDownloading(true);
              props.onUpdate();
            }}
            disabled={downloading()}
          >
            Update Now
          </button>
        </div>
      </div>
    </div>
  );
}
```

### Capabilities Configuration

```json
// src-tauri/capabilities/default.json
{
  "permissions": [
    "updater:default",
    "updater:allow-check",
    "updater:allow-download-and-install",
    "process:allow-restart"
  ]
}
```

### App Integration

```typescript
// src/App.tsx - add to existing App component
import { onMount, createSignal } from 'solid-js';
import { UpdateModal } from './components/UpdateModal';
import { checkForUpdates, downloadAndInstall, UpdateInfo } from './services/update-checker';

// Inside App component:
const [updateInfo, setUpdateInfo] = createSignal<UpdateInfo | null>(null);

onMount(async () => {
  setTimeout(async () => {
    try {
      const update = await checkForUpdates();
      if (update) setUpdateInfo(update);
    } catch (err) {
      console.error('Update check failed:', err);
    }
  }, 3000);
});

// Render UpdateModal when updateInfo is set
```

## CI/CD Changes

### GitHub Secrets Required

- `TAURI_SIGNING_PRIVATE_KEY` - Private key from key generation
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` - Password for key (if set)

### Release Workflow Updates

```yaml
# .github/workflows/release.yml

env:
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}

# After all builds complete, generate latest.json with platform URLs and signatures
# Upload latest.json to the GitHub release
```

### Update Manifest Format (latest.json)

```json
{
  "version": "v0.2.0",
  "notes": "Release notes from GitHub",
  "pub_date": "2026-01-01T12:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "url": "https://github.com/brannon-bowden/ClaudeMaster/releases/download/v0.2.0/claude-master-linux-x64.tar.gz",
      "signature": "<base64_signature>"
    },
    "darwin-x86_64": {
      "url": "...",
      "signature": "..."
    },
    "darwin-aarch64": {
      "url": "...",
      "signature": "..."
    },
    "windows-x86_64": {
      "url": "...",
      "signature": "..."
    }
  }
}
```

## One-Time Setup

1. Generate signing keypair:
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/claude-master.key
   ```

2. Add private key to GitHub Secrets as `TAURI_SIGNING_PRIVATE_KEY`

3. Embed public key in `tauri.conf.json` under `plugins.updater.pubkey`

## Security

- Updates are signed with Ed25519 keys
- Public key embedded in app binary
- Signatures verified before applying updates
- HTTPS-only download URLs
- No arbitrary code execution - only signed Tauri bundles

## Testing

1. Build app with version X
2. Create release with version X+1
3. Launch app version X
4. Verify update modal appears
5. Click "Update Now"
6. Verify download progress
7. Verify app restarts with new version
