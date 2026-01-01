#!/bin/bash
# Build script for Claude Master
# Usage: ./scripts/build.sh [daemon|gui|all] [debug|release]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default values
TARGET="${1:-all}"
BUILD_TYPE="${2:-release}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust: https://rustup.rs"
        exit 1
    fi

    if ! command -v node &> /dev/null; then
        log_error "Node.js not found. Please install Node.js: https://nodejs.org"
        exit 1
    fi

    if ! command -v npm &> /dev/null; then
        log_error "npm not found. Please install npm"
        exit 1
    fi

    log_info "All prerequisites satisfied"
}

# Build daemon
build_daemon() {
    log_info "Building daemon ($BUILD_TYPE)..."
    cd "$PROJECT_ROOT"

    if [ "$BUILD_TYPE" = "release" ]; then
        cargo build -p claude-master-daemon --release
    else
        cargo build -p claude-master-daemon
    fi

    log_info "Daemon build complete"
}

# Build GUI
build_gui() {
    log_info "Building GUI ($BUILD_TYPE)..."
    cd "$PROJECT_ROOT/gui"

    # Install npm dependencies if needed
    if [ ! -d "node_modules" ]; then
        log_info "Installing npm dependencies..."
        npm install
    fi

    if [ "$BUILD_TYPE" = "release" ]; then
        npm run tauri:build
    else
        npm run tauri:build:debug
    fi

    log_info "GUI build complete"
}

# Main build logic
main() {
    check_prerequisites

    case "$TARGET" in
        daemon)
            build_daemon
            ;;
        gui)
            build_gui
            ;;
        all)
            build_daemon
            build_gui
            ;;
        *)
            log_error "Unknown target: $TARGET"
            echo "Usage: $0 [daemon|gui|all] [debug|release]"
            exit 1
            ;;
    esac

    log_info "Build complete!"

    # Show output locations
    echo ""
    log_info "Build artifacts:"

    if [ "$TARGET" = "daemon" ] || [ "$TARGET" = "all" ]; then
        if [ "$BUILD_TYPE" = "release" ]; then
            echo "  Daemon: $PROJECT_ROOT/target/release/claude-master-daemon"
        else
            echo "  Daemon: $PROJECT_ROOT/target/debug/claude-master-daemon"
        fi
    fi

    if [ "$TARGET" = "gui" ] || [ "$TARGET" = "all" ]; then
        echo "  GUI: $PROJECT_ROOT/gui/src-tauri/target/release/bundle/"
    fi
}

main
