@echo off
REM Build script for Agent Deck (Windows)
REM Usage: build.bat [daemon|gui|all] [debug|release]

setlocal enabledelayedexpansion

set TARGET=%1
set BUILD_TYPE=%2

if "%TARGET%"=="" set TARGET=all
if "%BUILD_TYPE%"=="" set BUILD_TYPE=release

REM Get script directory
set SCRIPT_DIR=%~dp0
set PROJECT_ROOT=%SCRIPT_DIR%..

echo [INFO] Building Agent Deck (%TARGET%, %BUILD_TYPE%)

REM Check prerequisites
where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Rust/Cargo not found. Please install Rust: https://rustup.rs
    exit /b 1
)

where node >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Node.js not found. Please install Node.js: https://nodejs.org
    exit /b 1
)

REM Build daemon
if "%TARGET%"=="daemon" goto build_daemon
if "%TARGET%"=="all" goto build_daemon
goto skip_daemon

:build_daemon
echo [INFO] Building daemon...
cd %PROJECT_ROOT%
if "%BUILD_TYPE%"=="release" (
    cargo build -p agent-deck-daemon --release
) else (
    cargo build -p agent-deck-daemon
)
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Daemon build failed
    exit /b 1
)
echo [INFO] Daemon build complete
:skip_daemon

REM Build GUI
if "%TARGET%"=="gui" goto build_gui
if "%TARGET%"=="all" goto build_gui
goto skip_gui

:build_gui
echo [INFO] Building GUI...
cd %PROJECT_ROOT%\gui

REM Install npm dependencies if needed
if not exist "node_modules" (
    echo [INFO] Installing npm dependencies...
    call npm install
)

if "%BUILD_TYPE%"=="release" (
    call npm run tauri:build
) else (
    call npm run tauri:build:debug
)
if %ERRORLEVEL% neq 0 (
    echo [ERROR] GUI build failed
    exit /b 1
)
echo [INFO] GUI build complete
:skip_gui

echo.
echo [INFO] Build complete!
echo.
echo Build artifacts:
if "%TARGET%"=="daemon" goto show_daemon
if "%TARGET%"=="all" goto show_daemon
goto skip_show_daemon

:show_daemon
if "%BUILD_TYPE%"=="release" (
    echo   Daemon: %PROJECT_ROOT%\target\release\agent-deck-daemon.exe
) else (
    echo   Daemon: %PROJECT_ROOT%\target\debug\agent-deck-daemon.exe
)
:skip_show_daemon

if "%TARGET%"=="gui" goto show_gui
if "%TARGET%"=="all" goto show_gui
goto skip_show_gui

:show_gui
echo   GUI: %PROJECT_ROOT%\gui\src-tauri\target\release\bundle\
:skip_show_gui

endlocal
