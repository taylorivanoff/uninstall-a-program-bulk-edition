mod categories;
mod elevation;
mod programs;
mod size;
mod uninstall;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::json;
use tauri::AppHandle;
use tauri_tray_base::{
    apply_window_settings, install_state, setup_tray, sync_autostart, with_common_plugins,
    TrayBaseOptions, TrayExtraItem, TraySetupOptions,
};

struct SizeProbeGate {
    generation: AtomicU64,
}

#[tauri::command]
fn list_programs(show_system: bool) -> Result<Vec<programs::InstalledProgram>, String> {
    programs::list_installed_programs(show_system)
}

#[tauri::command]
fn check_elevated() -> bool {
    elevation::is_elevated()
}

#[tauri::command]
async fn uninstall_selected(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || uninstall::uninstall_programs(app, ids))
        .await
        .map_err(|e| format!("Uninstall task failed: {e}"))?
}

#[tauri::command]
fn probe_missing_sizes(
    app: AppHandle,
    gate: tauri::State<'_, Arc<SizeProbeGate>>,
    items: Vec<size::SizeProbeItem>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }

    let generation = gate.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let gate = Arc::clone(&gate);
    tauri::async_runtime::spawn_blocking(move || {
        size::probe_missing_sizes(app, items, || {
            gate.generation.load(Ordering::SeqCst) == generation
        });
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let size_gate = Arc::new(SizeProbeGate {
        generation: AtomicU64::new(0),
    });

    let builder = with_common_plugins(tauri::Builder::default())
        .manage(size_gate)
        .invoke_handler(tauri::generate_handler![
            tauri_tray_base::settings_get,
            tauri_tray_base::settings_set,
            tauri_tray_base::app_get_state,
            list_programs,
            check_elevated,
            uninstall_selected,
            probe_missing_sizes
        ])
        .setup(|app| {
            let mut defaults = HashMap::new();
            defaults.insert("alwaysOnTop".into(), json!(false));
            defaults.insert("startMinimised".into(), json!(false));
            defaults.insert("opacity".into(), json!(1.0));

            install_state(
                app.handle(),
                TrayBaseOptions {
                    app_name: "Uninstall Many Programs".into(),
                    settings_file_name: "uninstall-a-program-settings.json".into(),
                    defaults,
                    show_always_on_top: false,
                    extra_tray_items: vec![TrayExtraItem {
                        id: "refresh".into(),
                        label: "Refresh".into(),
                    }],
                    ..Default::default()
                },
            )?;

            setup_tray(app.handle(), TraySetupOptions::default())?;
            apply_window_settings(app.handle());
            tauri_tray_base::enable_frameless_chrome(app.handle());
            sync_autostart(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| {
            tauri_tray_base::on_window_event(window, event);
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running Uninstall Many Programs");
}
