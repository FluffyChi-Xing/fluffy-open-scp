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
mod annotations;
mod atomic_fs;
mod locale_service;
mod media_tools;
mod mod_project;
mod overrides;
mod package_browser;
mod package_service;
mod raster_edit;
mod render_telemetry;
mod settings;
mod stats;
mod version_service;
mod workspace;
mod wwise;

use activity::{
    AppState, activity_clear, activity_list_events, activity_list_operations,
    activity_list_packages,
};
use annotations::{
    annotation_create, annotation_delete, annotation_topic_stats, annotation_update,
    annotations_for_tgi, annotations_list,
};
use locale_service::{locale_items, locale_tables, write_locale_overlay};
use media_tools::{MediaTools, application_dir, resolve_tools};
use mod_project::{
    mod_group_create, mod_group_delete, mod_group_list, mod_group_rename, mod_project_create,
    mod_project_delete, mod_project_list, mod_project_stats, mod_project_update, setup_status,
    studio_complete_onboarding, studio_set_mod_root,
};
use overrides::override_scan;
use raster_edit::{
    create_lot_overlay, list_property_documents, read_image_rgba, read_lot_material,
    read_raster_rgba, register_decal_entry, save_raster_overlay,
};
use package_browser::{list_game_tree, list_package_files};
use package_service::{
    close_package, export, export_status, list_resources, open_package, patch_property_overlay,
    read_decal_dictionary, read_decal_images, read_image_preview, read_lot_editor_session,
    read_lot_model_meshes, read_property_preview, read_raster_preview, read_resource_bytes,
    read_resource_data, read_resource_text, read_resource_text_range, read_rw4_preview,
    read_rw4_section_detail,
    read_wwise_bank, resolve_name, resolve_names, write_export_file,
};
use render_telemetry::{render_telemetry_clear, render_telemetry_record, render_telemetry_summary};
use settings::{game_directory_detect, settings_get, settings_set_game_directory};
use stats::package_statistics;
use tauri::{Manager, PhysicalPosition};
use version_service::{
    version_capture_baseline, version_changeset_detail, version_delete_changeset,
    version_list_changesets, version_list_targets, version_record_changeset, version_rollback,
};
use workspace::{
    code_manifest, code_package_entries, code_package_info, code_read_text, code_resource_preview,
    code_tree, code_write_text, workspace_create_folder, workspace_create_markdown,
    workspace_get, workspace_list, workspace_move, workspace_read_markdown, workspace_rename,
    workspace_set_root, workspace_write_markdown,
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
            annotation_create,
            annotation_update,
            annotation_delete,
            annotations_for_tgi,
            annotations_list,
            annotation_topic_stats,
            package_statistics,
            override_scan,
            locale_tables,
            locale_items,
            write_locale_overlay,
            activity_list_operations,
            activity_list_events,
            activity_list_packages,
            activity_clear,
            render_telemetry_record,
            render_telemetry_summary,
            render_telemetry_clear,
            version_list_targets,
            version_list_changesets,
            version_changeset_detail,
            version_record_changeset,
            version_capture_baseline,
            version_rollback,
            version_delete_changeset,
            list_game_tree,
            list_package_files,
            open_package,
            close_package,
            list_resources,
            read_resource_bytes,
            read_resource_data,
            read_resource_text,
            read_resource_text_range,
            read_lot_editor_session,
            read_lot_model_meshes,
            read_raster_preview,
            read_image_preview,
            write_export_file,
            patch_property_overlay,
            read_property_preview,
            read_image_rgba,
            read_raster_rgba,
            save_raster_overlay,
            list_property_documents,
            read_lot_material,
            register_decal_entry,
            create_lot_overlay,
            read_decal_dictionary,
            read_decal_images,
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
            code_tree,
            code_read_text,
            code_write_text,
            code_package_info,
            code_package_entries,
            code_resource_preview,
            code_manifest,
            workspace_create_markdown,
            workspace_rename,
            workspace_move,
            setup_status,
            studio_set_mod_root,
            studio_complete_onboarding,
            mod_project_list,
            mod_project_create,
            mod_project_update,
            mod_project_delete,
            mod_group_list,
            mod_group_create,
            mod_group_rename,
            mod_group_delete,
            mod_project_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
