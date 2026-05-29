mod models;
mod platform;
mod service;
mod storage;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

use models::{AppOverview, LanguageMode};
use service::SmartSwitcherService;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    service: Arc<Mutex<SmartSwitcherService>>,
    monitor_started: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        Self {
            service: Arc::new(Mutex::new(SmartSwitcherService::new())),
            monitor_started: AtomicBool::new(false),
        }
    }
}

#[tauri::command]
fn get_overview(state: State<'_, AppState>) -> Result<AppOverview, String> {
    let service = state.service.lock().map_err(|err| err.to_string())?;
    Ok(service.overview())
}

#[tauri::command]
fn upsert_manual_rule(
    state: State<'_, AppState>,
    executable: String,
    preferred_language: LanguageMode,
    note: Option<String>,
) -> Result<AppOverview, String> {
    let mut service = state.service.lock().map_err(|err| err.to_string())?;
    service.upsert_manual_rule(executable, preferred_language, note)?;
    Ok(service.overview())
}

#[tauri::command]
fn delete_manual_rule(
    state: State<'_, AppState>,
    executable: String,
) -> Result<AppOverview, String> {
    let mut service = state.service.lock().map_err(|err| err.to_string())?;
    service.delete_manual_rule(&executable)?;
    Ok(service.overview())
}

#[tauri::command]
fn update_settings(
    state: State<'_, AppState>,
    auto_switch_enabled: bool,
    learning_enabled: bool,
) -> Result<AppOverview, String> {
    let mut service = state.service.lock().map_err(|err| err.to_string())?;
    service.update_settings(auto_switch_enabled, learning_enabled)?;
    Ok(service.overview())
}

#[tauri::command]
fn learn_current_preference(state: State<'_, AppState>) -> Result<AppOverview, String> {
    let mut service = state.service.lock().map_err(|err| err.to_string())?;
    service.learn_current_preference()?;
    Ok(service.overview())
}

fn emit_overview(app: &AppHandle, overview: &AppOverview) {
    let _ = app.emit("smart-keyword://overview-updated", overview);
}

fn start_monitor(app: AppHandle, state: Arc<Mutex<SmartSwitcherService>>) {
    thread::spawn(move || loop {
        let (overview_to_emit, poll_interval_ms) = {
            let mut service = match state.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    thread::sleep(Duration::from_millis(1200));
                    continue;
                }
            };

            if service.poll_once().is_ok() {
                let overview = service.overview();
                let poll_interval_ms = overview.settings.poll_interval_ms;
                (Some(overview), poll_interval_ms)
            } else {
                (None, 1200)
            }
        };

        if let Some(overview) = overview_to_emit {
            emit_overview(&app, &overview);
        }

        thread::sleep(Duration::from_millis(poll_interval_ms.max(400)));
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::new();
            let handle = app.handle().clone();

            let initial_overview = {
                let mut service = state
                    .service
                    .lock()
                    .map_err(|err| std::io::Error::other(err.to_string()))?;
                service.initialize().map_err(std::io::Error::other)?;
                service.overview()
            };

            emit_overview(&handle, &initial_overview);

            if !state.monitor_started.swap(true, Ordering::SeqCst) {
                start_monitor(handle, Arc::clone(&state.service));
            }

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            upsert_manual_rule,
            delete_manual_rule,
            update_settings,
            learn_current_preference
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
