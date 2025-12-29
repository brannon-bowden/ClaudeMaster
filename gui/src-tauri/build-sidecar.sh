#!/bin/bash
# Build and copy the daemon sidecar binary
# This script is called before Tauri build

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Determine target triple
if [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ $(uname -m) == "arm64" ]]; then
        TARGET="aarch64-apple-darwin"
    else
        TARGET="x86_64-apple-darwin"
    fi
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    TARGET="x86_64-unknown-linux-gnu"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    TARGET="x86_64-pc-windows-msvc"
else
    echo "Unknown OS: $OSTYPE"
    exit 1
fi

echo "Building daemon for target: $TARGET"

# Build daemon
cd "$PROJECT_ROOT"
cargo build -p agent-deck-daemon --release

# Create binaries directory
mkdir -p "$SCRIPT_DIR/binaries"

# Copy binary with target suffix (Tauri convention)
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    cp "$PROJECT_ROOT/target/release/agent-deck-daemon.exe" "$SCRIPT_DIR/binaries/agent-deck-daemon-$TARGET.exe"
else
    cp "$PROJECT_ROOT/target/release/agent-deck-daemon" "$SCRIPT_DIR/binaries/agent-deck-daemon-$TARGET"
fi

echo "Sidecar binary copied to: $SCRIPT_DIR/binaries/agent-deck-daemon-$TARGET"
