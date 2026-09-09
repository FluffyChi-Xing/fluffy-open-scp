//! TGI 复写检测（Mod Studio WP0）。
//!
//! 按给定 roots 顺序（游戏目录在前、mod 目录在后）递归枚举 .package，
//! 仅读 DBPF 索引（mmap），把全部资源按 TGI 分组：同一 TGI 出现在多个包
//! 即为"被复写"，后加载的包覆盖先前的。加载顺序近似为 roots 顺序 +
//! 目录内字母序（游戏实际按字母序加载，见 docs/roadmap/workspace-panels.md §5）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dbpf::Package;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::activity::CommandError;
use crate::stats::KNOWN_EXTENSIONS;

const MAX_DEPTH: usize = 4;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideScanRequest {
    /// 按加载顺序排列的根目录（先游戏 SimCityData，后 mod 目录）。
    pub roots: Vec<String>,
}

/// 覆盖链中的一环：哪个包、包内多大。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideChainItem {
    pub path: String,
    pub name: String,
    pub decompressed_size: u64,
    /// 是否为该 TGI 的最终生效版本（链尾）。
    pub wins: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideConflict {
    pub type_id: u32,
    pub group_id: u32,
    pub instance_id: u32,
    pub ext: String,
    pub chain: Vec<OverrideChainItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverridePackageStat {
    pub path: String,
    pub name: String,
    pub entry_count: u64,
    pub overrides: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideScanStats {
    pub packages: u64,
    pub entries: u64,
    pub duplicated_tgis: u64,
    pub overridden_entries: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideScanResponse {
    pub conflicts: Vec<OverrideConflict>,
    pub packages: Vec<OverridePackageStat>,
    pub stats: OverrideScanStats,
    pub failed: Vec<String>,
}

fn ext_label(type_id: u32) -> String {
    KNOWN_EXTENSIONS
        .iter()
        .find(|(id, _, _)| *id == type_id)
        .map(|(_, ext, _)| (*ext).to_string())
        .unwrap_or_else(|| format!("{type_id:08X}"))
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// 递归枚举 root 下的 .package（字母序，深度 ≤ MAX_DEPTH），追加到 out。
fn collect_packages(root: &str, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let mut entries: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, depth + 1, out)?;
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("package"))
            {
                out.push(path);
            }
        }
        Ok(())
    }
    let root_path = PathBuf::from(root);
    if !root_path.is_dir() {
        return Ok(());
    }
    walk(&root_path, 0, out)
}

pub fn scan_overrides(
    roots: &[String],
) -> Result<OverrideScanResponse, CommandError> {
    // (TGI) -> 覆盖链（按加载顺序）
    #[derive(Default)]
    struct Slot {
        chain: Vec<(usize, u64)>, // (package index, decompressed size)
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut failed = Vec::new();
    for root in roots {
        let root_path = PathBuf::from(root);
        if !root_path.is_dir() {
            failed.push(format!("{root}: not a directory"));
            continue;
        }
        let mut found = Vec::new();
        match collect_packages(root, &mut found) {
            Ok(()) => paths.extend(found),
            Err(error) => failed.push(format!("{root}: {error}")),
        }
    }

    let mut slots: HashMap<(u32, u32, u32), Slot> = HashMap::new();
    let mut package_stats = Vec::with_capacity(paths.len());
    let mut total_entries = 0u64;

    for (index, path) in paths.iter().enumerate() {
        let package = match Package::open(path) {
            Ok(package) => package,
            Err(_) => {
                failed.push(path.to_string_lossy().into_owned());
                continue;
            }
        };
        let mut entry_count = 0u64;
        for entry in package.entries() {
            entry_count += 1;
            total_entries += 1;
            let key = (
                entry.id.type_id,
                entry.id.group,
                entry.id.instance,
            );
            slots
                .entry(key)
                .or_default()
                .chain
                .push((index, u64::from(entry.decompressed_size)));
        }
        package_stats.push(OverridePackageStat {
            path: path.to_string_lossy().into_owned(),
            name: file_name_of(path),
            entry_count,
            overrides: 0,
        });
    }

    let duplicated = slots.values().filter(|slot| slot.chain.len() > 1).count() as u64;
    // 链上除最终生效者外的每个包记一次 override（"覆盖了先前的版本"）
    let mut overrides_by_package = vec![0u64; package_stats.len()];
    let mut overridden_entries = 0u64;
    let mut conflicts: Vec<OverrideConflict> = slots
        .into_iter()
        .filter(|(_, slot)| slot.chain.len() > 1)
        .map(|((type_id, group_id, instance_id), slot)| {
            overridden_entries += slot.chain.len() as u64 - 1;
            for &(index, _) in &slot.chain[..slot.chain.len() - 1] {
                overrides_by_package[index] += 1;
            }
            let chain: Vec<OverrideChainItem> = slot
                .chain
                .iter()
                .enumerate()
                .map(|(position, &(index, size))| OverrideChainItem {
                    path: package_stats[index].path.clone(),
                    name: package_stats[index].name.clone(),
                    decompressed_size: size,
                    wins: position == slot.chain.len() - 1,
                })
                .collect();
            OverrideConflict {
                type_id,
                group_id,
                instance_id,
                ext: ext_label(type_id),
                chain,
            }
        })
        .collect();
    conflicts.sort_by(|a, b| b.chain.len().cmp(&a.chain.len()).then(a.ext.cmp(&b.ext)));
    for (stat, count) in package_stats.iter_mut().zip(overrides_by_package) {
        stat.overrides = count;
    }
    package_stats.sort_by(|a, b| b.overrides.cmp(&a.overrides).then(a.name.cmp(&b.name)));

    Ok(OverrideScanResponse {
        stats: OverrideScanStats {
            packages: package_stats.len() as u64,
            entries: total_entries,
            duplicated_tgis: duplicated,
            overridden_entries,
        },
        conflicts,
        packages: package_stats,
        failed,
    })
}

#[tauri::command]
pub async fn override_scan(
    _app: AppHandle,
    request: OverrideScanRequest,
) -> Result<OverrideScanResponse, CommandError> {
    tauri::async_runtime::spawn_blocking(move || scan_overrides(&request.roots))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_label_uses_known_extensions() {
        assert_eq!(ext_label(0x00B1_B104), "property");
        assert_eq!(ext_label(0x2F4E_681B), "rw4");
        assert_eq!(ext_label(0xDEAD_BEEF), "DEADBEEF");
    }

    #[test]
    fn collect_packages_walks_sorted_and_depth_limited() {
        let dir = std::env::temp_dir().join("openscp_override_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("b/nested")).unwrap();
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("z.package"), b"").unwrap();
        std::fs::write(dir.join("a/mid.package"), b"").unwrap();
        std::fs::write(dir.join("b/nested/deep.package"), b"").unwrap();
        std::fs::write(dir.join("note.txt"), b"").unwrap();
        let mut found = Vec::new();
        collect_packages(dir.to_str().unwrap(), &mut found).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["mid.package", "deep.package", "z.package"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_reports_failed_roots() {
        let response = scan_overrides(&["Z:/definitely/not/here".into()]).unwrap();
        assert!(response.packages.is_empty());
        assert_eq!(response.failed.len(), 1);
        assert_eq!(response.stats.packages, 0);
    }
}
