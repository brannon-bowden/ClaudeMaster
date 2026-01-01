# Build and copy the daemon sidecar binary (Windows)
# This script is called before Tauri build
#
# Usage: .\build-sidecar.ps1 [target-triple]
# If no target is specified, uses SIDECAR_TARGET env var or defaults to x86_64-pc-windows-msvc

param(
    [string]$Target
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)

# Determine target: argument > env var > default
if ($Target) {
    $TargetTriple = $Target
} elseif ($env:SIDECAR_TARGET) {
    $TargetTriple = $env:SIDECAR_TARGET
} else {
    $TargetTriple = "x86_64-pc-windows-msvc"
}

Write-Host "Building daemon for target: $TargetTriple"

# Build daemon with target specification
Set-Location $ProjectRoot
cargo build -p claude-master-daemon --release --target $TargetTriple
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to build daemon"
    exit 1
}

# Create binaries directory
$BinariesDir = Join-Path $ScriptDir "binaries"
if (-not (Test-Path $BinariesDir)) {
    New-Item -ItemType Directory -Path $BinariesDir | Out-Null
}

# Copy binary with target suffix (Tauri convention)
# Note: Join-Path only accepts 2 arguments, so chain them
$SourceBinary = Join-Path (Join-Path (Join-Path (Join-Path $ProjectRoot "target") $TargetTriple) "release") "claude-master-daemon.exe"
$DestBinary = Join-Path $BinariesDir "claude-master-daemon-$TargetTriple.exe"

Copy-Item $SourceBinary $DestBinary -Force

Write-Host "Sidecar binary copied to: $DestBinary"
