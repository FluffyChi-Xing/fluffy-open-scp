//! OpenSCP — SimCity (2013) package explorer（fluffy-open-scp）。
//!
//! Tauri 应用壳：将 `dbpf` / `rw4` / `sc-properties` / `sc-exporter` 各解析库
//! 以 command 形式暴露给 Vue 前端。原 WPF GUI（`MainWindow.xaml` 及各 View）
//! 的功能由前端 + commands 迁移实现。
//!
//! 外部工具调度（对应原 `CliRunner`）：
//! - 音频导出：bundled vgmstream（`Tools\vgmstream\`）
//! - 视频导出：ffmpeg（`Tools\ffmpeg\` 或 PATH）

mod activity;
mod media_tools;
mod package_browser;
mod package_service;
mod settings;
mod workspace;
mod wwise;

use activity::{
    AppState, activity_clear, activity_list_events, activity_list_operations,
    activity_list_packages,
};
use media_tools::{MediaTools, application_dir, resolve_tools};
use package_browser::{list_game_tree, list_package_files};
use package_service::{
    close_package, export, export_status, list_resources, open_package, patch_property_overlay,
    read_lot_editor_session, read_property_preview, read_resource_bytes, read_resource_data,
    read_rw4_preview, read_rw4_section_detail, read_wwise_bank, resolve_name, resolve_names,
};
use settings::{game_directory_detect, settings_get, settings_set_game_directory};
use tauri::{Manager, PhysicalPosition};
use workspace::{
    workspace_create_folder, workspace_create_markdown, workspace_get, workspace_list,
    workspace_move, workspace_read_markdown, workspace_rename, workspace_set_root,
    workspace_write_markdown,
};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! OpenSCP Tauri backend is running.")
}

#[tauri::command]
fn detect_media_tools() -> MediaTools {
    application_dir()
        .map(|directory| resolve_tools(&directory, std::env::var_os("PATH").as_deref()))
        .unwrap_or_else(|| resolve_tools(std::path::Path::new("."), None))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("openscp.db");
            let store = sc_store::Store::open(database_path)?;
            app.manage(AppState::new(app.handle().clone(), store));
            if let Some(window) = app.get_webview_window("main")
                && let Some(monitor) = window.primary_monitor()?
            {
                let area = monitor.work_area();
                let size = window.outer_size()?;
                let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
                let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
                window.set_position(PhysicalPosition::new(
                    x.max(area.position.x),
                    y.max(area.position.y),
                ))?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            activity_list_operations,
            activity_list_events,
            activity_list_packages,
            activity_clear,
            list_game_tree,
            list_package_files,
            open_package,
            close_package,
            list_resources,
            read_resource_bytes,
            read_resource_data,
            read_lot_editor_session,
            patch_property_overlay,
            read_property_preview,
            read_rw4_preview,
            read_rw4_section_detail,
            read_wwise_bank,
            resolve_name,
            resolve_names,
            export,
            export_status,
            detect_media_tools,
            settings_get,
            settings_set_game_directory,
            game_directory_detect,
            workspace_get,
            workspace_set_root,
            workspace_list,
            workspace_create_folder,
            workspace_read_markdown,
            workspace_write_markdown,
            workspace_create_markdown,
            workspace_rename,
            workspace_move,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
