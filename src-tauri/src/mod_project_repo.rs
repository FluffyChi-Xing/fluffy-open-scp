//! 项目详情页（GitHub-repo 形态）只读摘要：Releases / Languages / package.json 元数据。
//!
//! 与 `code_tree` 解耦：语言统计跳过 `.build` 等产物目录，避免构建产物淹没占比；
//! Releases 只扫描 `.build/<hash>/packages/*.package`。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::workspace::{code_root, CodeProjectRequest};

const LANG_SCAN_MAX_FILES: usize = 20_000;
const LANG_SCAN_MAX_DEPTH: usize = 16;

/// 语言统计跳过的目录名（小写比较）。
const SKIP_DIRS: &[&str] = &[".build", ".git", "node_modules", "target", "dist"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectRelease {
    pub name: String,
    pub relative_path: String,
    pub build_hash: String,
    pub size: u64,
    pub modified_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectLanguageStat {
    pub extension: String,
    pub file_count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectRepoSummary {
    pub author: String,
    pub version: String,
    pub mod_type: String,
    pub description: String,
    pub releases: Vec<ModProjectRelease>,
    pub languages: Vec<ModProjectLanguageStat>,
    pub language_truncated: bool,
    pub total_source_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct PackageManifestLite {
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    mod_type: Option<String>,
    #[serde(default)]
    #[serde(rename = "modType")]
    mod_type_camel: Option<String>,
}

fn modified_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn extension_of(name: &str) -> String {
    let Some(dot) = name.rfind('.') else {
        return String::new();
    };
    if dot == 0 || dot + 1 >= name.len() {
        return String::new();
    }
    name[dot + 1..].to_ascii_lowercase()
}

fn should_skip_dir(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SKIP_DIRS.iter().any(|skip| *skip == lower.as_str())
}

fn scan_languages(
    root: &Path,
    current: &Path,
    relative: &str,
    depth: usize,
    budget: &mut usize,
    truncated: &mut bool,
    buckets: &mut BTreeMap<String, (usize, u64)>,
) {
    if *budget == 0 || depth > LANG_SCAN_MAX_DEPTH {
        *truncated = true;
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            *truncated = true;
            break;
        }
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if metadata.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            let child_relative = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            scan_languages(
                root,
                &path,
                &child_relative,
                depth + 1,
                budget,
                truncated,
                buckets,
            );
        } else if metadata.is_file() {
            *budget = budget.saturating_sub(1);
            let ext = extension_of(&name);
            let key = if ext.is_empty() {
                "(none)".to_string()
            } else {
                ext
            };
            let size = metadata.len();
            let slot = buckets.entry(key).or_insert((0, 0));
            slot.0 += 1;
            slot.1 += size;
        }
    }
}

fn scan_releases(root: &Path) -> Vec<ModProjectRelease> {
    let build_root = root.join(".build");
    let Ok(hashes) = fs::read_dir(&build_root) else {
        return Vec::new();
    };
    let mut releases = Vec::new();
    for hash_entry in hashes.flatten() {
        let Ok(meta) = fs::symlink_metadata(hash_entry.path()) else {
            continue;
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            continue;
        }
        let build_hash = hash_entry.file_name().to_string_lossy().into_owned();
        // 跳过非构建哈希目录（queue / ledger / 配置文件旁路）
        if matches!(
            build_hash.as_str(),
            "queue" | "ledger" | "pool-last.json" | "material-baseline.json"
        ) || build_hash.ends_with(".json")
        {
            continue;
        }
        let packages_dir = hash_entry.path().join("packages");
        let Ok(packages) = fs::read_dir(&packages_dir) else {
            continue;
        };
        for package_entry in packages.flatten() {
            let path = package_entry.path();
            let Ok(package_meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !package_meta.is_file() {
                continue;
            }
            let name = package_entry.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".package") {
                continue;
            }
            let relative_path = format!(".build/{build_hash}/packages/{name}");
            releases.push(ModProjectRelease {
                name,
                relative_path,
                build_hash: build_hash.clone(),
                size: package_meta.len(),
                modified_ms: modified_ms(&path),
            });
        }
    }
    releases.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then(a.name.cmp(&b.name)));
    releases
}

fn read_manifest(root: &Path) -> PackageManifestLite {
    let path = root.join("package.json");
    let Ok(bytes) = fs::read(&path) else {
        return PackageManifestLite {
            author: String::new(),
            version: String::new(),
            description: String::new(),
            mod_type: None,
            mod_type_camel: None,
        };
    };
    serde_json::from_slice(&bytes).unwrap_or(PackageManifestLite {
        author: String::new(),
        version: String::new(),
        description: String::new(),
        mod_type: None,
        mod_type_camel: None,
    })
}

fn summarize_project(root: &Path) -> ModProjectRepoSummary {
    let manifest = read_manifest(root);
    let mut buckets = BTreeMap::new();
    let mut budget = LANG_SCAN_MAX_FILES;
    let mut truncated = false;
    scan_languages(root, root, "", 0, &mut budget, &mut truncated, &mut buckets);

    let mut languages: Vec<ModProjectLanguageStat> = buckets
        .into_iter()
        .map(|(extension, (file_count, bytes))| ModProjectLanguageStat {
            extension,
            file_count,
            bytes,
        })
        .collect();
    languages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.extension.cmp(&b.extension)));
    let total_source_bytes = languages.iter().map(|entry| entry.bytes).sum();

    ModProjectRepoSummary {
        author: manifest.author,
        version: manifest.version,
        mod_type: manifest
            .mod_type
            .or(manifest.mod_type_camel)
            .unwrap_or_default(),
        description: manifest.description,
        releases: scan_releases(root),
        languages,
        language_truncated: truncated,
        total_source_bytes,
    }
}

#[tauri::command]
pub async fn mod_project_repo_summary(
    state: State<'_, AppState>,
    request: CodeProjectRequest,
) -> Result<ModProjectRepoSummary, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = code_root(&store, &request.project).map_err(CommandError::from)?;
        Ok(summarize_project(&root))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    fn write_file(path: &Path, body: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn summary_skips_build_for_languages_and_lists_releases() {
        let root = std::env::temp_dir().join(format!(
            "openscp-repo-summary-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_file(
            &root.join("package.json"),
            br#"{"author":"Ada","version":"1.2.3","description":"demo","mod_type":"assets"}"#,
        );
        write_file(&root.join("src/main.ts"), b" console.log(1)");
        write_file(&root.join("src/view.vue"), b"<template></template>");
        write_file(
            &root.join(".build/abc123/packages/demo.package"),
            b"pkg",
        );
        write_file(&root.join(".build/abc123/packages/demo.package"), b"pkg!!");
        // 产物不应进入语言统计
        write_file(&root.join(".build/noise.bin"), b"xxxxxxxx");

        let summary = summarize_project(&root);
        assert_eq!(summary.author, "Ada");
        assert_eq!(summary.version, "1.2.3");
        assert_eq!(summary.mod_type, "assets");
        assert_eq!(summary.releases.len(), 1);
        assert_eq!(summary.releases[0].name, "demo.package");
        assert!(summary
            .languages
            .iter()
            .any(|entry| entry.extension == "ts"));
        assert!(summary
            .languages
            .iter()
            .any(|entry| entry.extension == "vue"));
        assert!(!summary
            .languages
            .iter()
            .any(|entry| entry.extension == "bin" || entry.extension == "package"));

        let _ = fs::remove_dir_all(&root);
    }
}
