#!/bin/bash
# Build release packages for Claude Master
# Usage: ./scripts/build-release.sh [platform]
#   platform: macos, windows, linux, or all (default: current platform)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

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

# Detect current platform
detect_platform() {
    case "$(uname -s)" in
        Darwin) echo "macos" ;;
        Linux) echo "linux" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) echo "unknown" ;;
    esac
}

# Build for macOS
build_macos() {
    log_info "Building for macOS..."

    cd "$PROJECT_ROOT/gui"

    # Build sidecar for current architecture
    npm run sidecar:build

    # Build Tauri app
    npm run tauri:build -- --bundles app,dmg

    log_info "macOS build complete!"
    log_info "Output: target/release/bundle/macos/"
}

# Build for Windows (requires Windows or cross-compilation setup)
build_windows() {
    log_info "Building for Windows..."

    local current_platform=$(detect_platform)

    if [ "$current_platform" != "windows" ]; then
        log_warn "Cross-compilation for Windows requires additional setup"
        log_warn "Consider using GitHub Actions for Windows builds"
        return 1
    fi

    cd "$PROJECT_ROOT/gui"

    # Build sidecar
    npm run sidecar:build

    # Build Tauri app with MSI and NSIS installers
    npm run tauri:build -- --bundles msi,nsis

    log_info "Windows build complete!"
    log_info "Output: target/release/bundle/msi/ and target/release/bundle/nsis/"
}

# Build for Linux (requires Linux or cross-compilation setup)
build_linux() {
    log_info "Building for Linux..."

    local current_platform=$(detect_platform)

    if [ "$current_platform" != "linux" ]; then
        log_warn "Cross-compilation for Linux requires additional setup"
        log_warn "Consider using GitHub Actions for Linux builds"
        return 1
    fi

    cd "$PROJECT_ROOT/gui"

    # Build sidecar
    npm run sidecar:build

    # Build Tauri app with AppImage and deb
    npm run tauri:build -- --bundles appimage,deb

    log_info "Linux build complete!"
    log_info "Output: target/release/bundle/appimage/ and target/release/bundle/deb/"
}

# Main
main() {
    local platform="${1:-$(detect_platform)}"

    log_info "Claude Master Release Build"
    log_info "Platform: $platform"
    echo

    # Ensure we're in the right directory
    cd "$PROJECT_ROOT"

    # Install dependencies if needed
    if [ ! -d "gui/node_modules" ]; then
        log_info "Installing npm dependencies..."
        cd gui && npm install && cd ..
    fi

    case "$platform" in
        macos)
            build_macos
            ;;
        windows)
            build_windows
            ;;
        linux)
            build_linux
            ;;
        all)
            log_info "Building for all platforms..."
            build_macos || log_warn "macOS build skipped"
            build_windows || log_warn "Windows build skipped"
            build_linux || log_warn "Linux build skipped"
            ;;
        *)
            log_error "Unknown platform: $platform"
            echo "Usage: $0 [macos|windows|linux|all]"
            exit 1
            ;;
    esac

    echo
    log_info "Build complete! Check target/release/bundle/ for outputs"
}

main "$@"
