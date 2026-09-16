//! Locale 文本编辑（Mod Studio WP2）：读取已打开 package 内的字符串表
//! （type `0x0A98EAF0`），编辑后以「确定性 overlay package」写回——
//! 只含被修改的资源、同 TGI、明文存储，游戏按后加载优先覆盖原版。
//!
//! 这条 脏条目 → 重序列化(BOM+JSON) → overlay 写出 链路是所有编辑面板
//! 共用的写回试金石（见 docs/roadmap/workspace-panels.md §3.2）。

use std::path::PathBuf;
use std::sync::Arc;

use dbpf::{OverlayEntry, ResourceId, write_uncompressed_overlay};
use sc_properties::locale::{
    LocaleItem, parse_locale_items, serialize_locale_items,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use sc_store::ChangeItemInput;

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
    /// 写出的条目总数（= 本次编辑 + 从目标里合并保留的）。
    pub entry_count: usize,
    pub bytes_written: u64,
    /// 来自目标文件、本次未被编辑因而原样保留的条目数。
    pub merged_kept: usize,
    /// 版本记录（best-effort：失败时为 None，原因见 `diagnostic`）。
    pub changeset_id: Option<i64>,
    pub revision: Option<i64>,
    pub diagnostic: Option<String>,
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

// ---- 游戏 Locale 包 text 引用解析（语义预览卡用） ----
//
// 实证（2026-09-16 locale_probe）：SimAction/Alert/MapLayer 的 text 引用指向的
// 字符串表不在主包（Game 仅 3 张/App 6 张杂表），而在
// `<game_data_path>/Locale/<lang>/Data.package`（en-us 362 张 / zh-tw 356 张）。

/// 归一化语言目录名：仅允许小写字母数字与连字符（路径安全）。
fn locale_dir_name(lang: &str) -> String {
    let lowered = lang.to_ascii_lowercase();
    let sanitized: String = lowered
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .collect();
    if sanitized.is_empty() {
        "en-us".into()
    } else {
        sanitized
    }
}

/// 缓存条目：解析好的字符串表 + 该包内容语言的 CJK 占比判定。
#[derive(Debug)]
pub(crate) struct CachedLocale {
    pub locale: Arc<sc_properties::Locale>,
    /// 抽样判定包内容是否为中文（离线整合包存在目录名与内容错位：
    /// zh-tw 目录装英文、en-us 目录装中文，2026-09-17 locale_lang_probe 实证）。
    pub cjk: bool,
}

/// 抽样判定 Locale 包内容是否为中文（CJK 字符占比 ≥10%）。
fn package_is_cjk(package: &dbpf::Package) -> bool {
    let mut cjk = 0usize;
    let mut total = 0usize;
    'outer: for entry in package.entries() {
        if entry.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        if let Ok(items) = parse_locale_items(&data) {
            for item in items {
                if item.id.is_none() {
                    continue;
                }
                for ch in item.text.chars() {
                    total += 1;
                    if matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}') {
                        cjk += 1;
                    }
                }
                if total >= 4000 {
                    break 'outer;
                }
            }
        }
    }
    total > 0 && cjk * 10 >= total
}

