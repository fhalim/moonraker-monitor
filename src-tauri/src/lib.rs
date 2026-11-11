mod config;
mod monitor;
mod moonraker;
mod tray;

use std::{sync::Mutex, time::Duration};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;

pub struct AppState {
    pub client: reqwest::Client,
    pub url: Mutex<Option<String>>,
    pub cameras: Mutex<Vec<String>>,
    pub last: Mutex<Option<monitor::StatusPayload>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    base_url: Option<String>,
    cameras: Vec<String>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> AppConfig {
    AppConfig {
        base_url: state.url.lock().unwrap().clone(),
        cameras: state.cameras.lock().unwrap().clone(),
    }
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Option<monitor::StatusPayload> {
    state.last.lock().unwrap().clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Discovery {
    base_url: String,
    webcams: Vec<moonraker::Webcam>,
}

/// Validates the URL against /server/info and lists webcams. A webcam
/// listing failure (e.g. no webcam component) just means no cameras.
#[tauri::command]
async fn discover(state: State<'_, AppState>, url: String) -> Result<Discovery, String> {
    let base_url = moonraker::normalize_url(&url)?;
    moonraker::server_info(&state.client, &base_url).await?;
    let webcams = moonraker::list_webcams(&state.client, &base_url)
        .await
        .unwrap_or_default();
    Ok(Discovery { base_url, webcams })
}

/// Persists the URL and selected camera uids. `discover` validated the URL.
#[tauri::command]
async fn save_config(app: AppHandle, state: State<'_, AppState>, url: String, cameras: Vec<String>) -> Result<String, String> {
    let base = moonraker::normalize_url(&url)?;
    config::save(&app, &config::Config { base_url: Some(base.clone()), cameras: cameras.clone() })?;
    *state.last.lock().unwrap() = None;
    *state.url.lock().unwrap() = Some(base.clone());
    *state.cameras.lock().unwrap() = cameras;
    Ok(base)
}

/// Selected webcams that still exist on the printer.
#[tauri::command]
async fn get_webcams(state: State<'_, AppState>) -> Result<Vec<moonraker::Webcam>, String> {
    let base = state.url.lock().unwrap().clone().ok_or("No printer configured")?;
    let selected = state.cameras.lock().unwrap().clone();
    if selected.is_empty() {
        return Ok(vec![]);
    }
    let all = moonraker::list_webcams(&state.client, &base).await?;
    Ok(all.into_iter().filter(|w| selected.contains(&w.uid)).collect())
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
                let _ = window.emit("window-visible", false);
            }
        })
        .setup(|app| {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()?;
            let cfg = config::load(app.handle());
            app.manage(AppState {
                client,
                url: Mutex::new(cfg.base_url),
                cameras: Mutex::new(cfg.cameras),
                last: Mutex::new(None),
            });
            tray::setup(app.handle())?;
            monitor::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_status,
            discover,
            save_config,
            get_webcams,
            printer_action,
            test_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
