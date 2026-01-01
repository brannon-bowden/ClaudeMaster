#!/bin/bash
# Development script for Claude Master
# Starts both daemon and GUI in development mode

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

cleanup() {
    log_info "Shutting down..."
    # Kill all background jobs
    jobs -p | xargs -r kill 2>/dev/null || true
    exit 0
}

trap cleanup SIGINT SIGTERM

# Start daemon in background
start_daemon() {
    log_info "Starting daemon..."
    cd "$PROJECT_ROOT"
    RUST_LOG=info cargo run -p claude-master-daemon &
    DAEMON_PID=$!
    log_info "Daemon started (PID: $DAEMON_PID)"

    # Give daemon time to start
    sleep 2
}

# Start GUI in development mode
start_gui() {
    log_info "Starting GUI..."
    cd "$PROJECT_ROOT/gui"

    # Install npm dependencies if needed
    if [ ! -d "node_modules" ]; then
        log_info "Installing npm dependencies..."
        npm install
    fi

    npm run tauri:dev
}

main() {
    log_info "Starting Claude Master in development mode..."
    start_daemon
    start_gui
}

main
