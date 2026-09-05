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
mod package_service;
mod workspace;

use activity::{
    AppState, activity_clear, activity_list_events, activity_list_operations,
    activity_list_packages,
};
use media_tools::{MediaTools, application_dir, resolve_tools};
use package_service::{
    close_package, export, export_status, list_resources, open_package, read_resource_bytes,
    resolve_name,
};
use tauri::Manager;
use workspace::{
    workspace_create_folder, workspace_create_markdown, workspace_get, workspace_list,
    workspace_read_markdown, workspace_set_root, workspace_write_markdown,
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
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("openscp.db");
            let store = sc_store::Store::open(database_path)?;
            app.manage(AppState::new(app.handle().clone(), store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            activity_list_operations,
            activity_list_events,
            activity_list_packages,
            activity_clear,
            open_package,
            close_package,
            list_resources,
            read_resource_bytes,
            resolve_name,
            export,
            export_status,
            detect_media_tools,
            workspace_get,
            workspace_set_root,
            workspace_list,
            workspace_create_folder,
            workspace_read_markdown,
            workspace_write_markdown,
            workspace_create_markdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
