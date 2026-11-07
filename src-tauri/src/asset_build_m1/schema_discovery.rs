//! 项目内资产 schema 自动发现（2026-11-07）。
//!
//! 目标：构建不再要求手工粘贴 schema。项目里已有的 M1 资产 schema
//! （`$schema: "openscp.asset"`）就是构建输入；本模块只做"找到它"。
//!
//! 选择规则（确定性、fail-closed）：
//! 1. 扫描 `schemas/` 下全部 JSON，只收 `openscp.asset` + `schema_version: 2`；
//! 2. 优先保留 `scene.document` 在项目内真实存在的候选；
//! 3. 恰好一个 → 返回；多个 → 拒绝并列出候选（不猜）；零个 → 拒绝并给指引。

use serde_json::Value;
use std::path::{Path, PathBuf};

/// 一个候选 schema（相对路径 + 原始文本 + scene.document）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaCandidate {
    pub path: String,
    pub raw: String,
    pub document: String,
}

/// 从候选里选唯一构建输入（纯函数，便于单测）。
pub fn select(
    candidates: Vec<SchemaCandidate>,
    document_exists: &dyn Fn(&str) -> bool,
) -> Result<SchemaCandidate, String> {
    if candidates.is_empty() {
        return Err(
            "no asset schema found: add a JSON file under schemas/ with \
             \"$schema\": \"openscp.asset\" and schema_version 2"
                .into(),
        );
    }
    let mut existing: Vec<SchemaCandidate> = candidates
        .iter()
        .filter(|candidate| document_exists(&candidate.document))
        .cloned()
        .collect();
    if existing.is_empty() {
        existing = candidates;
    }
    if existing.len() == 1 {
        return Ok(existing.remove(0));
    }
    let mut paths: Vec<&str> = existing.iter().map(|candidate| candidate.path.as_str()).collect();
    paths.sort_unstable();
    Err(format!(
        "multiple asset schemas found ({}); keep exactly one or pass the schema explicitly: {}",
        paths.len(),
        paths.join(", ")
    ))
}

/// 扫描项目并返回唯一资产 schema。
pub fn discover(root: &Path) -> Result<SchemaCandidate, String> {
    let schemas = root.join("schemas");
    let base = if schemas.is_dir() { schemas } else { root.to_path_buf() };
    let mut candidates = Vec::new();
    walk(&base, root, 0, &mut candidates)?;
    select(candidates, &|document| root.join(document).is_file())
}

fn walk(
    dir: &Path,
    root: &Path,
    depth: usize,
    out: &mut Vec<SchemaCandidate>,
) -> Result<(), String> {
    if depth > 6 || out.len() >= 256 {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(&path, root, depth + 1, out)?;
            continue;
        }
        if !kind.is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("json")
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if value.get("$schema").and_then(Value::as_str) != Some("openscp.asset")
            || value.get("schema_version").and_then(Value::as_u64) != Some(2)
        {
            continue;
        }
        let Some(document) = value
            .pointer("/scene/document")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(SchemaCandidate {
            path: relative,
            raw: String::from_utf8_lossy(&bytes).into_owned(),
            document,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(path: &str, document: &str) -> SchemaCandidate {
        SchemaCandidate {
            path: path.into(),
            raw: "{}".into(),
            document: document.into(),
        }
    }

    #[test]
    fn selects_the_only_schema() {
        let picked = select(
            vec![candidate("schemas/assets/m1.json", "asset.lot.json")],
            &|_| true,
        )
        .unwrap();
        assert_eq!(picked.path, "schemas/assets/m1.json");
    }

    #[test]
    fn prefers_candidates_with_an_existing_document() {
        let picked = select(
            vec![
                candidate("schemas/old.json", "missing.lot.json"),
                candidate("schemas/m1.json", "asset.lot.json"),
            ],
            &|document| document == "asset.lot.json",
        )
        .unwrap();
        assert_eq!(picked.path, "schemas/m1.json");
    }

    #[test]
    fn refuses_to_guess_between_two_valid_schemas() {
        let error = select(
            vec![
                candidate("schemas/a.json", "asset.lot.json"),
                candidate("schemas/b.json", "asset.lot.json"),
            ],
            &|_| true,
        )
        .unwrap_err();
        assert!(error.contains("multiple asset schemas"), "{error}");
        assert!(error.contains("schemas/a.json"), "{error}");
    }

    #[test]
    fn empty_project_names_the_expected_shape() {
        let error = select(vec![], &|_| true).unwrap_err();
        assert!(error.contains("openscp.asset"), "{error}");
    }
}
