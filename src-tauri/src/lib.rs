mod elevation;
mod programs;
mod uninstall;

use std::collections::HashMap;

use serde_json::json;
use tauri::AppHandle;
use tauri_tray_base::{
    apply_window_settings, install_state, setup_tray, sync_autostart, was_launched_minimised,
    with_common_plugins, TrayBaseOptions, TrayExtraItem, TraySetupOptions,
};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = with_common_plugins(tauri::Builder::default())
        .invoke_handler(tauri::generate_handler![
            tauri_tray_base::settings_get,
            tauri_tray_base::settings_set,
            tauri_tray_base::app_get_state,
            list_programs,
            check_elevated,
            uninstall_selected
        ])
        .setup(|app| {
            let mut defaults = HashMap::new();
            defaults.insert("alwaysOnTop".into(), json!(false));
            defaults.insert("startMinimised".into(), json!(false));
            defaults.insert("opacity".into(), json!(1.0));

            install_state(
                app.handle(),
                TrayBaseOptions {
                    app_name: "Uninstall a Program (Bulk Edition)".into(),
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
            sync_autostart(app.handle());

            if was_launched_minimised() {
                tauri_tray_base::hide_main(app.handle());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            tauri_tray_base::on_window_event(window, event);
        });

    builder
        .run(tauri::generate_context!())
        .expect("error while running Uninstall a Program (Bulk Edition)");
}
