//! Debug 工具解锁：生成并原子落盘 overlay 包。
//!
//! 产物放入游戏 mod 目录（`SimCityUserData/Packages/`）后由游戏覆盖加载，
//! 重新打开三个被 property 门控的开发期工具。补丁机制见
//! [`sc_properties::debug_tools`]（enable_debug_tools 探针的产品化，P0-2）。
use serde::Serialize;

use sc_properties::debug_tools::{
    self, DebugToolPatch, DEFAULT_CATEGORY, DEFAULT_UI_CATEGORY,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugToolsOverlayDto {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub out_path: String,
    pub tools: Vec<DebugToolPatch>,
}

fn parse_hex_opt(value: Option<String>, fallback: u32) -> Result<u32, String> {
    match value {
        None => Ok(fallback),
        Some(v) => u32::from_str_radix(v.trim_start_matches("0x"), 16).map_err(|e| e.to_string()),
    }
}

/// 生成 debug 工具解锁 overlay 并写入 `out_path`（原子替换）。
///
/// `category`/`ui_category` 省略时用探针默认值（建筑工具可见分类）。
#[tauri::command]
pub fn debug_tools_write_overlay(
    base_package_path: String,
    out_path: String,
    category: Option<String>,
    ui_category: Option<String>,
) -> Result<DebugToolsOverlayDto, String> {
    let category = parse_hex_opt(category, DEFAULT_CATEGORY)?;
    let ui_category = parse_hex_opt(ui_category, DEFAULT_UI_CATEGORY)?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&base_package_path))
        .map_err(|e| e.to_string())?;
    let (entries, tools) = debug_tools::build_debug_tools_overlay(&package, category, ui_category)?;
    let bytes = dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())?;
    let size_bytes = bytes.len();
    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    crate::atomic_fs::write_atomic(std::path::Path::new(&out_path), &bytes, true)
        .map_err(|e| e.to_string())?;
    Ok(DebugToolsOverlayDto {
        entry_count: entries.len(),
        size_bytes,
        out_path,
        tools,
    })
}
