//! Package 扩展名统计（首页仪表盘）与可识别覆盖率。
//!
//! 仅解析各 package 的 DBPF 索引（mmap，不触发解压），按 typeId 聚类：
//! - 可识别：typeId 命中 `database_main.s3db` FileTypes 表（名称非空）；
//! - 扩展名：内置 25 个已知类型的官方后缀映射，其余用注册表名称回退。
//!
//! 覆盖率口径：decompressed（逻辑内容大小）为主，stored（磁盘占用）同步返回。

use std::collections::HashMap;
use std::path::Path;

use dbpf::Package;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::activity::CommandError;

/// SimCityPak 官方 FileTypes 表（database_main.s3db，25 条）的后缀映射。
/// key = typeId，value =（后缀，展示名）。
const KNOWN_EXTENSIONS: &[(u32, &str, &str)] = &[
    (0xDD62_33D6, "html", "HTML File"),
    (0xEA51_18B0, "effectdir", "Effects Directory File"),
    (0x00B1_B104, "property", "Property File"),
    (0x0239_3756, "cur", "Cursor File"),
    (0x024A_0E52, "unknown-prop", "Uknown File (Property/Spore?)"),
    (0x02FA_C0B6, "unknown", "Unknown File"),
    (0x03E4_21EC, "grey8", "Greyscale Map (8-bit)"),
    (0x03E4_21ED, "grey32", "Greyscale Map (32-bit)"),
    (0x03E4_21F0, "grey16", "Greyscale Map (16-bit)"),
    (0x0469_A3F7, "cpp", "C++ File"),
    (0x0806_8AEB, "er2b", "ER2 Binary Rule File"),
    (0x0806_8AEC, "er2", "ER2 Rule File"),
    (0x0A4D_8D09, "bkhd", "BKHD File"),
    (0x0A98_EAF0, "json", "JSON File"),
    (0x0D9E_5710, "wav", "Wav Audio File"),
    (0x276C_A4B9, "ttf", "TrueType Font File"),
    (0x2C97_8DB6, "css", "Cascading Style Sheet File"),
    (0x2F4E_681B, "rw4", "RW4 File"),
    (0x2F4E_681C, "raster", "Raster File"),
    (0x2F7D_0004, "png", "PNG File"),
    (0x2F7D_0006, "tga", "TGA File"),
    (0x2F7D_0007, "gif", "GIF File"),
    (0x3768_40D7, "vp6", "EA VP60 Video File"),
    (0x3F86_62EA, "jpg", "JPG File"),
    (0x6777_1F5C, "js", "Javascript File"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageStatisticsRequest {
    pub paths: Vec<String>,
}

/// 单个 typeId 的聚类结果（已知 = 扩展名，未知 = 0x 十六进制 TGI）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
    pub ext: String,
    pub type_id: u32,
    pub known: bool,
    pub count: u64,
    pub stored_size: u64,
    pub decompressed_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageStat {
    pub path: String,
    pub name: String,
    pub file_size: u64,
    pub entry_count: u64,
    pub extensions: Vec<ExtensionStat>,
    pub known_decompressed: u64,
    pub unknown_decompressed: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalsStat {
    pub file_size: u64,
    pub stored_size: u64,
    pub decompressed_size: u64,
    pub known_decompressed: u64,
    pub unknown_decompressed: u64,
    pub known_count: u64,
    pub unknown_count: u64,
    pub entry_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageStatisticsResponse {
    pub packages: Vec<PackageStat>,
    pub extensions: Vec<ExtensionStat>,
    pub totals: TotalsStat,
    /// 无法打开（文件被移动/删除等）的路径。
    pub failed: Vec<String>,
}

fn extension_label(
    type_id: u32,
    registry: Option<&sc_registry::Registry>,
) -> (String, bool) {
    if let Some((_, ext, _)) =
        KNOWN_EXTENSIONS.iter().find(|(id, _, _)| *id == type_id)
    {
        return ((*ext).to_string(), true);
    }
    if let Some(registry) = registry
        && let Some(record) = registry.file_types().get(&type_id)
        && !record.name.is_empty()
    {
        return (record.name.clone(), true);
    }
    (format!("{type_id:08X}"), false)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn package_statistics(
    app: AppHandle,
    request: PackageStatisticsRequest,
) -> Result<PackageStatisticsResponse, CommandError> {
    let registry_path = crate::package_service::bundled_registry_path(&app);
    tauri::async_runtime::spawn_blocking(move || {
        analyze_packages(&request.paths, registry_path.as_deref())
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn analyze_packages(
    paths: &[String],
    registry_path: Option<&Path>,
) -> Result<PackageStatisticsResponse, CommandError> {
    let registry = registry_path
        .and_then(|path| sc_registry::Registry::open(path).ok());
    let mut packages = Vec::with_capacity(paths.len());
    let mut failed = Vec::new();
    // (ext, type_id, known) -> 聚合计数
    let mut aggregate: HashMap<(String, u32, bool), [u64; 3]> = HashMap::new();
    let mut totals = TotalsStat {
        file_size: 0,
        stored_size: 0,
        decompressed_size: 0,
        known_decompressed: 0,
        unknown_decompressed: 0,
        known_count: 0,
        unknown_count: 0,
        entry_count: 0,
    };

    for path in paths {
        let package = match Package::open(path) {
            Ok(package) => package,
            Err(_) => {
                failed.push(path.clone());
                continue;
            }
        };
        let file_size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
        let mut package_exts: HashMap<(String, u32, bool), [u64; 3]> =
            HashMap::new();
        let mut package_known = 0u64;
        let mut package_unknown = 0u64;
        let mut entry_count = 0u64;
        for entry in package.entries() {
            entry_count += 1;
            let (ext, known) = extension_label(entry.id.type_id, registry.as_ref());
            let stored = entry.stored_len();
            let decompressed = u64::from(entry.decompressed_size);
            let key = (ext, entry.id.type_id, known);
            let slot = package_exts.entry(key.clone()).or_insert([0; 3]);
            slot[0] += 1;
            slot[1] += stored;
            slot[2] += decompressed;
            let agg = aggregate.entry(key).or_insert([0; 3]);
            agg[0] += 1;
            agg[1] += stored;
            agg[2] += decompressed;
            if known {
                package_known += decompressed;
                totals.known_decompressed += decompressed;
                totals.known_count += 1;
            } else {
                package_unknown += decompressed;
                totals.unknown_decompressed += decompressed;
                totals.unknown_count += 1;
            }
            totals.stored_size += stored;
            totals.decompressed_size += decompressed;
        }
        packages.push(PackageStat {
            path: path.clone(),
            name: file_name_of(Path::new(path)),
            file_size,
            entry_count,
            extensions: finalize_stats(package_exts),
            known_decompressed: package_known,
            unknown_decompressed: package_unknown,
        });
        totals.file_size += file_size;
        totals.entry_count += entry_count;
    }

    Ok(PackageStatisticsResponse {
        extensions: finalize_stats(aggregate),
        packages,
        totals,
        failed,
    })
}

fn finalize_stats(
    map: HashMap<(String, u32, bool), [u64; 3]>,
) -> Vec<ExtensionStat> {
    let mut stats: Vec<ExtensionStat> = map
        .into_iter()
        .map(|((ext, type_id, known), [count, stored, decompressed])| {
            ExtensionStat {
                ext,
                type_id,
                known,
                count,
                stored_size: stored,
                decompressed_size: decompressed,
            }
        })
        .collect();
    stats.sort_by(|a, b| b.decompressed_size.cmp(&a.decompressed_size));
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_label_maps_known_types() {
        assert_eq!(
            extension_label(0x00B1_B104, None),
            ("property".to_string(), true)
        );
        assert_eq!(
            extension_label(0x2C97_8DB6, None),
            ("css".to_string(), true)
        );
        let (ext, known) = extension_label(0xDEAD_BEEF, None);
        assert!(!known);
        assert_eq!(ext, "DEADBEEF");
    }

    #[test]
    fn known_extension_table_ids_are_unique() {
        let mut ids: Vec<u32> = KNOWN_EXTENSIONS.iter().map(|(id, _, _)| *id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), KNOWN_EXTENSIONS.len());
    }

    /// 手动探针：`OPENSCP_PROBE_DIRS`（分号分隔目录）下全部 .package 的
    /// 可识别容量覆盖率。运行：
    /// `cargo test -p fluffy-open-scp probe_coverage -- --ignored --nocapture`
    #[test]
    #[ignore = "manual probe: needs game data on disk"]
    fn probe_coverage() {
        let Some(dirs) = std::env::var_os("OPENSCP_PROBE_DIRS") else {
            return;
        };
        let registry_path = std::env::current_dir()
            .ok()
            .map(|dir| dir.join("resources/database_main.s3db"))
            .filter(|path| path.is_file());
        let mut paths = Vec::new();
        for dir in dirs.to_string_lossy().split(';') {
            let dir = dir.trim();
            if dir.is_empty() {
                continue;
            }
            let mut found: Vec<_> = std::fs::read_dir(dir)
                .expect("read probe dir")
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("package"))
                })
                .collect();
            found.sort();
            for path in found {
                paths.push(path.to_string_lossy().into_owned());
            }
        }
        let response = analyze_packages(&paths, registry_path.as_deref()).unwrap();
        println!("packages analyzed: {}", response.packages.len());
        println!(
            "failed: {:?}",
            response.failed.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let totals = &response.totals;
        let ratio = if totals.decompressed_size > 0 {
            totals.known_decompressed as f64 / totals.decompressed_size as f64
        } else {
            0.0
        };
        println!("=== coverage probe (decompressed size) ===");
        println!("file size:            {}", totals.file_size);
        println!("total decompressed:   {}", totals.decompressed_size);
        println!("known decompressed:   {} ({:.2}%)", totals.known_decompressed, ratio * 100.0);
        println!("unknown decompressed: {} ({:.2}%)", totals.unknown_decompressed, (1.0 - ratio) * 100.0);
        println!("entries: {} (known {} / unknown {})", totals.entry_count, totals.known_count, totals.unknown_count);
        println!("=== per-extension (top 30 by decompressed) ===");
        for ext in response.extensions.iter().take(30) {
            println!(
                "{:>10}  {:>8} files  {:>14} bytes  known={}",
                ext.ext, ext.count, ext.decompressed_size, ext.known
            );
        }
    }
}
