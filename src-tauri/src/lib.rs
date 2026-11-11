mod config;
mod monitor;
mod moonraker;
mod tray;

use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;

pub struct AppState {
    pub client: reqwest::Client,
    pub url: Mutex<Option<String>>,
    pub last: Mutex<Option<monitor::StatusPayload>>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Option<String> {
    state.url.lock().unwrap().clone()
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Option<monitor::StatusPayload> {
    state.last.lock().unwrap().clone()
}

/// Normalizes and validates the URL against /server/info; persists only on success.
#[tauri::command]
async fn save_config(app: AppHandle, state: State<'_, AppState>, url: String) -> Result<String, String> {
    let base = moonraker::normalize_url(&url)?;
    moonraker::server_info(&state.client, &base).await?;
    config::save(&app, &config::Config { base_url: Some(base.clone()) })?;
    *state.last.lock().unwrap() = None;
    *state.url.lock().unwrap() = Some(base.clone());
    Ok(base)
}

#[tauri::command]
async fn printer_action(state: State<'_, AppState>, action: moonraker::Action) -> Result<(), String> {
    let base = state.url.lock().unwrap().clone().ok_or("No printer configured")?;
    moonraker::post_action(&state.client, &base, action).await
}

#[tauri::command]
fn test_notification(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("Moonraker Monitor")
        .body("Test notification")
        .show()
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            // Keep monitoring in the tray: closing the window only hides it.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;
            let url = config::load(app.handle()).base_url;
            app.manage(AppState {
                client,
                url: Mutex::new(url),
                last: Mutex::new(None),
            });
            tray::setup(app.handle())?;
            monitor::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_status,
            save_config,
            printer_action,
            test_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
