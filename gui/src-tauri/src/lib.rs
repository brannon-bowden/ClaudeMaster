mod commands;
mod event_listener;
mod ipc_client;

use ipc_client::IpcClient;

/// Global state for the daemon connection
pub struct DaemonState {
    pub client: IpcClient,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DaemonState {
            client: IpcClient::new(),
        })
        .setup(|app| {
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
            commands::send_input,
            commands::resize_session,
            commands::list_groups,
            commands::create_group,
            commands::delete_group,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