/// 读取（带缓存 + 内容语言探测）游戏 Locale 包的字符串表集合。
/// 目录名可能不可信（整合包错位），故按内容 CJK 占比自动回退：
/// 中文 UI 优先 zh-tw、内容非中文则回退 en-us；英文 UI 反之。
/// game_data_path 未配置或两包均缺失时返回 None（优雅降级为未解析）。
pub(crate) fn game_locale(
    manager: &crate::package_service::PackageManager,
    store: &sc_store::Store,
    lang: &str,
) -> Option<Arc<sc_properties::Locale>> {
    let settings = store.app_settings().ok()??;
    let game_data = PathBuf::from(settings.game_data_path?);
    let requested = locale_dir_name(lang);
    let wants_cjk = requested.starts_with("zh");
    let other = if wants_cjk { "en-us" } else { "zh-tw" };

    let mut candidates = vec![requested];
    if !candidates.iter().any(|d| d == other) {
        candidates.push(other.to_string());
    }

    let mut fallback: Option<Arc<sc_properties::Locale>> = None;
    for dir in candidates {
        let path = game_data.join("Locale").join(dir).join("Data.package");
        let cached = {
            let mut cache = manager.locale_cache.lock().ok()?;
            match cache.get(&path) {
                Some(hit) => Some((Arc::clone(&hit.locale), hit.cjk)),
                None => {
                    let package = dbpf::Package::open(&path).ok()?;
                    let resources = package.entries().iter().filter_map(|entry| {
                        if entry.id.type_id != LOCALE_RESOURCE_TYPE {
                            return None;
                        }
                        let data = package.read(entry).ok()?;
                        Some((entry.id.instance, data))
                    });
                    let locale = Arc::new(sc_properties::Locale::from_resources(resources).ok()?);
                    let cjk = package_is_cjk(&package);
                    cache.insert(path.clone(), CachedLocale {
                        locale: Arc::clone(&locale),
                        cjk,
                    });
                    Some((locale, cjk))
                }
            }
        };
        if let Some((locale, cjk)) = cached {
            if cjk == wants_cjk {
                return Some(locale);
            }
            if fallback.is_none() {
                fallback = Some(locale);
            }
        }
    }
    fallback
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTextRef {
    pub table_id: u32,
    pub instance_id: u32,
    pub text: Option<String>,
}

/// 在给定 Locale 里解析一批 text 引用（顺序与输入一致）。
pub(crate) fn resolve_texts(
    locale: Option<&sc_properties::Locale>,
    refs: impl IntoIterator<Item = (u32, u32)>,
) -> Vec<ResolvedTextRef> {
    refs.into_iter()
        .map(|(table_id, instance_id)| ResolvedTextRef {
            table_id,
            instance_id,
            text: locale
                .and_then(|l| l.get(table_id, instance_id))
                .map(str::to_owned),
        })
        .collect()
}

/// 写入核心（脱离 Tauri State，便于测试）。
fn write_locale_overlay_inner(
    store: &sc_store::Store,
    request: &WriteLocaleOverlayRequest,
) -> Result<WriteLocaleOverlayResponse, CommandError> {
    let output_path = PathBuf::from(&request.output_path);
    // 1) 读目标已有条目 —— **合并保留**：本次没编辑到的资源必须原样写回，
    //    否则会把用户 overlay 里的其它资源整体抹掉（真实数据丢失隐患）。
    let mut merged: std::collections::BTreeMap<(u32, u32, u32), Vec<u8>> =
        crate::version_service::read_target_entries(&output_path)
            .into_iter()
            .map(|(id, data)| ((id.type_id, id.group, id.instance), data))
            .collect();
    let mut change_items = Vec::with_capacity(request.edits.len());
    for edit in &request.edits {
        let id = edit.tgi.resource_id();
        let key = (id.type_id, id.group, id.instance);
        let before = merged.get(&key).cloned();
        let data = serialize_locale_items(&edit.items)
            .map_err(|error| CommandError::new("serialize_failed", error))?;
        change_items.push(ChangeItemInput {
            type_id: key.0,
            group_id: key.1,
            instance: key.2,
            before,
            after: Some(data.clone()),
        });
        merged.insert(key, data);
    }
    let merged_kept = merged.len().saturating_sub(request.edits.len());
    let entries: Vec<OverlayEntry> = merged
        .into_iter()
        .map(|((type_id, group, instance), data)| {
            OverlayEntry::new(ResourceId { type_id, group, instance }, data)
        })
        .collect();
    let overlay = write_uncompressed_overlay(&entries)
        .map_err(|error| CommandError::new("write_failed", error.to_string()))?;
    let bytes_written = overlay.len() as u64;
    let file_digest = crate::version_service::sha256_hex(&overlay);
    // 原子替换：崩溃/中断不会留下半截 overlay。
    crate::atomic_fs::write_atomic(&output_path, &overlay, true)
        .map_err(|error| CommandError::new("write_failed", error.to_string()))?;
    // 2) 记录版本。**记录失败不能让写入失败** —— 把原因放进 diagnostic 回给前端。
    let target_path = output_path.to_string_lossy().into_owned();
    let (changeset_id, revision, diagnostic) = match crate::version_service::record_written(
        store,
        &target_path,
        "locale_overlay",
        None,
        None,
        None,
        Some(file_digest),
        change_items,
    ) {
        Ok(summary) => (Some(summary.id), Some(summary.revision), None),
        Err(error) => (None, None, Some(error.to_string())),
    };
    Ok(WriteLocaleOverlayResponse {
        output_path: target_path,
        entry_count: entries.len(),
        bytes_written,
        merged_kept,
        changeset_id,
        revision,
        diagnostic,
    })
}

#[tauri::command]
pub async fn write_locale_overlay(
    state: State<'_, AppState>,
    request: WriteLocaleOverlayRequest,
) -> Result<WriteLocaleOverlayResponse, CommandError> {
    if request.edits.is_empty() {
        return Err(CommandError::new("invalid_argument", "no edits to write"));
    }
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || write_locale_overlay_inner(&store, &request))
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
        std::fs::write(&out, write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let package = dbpf::Package::open(&out).unwrap();
        let entry = package
            .entry(ResourceId { type_id: LOCALE_RESOURCE_TYPE, group: 0x9E14D920, instance: 0x00000001 })
            .unwrap();
        let read_back = package.read(entry).unwrap();
        assert_eq!(parse_locale_items(&read_back).unwrap(), items);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn edit(instance: u32, text: &str) -> LocaleTableEdit {
        LocaleTableEdit {
            tgi: LocaleTgi {
                type_id: LOCALE_RESOURCE_TYPE,
                group: 0x9E14_D920,
                instance,
            },
            items: vec![LocaleItem {
                key: "0x00000001".into(),
                id: Some(1),
                text: text.into(),
            }],
        }
    }

    fn request(path: &std::path::Path, edits: Vec<LocaleTableEdit>) -> WriteLocaleOverlayRequest {
        WriteLocaleOverlayRequest {
            edits,
            output_path: path.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn merge_keeps_untouched_entries_and_records_versions() {
        let dir = std::env::temp_dir().join(format!("openscp-locale-merge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = sc_store::Store::open(dir.join("openscp.db")).unwrap();
        let out = dir.join("overlay.package");
        let untouched = ResourceId {
            type_id: LOCALE_RESOURCE_TYPE,
            group: 0x9E14_D920,
            instance: 0x99,
        };
        // 目标里预先存在一条与本次编辑无关的资源。
        std::fs::write(
            &out,
            write_uncompressed_overlay(&[OverlayEntry::new(untouched, b"keep-me".to_vec())]).unwrap(),
        )
        .unwrap();

        let first =
            write_locale_overlay_inner(&store, &request(&out, vec![edit(0x01, "第一次")])).unwrap();
        assert_eq!(first.revision, Some(1));
        assert_eq!(first.entry_count, 2, "合并保留 = 目标既有条目 + 本次编辑");
        assert_eq!(first.merged_kept, 1);
        assert!(first.diagnostic.is_none());

        let second =
            write_locale_overlay_inner(&store, &request(&out, vec![edit(0x01, "第二次")])).unwrap();
        assert_eq!(second.revision, Some(2));
        assert_eq!(second.merged_kept, 1);

        // 目标文件里那条未被编辑的资源必须原样还在。
        let package = dbpf::Package::open(&out).unwrap();
        let entry = package.entry(untouched).expect("未被编辑的资源必须仍在");
        assert_eq!(package.read(entry).unwrap(), b"keep-me");

        // 第二条版本记录里，0x01 的 before 应等于第一次保存的字节。
        let (_, items) = store.changeset_detail(second.changeset_id.unwrap()).unwrap();
        assert_eq!(items.len(), 1);
        let before = store
            .load_blob(items[0].before_digest.as_ref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(parse_locale_items(&before).unwrap()[0].text, "第一次");
        let after = store
            .load_blob(items[0].after_digest.as_ref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(parse_locale_items(&after).unwrap()[0].text, "第二次");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capture_baseline_then_save_keeps_version_line_total() {
        let dir = std::env::temp_dir().join(format!("openscp-locale-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = sc_store::Store::open(dir.join("openscp.db")).unwrap();
        let out = dir.join("overlay.package");
        let tgi = ResourceId {
            type_id: LOCALE_RESOURCE_TYPE,
            group: 0x9E14_D920,
            instance: 0x99,
        };
        std::fs::write(
            &out,
            write_uncompressed_overlay(&[OverlayEntry::new(tgi, b"legacy".to_vec())]).unwrap(),
        )
        .unwrap();

        // 基线捕获把既有条目登记成版本 1（before = None）。
        let entries = crate::version_service::read_target_entries(&out);
        assert_eq!(entries.len(), 1);
        let summary = crate::version_service::record_written(
            &store,
            &out.to_string_lossy(),
            "baseline",
            None,
            None,
            None,
            None,
            entries
                .into_iter()
                .map(|(id, data)| ChangeItemInput {
                    type_id: id.type_id,
                    group_id: id.group,
                    instance: id.instance,
                    before: None,
                    after: Some(data),
                })
                .collect(),
        )
        .unwrap();
        assert_eq!(summary.revision, 1);

        // 之后的保存接在同一条版本线上。
        let saved =
            write_locale_overlay_inner(&store, &request(&out, vec![edit(0x01, "新增")])).unwrap();
        assert_eq!(saved.revision, Some(2));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
