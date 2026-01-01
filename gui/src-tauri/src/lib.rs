mod commands;
mod daemon_launcher;
mod event_listener;
mod ipc_client;

use ipc_client::IpcClient;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Global state for the daemon connection
pub struct DaemonState {
    pub client: IpcClient,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DaemonState {
            client: IpcClient::new(),
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Ensure daemon is running (installs LaunchAgent if needed)
            tauri::async_runtime::spawn(async move {
                info!("Ensuring daemon is running...");
                if let Err(e) = daemon_launcher::ensure_daemon_running(&handle).await {
                    error!("Failed to ensure daemon is running: {}", e);
                }
            });

            // Start event listener in background
            event_listener::start_event_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_daemon,
            commands::is_daemon_connected,
            commands::ping_daemon,
            commands::list_sessions,
            commands::create_session,
            commands::stop_session,
            commands::delete_session,
            commands::fork_session,
            commands::restart_session,
            commands::send_input,
            commands::resize_session,
            commands::update_session,
            commands::reorder_session,
            commands::list_groups,
            commands::create_group,
            commands::delete_group,
            commands::update_group,
            commands::reorder_group,
            commands::shutdown_daemon,
            commands::uninstall_daemon_service,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
