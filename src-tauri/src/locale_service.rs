//! Locale 文本编辑（Mod Studio WP2）：读取已打开 package 内的字符串表
//! （type `0x0A98EAF0`），编辑后以「确定性 overlay package」写回——
//! 只含被修改的资源、同 TGI、明文存储，游戏按后加载优先覆盖原版。
//!
//! 这条 脏条目 → 重序列化(BOM+JSON) → overlay 写出 链路是所有编辑面板
//! 共用的写回试金石（见 docs/roadmap/workspace-panels.md §3.2）。

use std::path::PathBuf;
use std::sync::Arc;

use dbpf::{OverlayEntry, ResourceId, write_uncompressed_overlay_to_path};
use sc_properties::locale::{
    LocaleItem, parse_locale_items, serialize_locale_items,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};

/// Locale JSON string-table resource type id（与 sc-properties 保持一致）。
const LOCALE_RESOURCE_TYPE: u32 = 0x0A98_EAF0;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleTgi {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

impl LocaleTgi {
    fn resource_id(self) -> ResourceId {
        ResourceId {
            type_id: self.type_id,
            group: self.group,
            instance: self.instance,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleTableSummary {
    pub tgi: LocaleTgi,
    pub table_id: u32,
    pub string_count: usize,
    /// 前三条字符串预览（注释行跳过），供列表辨认表用途。
    pub sample: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleTablesResponse {
    pub tables: Vec<LocaleTableSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleItemsRequest {
    pub package_id: u64,
    pub tgi: LocaleTgi,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleItemsResponse {
    pub tgi: LocaleTgi,
    pub items: Vec<LocaleItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleTableEdit {
    pub tgi: LocaleTgi,
    pub items: Vec<LocaleItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteLocaleOverlayRequest {
    pub edits: Vec<LocaleTableEdit>,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteLocaleOverlayResponse {
    pub output_path: String,
    pub entry_count: usize,
    pub bytes_written: u64,
}

/// `spawn_blocking` 需要 'static：把 PackageManager（Arc）复制出去即可。
fn spawn_packages(state: &State<'_, AppState>) -> Arc<crate::package_service::PackageManager> {
    Arc::clone(&state.packages)
}

fn locale_tables_inner(
    packages: &crate::package_service::PackageManager,
    package_id: u64,
) -> Result<LocaleTablesResponse, CommandError> {
    let package = packages
        .get(package_id)
        .map_err(|e| CommandError::internal(e.to_string()))?;
    let mut tables = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let data = package.read(entry).map_err(|e| CommandError::internal(e.to_string()))?;
        let items = match parse_locale_items(&data) {
            Ok(items) => items,
            // 解析失败的表也列出，但计 0 并给错误采样，编辑器可看到坏表
            Err(error) => {
                tables.push(LocaleTableSummary {
                    tgi: LocaleTgi {
                        type_id: entry.id.type_id,
                        group: entry.id.group,
                        instance: entry.id.instance,
                    },
                    table_id: entry.id.instance,
                    string_count: 0,
                    sample: vec![format!("<parse error: {error}>")],
                });
                continue;
            }
        };
        let string_count = items.iter().filter(|item| item.id.is_some()).count();
        let sample = items
            .iter()
            .filter(|item| item.id.is_some())
            .take(3)
            .map(|item| item.text.chars().take(48).collect())
            .collect();
        tables.push(LocaleTableSummary {
            tgi: LocaleTgi {
                type_id: entry.id.type_id,
                group: entry.id.group,
                instance: entry.id.instance,
            },
            table_id: entry.id.instance,
            string_count,
            sample,
        });
    }
    tables.sort_by_key(|table| table.table_id);
    Ok(LocaleTablesResponse { tables })
}

#[tauri::command]
pub async fn locale_tables(
    state: State<'_, AppState>,
    package_id: u64,
) -> Result<LocaleTablesResponse, CommandError> {
    let packages = spawn_packages(&state);
    tauri::async_runtime::spawn_blocking(move || locale_tables_inner(&packages, package_id))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn locale_items(
    state: State<'_, AppState>,
    request: LocaleItemsRequest,
) -> Result<LocaleItemsResponse, CommandError> {
    let packages = spawn_packages(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let package = packages
            .get(request.package_id)
            .map_err(|e| CommandError::internal(e.to_string()))?;
        let entry = package
            .entry(request.tgi.resource_id())
            .ok_or_else(|| CommandError::new("not_found", "locale resource not found"))?;
        let data = package.read(entry).map_err(|e| CommandError::internal(e.to_string()))?;
        let items = parse_locale_items(&data)
            .map_err(|error| CommandError::new("parse_failed", error))?;
        Ok(LocaleItemsResponse { tgi: request.tgi, items })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn write_locale_overlay(
    state: State<'_, AppState>,
    request: WriteLocaleOverlayRequest,
) -> Result<WriteLocaleOverlayResponse, CommandError> {
    if request.edits.is_empty() {
        return Err(CommandError::new("invalid_argument", "no edits to write"));
    }
    // 写 overlay 不需要已打开的 package 句柄（数据全部来自前端编辑状态），
    // 但保留对 state 的依赖以便未来追加活动记录。
    let _ = &state;
    let output_path = PathBuf::from(&request.output_path);
    tauri::async_runtime::spawn_blocking(move || {
        let mut entries = Vec::with_capacity(request.edits.len());
        for edit in &request.edits {
            let data = serialize_locale_items(&edit.items)
                .map_err(|error| CommandError::new("serialize_failed", error))?;
            entries.push(OverlayEntry::new(edit.tgi.resource_id(), data));
        }
        write_uncompressed_overlay_to_path(&output_path, &entries)
            .map_err(|error| CommandError::new("write_failed", error.to_string()))?;
        let bytes_written = std::fs::metadata(&output_path)
            .map(|meta| meta.len())
            .map_err(|error| CommandError::new("write_failed", error.to_string()))?;
        Ok(WriteLocaleOverlayResponse {
            output_path: output_path.to_string_lossy().into_owned(),
            entry_count: entries.len(),
            bytes_written,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_write_round_trip() {
        let dir = std::env::temp_dir().join(format!("openscp-locale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("overlay.package");
        let items = vec![
            LocaleItem { key: "//".into(), id: None, text: "note".into() },
            LocaleItem { key: "0x00000001".into(), id: Some(1), text: "你好".into() },
        ];
        let data = serialize_locale_items(&items).unwrap();
        let entries = vec![OverlayEntry::new(
            ResourceId { type_id: LOCALE_RESOURCE_TYPE, group: 0x9E14D920, instance: 0x00000001 },
            data,
        )];
        write_uncompressed_overlay_to_path(&out, &entries).unwrap();
        let package = dbpf::Package::open(&out).unwrap();
        let entry = package
            .entry(ResourceId { type_id: LOCALE_RESOURCE_TYPE, group: 0x9E14D920, instance: 0x00000001 })
            .unwrap();
        let read_back = package.read(entry).unwrap();
        assert_eq!(parse_locale_items(&read_back).unwrap(), items);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
