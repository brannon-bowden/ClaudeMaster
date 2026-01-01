#!/bin/bash
# Build and copy the daemon sidecar binary
# This script is called before Tauri build
#
# Usage: ./build-sidecar.sh [target-triple]
# If no target is specified, detects from host architecture

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Accept target from:
# 1. Command line argument
# 2. SIDECAR_TARGET environment variable (set by CI)
# 3. Auto-detect from host architecture
if [[ -n "$1" ]]; then
    TARGET="$1"
elif [[ -n "$SIDECAR_TARGET" ]]; then
    TARGET="$SIDECAR_TARGET"
elif [[ "$OSTYPE" == "darwin"* ]]; then
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

# Build daemon with target specification
cd "$PROJECT_ROOT"
cargo build -p claude-master-daemon --release --target "$TARGET"

# Create binaries directory
mkdir -p "$SCRIPT_DIR/binaries"

# Copy binary with target suffix (Tauri convention)
# The binary is in target/$TARGET/release/ when using --target
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    cp "$PROJECT_ROOT/target/$TARGET/release/claude-master-daemon.exe" "$SCRIPT_DIR/binaries/claude-master-daemon-$TARGET.exe"
else
    cp "$PROJECT_ROOT/target/$TARGET/release/claude-master-daemon" "$SCRIPT_DIR/binaries/claude-master-daemon-$TARGET"
fi

echo "Sidecar binary copied to: $SCRIPT_DIR/binaries/claude-master-daemon-$TARGET"
