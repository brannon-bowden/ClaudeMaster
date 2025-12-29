#!/bin/bash
# Version management script for Agent Deck
# Usage: ./scripts/version.sh [major|minor|patch|<version>]
#
# Examples:
#   ./scripts/version.sh patch        # 0.1.0 -> 0.1.1
#   ./scripts/version.sh minor        # 0.1.0 -> 0.2.0
#   ./scripts/version.sh major        # 0.1.0 -> 1.0.0
#   ./scripts/version.sh 1.2.3        # Set to specific version
#   ./scripts/version.sh              # Show current version

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Get current version from root Cargo.toml
get_current_version() {
    grep -m1 '^version' "$PROJECT_ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/'
}

# Parse version into components
parse_version() {
    local version=$1
    MAJOR=$(echo "$version" | cut -d. -f1)
    MINOR=$(echo "$version" | cut -d. -f2)
    PATCH=$(echo "$version" | cut -d. -f3 | cut -d- -f1)
    PRERELEASE=$(echo "$version" | grep -oE '\-.*$' || true)
}

# Bump version
bump_version() {
    local current=$1
    local bump_type=$2

    parse_version "$current"

    case "$bump_type" in
        major)
            MAJOR=$((MAJOR + 1))
            MINOR=0
            PATCH=0
            PRERELEASE=""
            ;;
        minor)
            MINOR=$((MINOR + 1))
            PATCH=0
            PRERELEASE=""
            ;;
        patch)
            PATCH=$((PATCH + 1))
            PRERELEASE=""
            ;;
        *)
            # Assume it's a specific version
            if [[ "$bump_type" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-.*)?$ ]]; then
                echo "$bump_type"
                return
            else
                log_error "Invalid version format: $bump_type"
                exit 1
            fi
            ;;
    esac

    echo "${MAJOR}.${MINOR}.${PATCH}${PRERELEASE}"
}

# Update version in a TOML file
update_toml_version() {
    local file=$1
    local version=$2
    local temp_file=$(mktemp)

    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -E "s/^version = \"[^\"]*\"/version = \"$version\"/" "$file" > "$temp_file"
    else
        sed -E "s/^version = \"[^\"]*\"/version = \"$version\"/" "$file" > "$temp_file"
    fi

    mv "$temp_file" "$file"
    log_info "Updated $file"
}

# Update version in package.json
update_package_json_version() {
    local file=$1
    local version=$2
    local temp_file=$(mktemp)

    if command -v jq &> /dev/null; then
        jq ".version = \"$version\"" "$file" > "$temp_file"
        mv "$temp_file" "$file"
    else
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -E "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" "$file" > "$temp_file"
        else
            sed -E "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" "$file" > "$temp_file"
        fi
        mv "$temp_file" "$file"
    fi
    log_info "Updated $file"
}

# Update version in tauri.conf.json
update_tauri_config_version() {
    local file=$1
    local version=$2
    update_package_json_version "$file" "$version"
}

# Main function
main() {
    local bump_type=$1
    local current_version=$(get_current_version)

    # If no argument, show current version
    if [ -z "$bump_type" ]; then
        echo -e "${BLUE}Current version:${NC} $current_version"
        return
    fi

    local new_version=$(bump_version "$current_version" "$bump_type")

    echo -e "${BLUE}Current version:${NC} $current_version"
    echo -e "${BLUE}New version:${NC} $new_version"
    echo ""

    # Confirm
    read -p "Update all version files? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_warn "Aborted"
        exit 0
    fi

    # Update all version files
    log_info "Updating version to $new_version..."

    # Root Cargo.toml (workspace)
    update_toml_version "$PROJECT_ROOT/Cargo.toml" "$new_version"

    # Daemon Cargo.toml
    update_toml_version "$PROJECT_ROOT/daemon/Cargo.toml" "$new_version"

    # Shared Cargo.toml
    update_toml_version "$PROJECT_ROOT/shared/Cargo.toml" "$new_version"

    # GUI Tauri Cargo.toml
    update_toml_version "$PROJECT_ROOT/gui/src-tauri/Cargo.toml" "$new_version"

    # GUI package.json
    update_package_json_version "$PROJECT_ROOT/gui/package.json" "$new_version"

    # Tauri config
    update_tauri_config_version "$PROJECT_ROOT/gui/src-tauri/tauri.conf.json" "$new_version"

    echo ""
    log_info "Version updated to $new_version"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "  1. Review changes: git diff"
    echo "  2. Commit: git commit -am \"chore: bump version to $new_version\""
    echo "  3. Tag: git tag v$new_version"
    echo "  4. Push: git push && git push --tags"
}

main "$@"
