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
use std::fs;
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
    // Same workbench document → deterministic pick (highest .sN, else stable path).
    // Different documents still refuse so we never guess across assets.
    let same_document = existing
        .windows(2)
        .all(|pair| pair[0].document == pair[1].document);
    if same_document {
        return Ok(pick_preferred_iteration(existing));
    }
    let mut paths: Vec<&str> = existing
        .iter()
        .map(|candidate| candidate.path.as_str())
        .collect();
    paths.sort_unstable();
    Err(format!(
        "multiple asset schemas found ({}); keep exactly one or pass the schema explicitly: {}",
        paths.len(),
        paths.join(", ")
    ))
}

/// Prefer `m1.s23.json` over `m1.json` when both target the same document.
fn pick_preferred_iteration(mut candidates: Vec<SchemaCandidate>) -> SchemaCandidate {
    candidates.sort_by(|left, right| {
        iteration_rank(&right.path)
            .cmp(&iteration_rank(&left.path))
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates.remove(0)
}

fn iteration_rank(path: &str) -> u32 {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_suffix(".json").unwrap_or(file);
    stem.rsplit_once(".s")
        .and_then(|(_, digits)| digits.parse::<u32>().ok())
        .unwrap_or(0)
}

/// 扫描项目并返回唯一资产 schema。
pub fn discover(root: &Path) -> Result<SchemaCandidate, String> {
    let schemas = root.join("schemas");
    let base = if schemas.is_dir() {
        schemas
    } else {
        root.to_path_buf()
    };
    let mut candidates = Vec::new();
    walk(&base, root, 0, &mut candidates)?;
    select(candidates, &|document| root.join(document).is_file())
}

/// Build an M1 schema from an Asset Editor workbench document when the project
/// has not yet materialized an `openscp.asset` schema.  This is deliberately a
/// narrow adapter: the editor document remains the source of menu/scene/
/// behavior facts, while game-side donor semantics are inferred from the
/// referenced package rather than invented in the UI.
pub fn discover_or_adapt(root: &Path) -> Result<SchemaCandidate, String> {
    match discover(root) {
        Ok(candidate) => Ok(candidate),
        // Adapter is only for projects that never materialized an openscp.asset
        // schema. Multiple-schema ambiguity must stay fail-closed — do not hide
        // it behind a second adapter error about sourceLot.packagePath.
        Err(discovery_error)
            if discovery_error.starts_with("no asset schema found")
                && root.join("asset.lot.json").is_file() =>
        {
            adapt_asset_editor(root)
                .map_err(|error| format!("Asset Editor (asset.lot.json): {error}"))
        }
        Err(error) => Err(error),
    }
}

fn adapt_asset_editor(root: &Path) -> Result<SchemaCandidate, String> {
    let document_path = root.join("asset.lot.json");
    let document: Value = serde_json::from_slice(
        &fs::read(&document_path).map_err(|e| format!("asset.lot.json: {e}"))?,
    )
    .map_err(|e| format!("asset.lot.json: {e}"))?;
    let workbench = document
        .pointer("/editor/workbench")
        .ok_or("missing editor.workbench")?;
    let model = workbench
        .get("assetPath")
        .or_else(|| workbench.get("nativeLotm"))
        .and_then(Value::as_str)
        .ok_or("workbench.assetPath/nativeLotm is required")?;
    if !root.join(model).is_file() {
        return Err(format!("workbench model is missing: {model}"));
    }
    // 自绘地面且未绑 donor（authored + sourceLot null）：地面不再需要
    // donor——构建器从项目内合成（native_lot 契约，scene.authoredGround）；
    // 参考包只服务菜单/行为骨架。绑了 donor 的文档维持克隆工作流。
    let authored = workbench.pointer("/lotStyle/authored").and_then(Value::as_bool);
    let authored_only = authored == Some(true)
        && workbench
            .pointer("/lotStyle/sourceLot")
            .map(Value::is_null)
            .unwrap_or(true);
    let source = workbench.pointer("/lotStyle/sourceLot").filter(|s| !s.is_null());
    let (reference_lot, schema_id) = match source {
        Some(source) => {
            let tgi = source.get("tgi").ok_or("sourceLot.tgi is required")?;
            let type_id = json_u32(tgi.get("typeId"), "sourceLot.tgi.typeId")?;
            let group = json_u32(tgi.get("group"), "sourceLot.tgi.group")?;
            let instance = json_u32(tgi.get("instance"), "sourceLot.tgi.instance")?;
            let package_source = source
                .get("packagePath")
                .and_then(Value::as_str)
                .unwrap_or("");
            let source_path = resolve_source_package(package_source).or_else(|primary| {
                // Workbench lots often record packageId + TGI without a filesystem
                // path; fall back to the offline game DLC0 package used by resolve.
                if package_source.is_empty() {
                    resolve_source_package(
                        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_DLC0.package",
                    )
                    .map_err(|fallback| format!("{primary}; fallback DLC0: {fallback}"))
                } else {
                    Err(primary)
                }
            })?;
            let donor = select_lot_config(&source_path, type_id, group, instance)?;
            let reference_package = root.join("build/reference.package");
            super::authored_reference::install(&source_path, &reference_package, true)
                .map_err(|error| format!(
                    "prepare reference {} -> {}: {error}",
                    source_path.display(), reference_package.display()
                ))?;
            (
                Some(format!("{:08X}:{:08X}:{:08X}", donor.0, donor.1, donor.2)),
                format!("asset-editor-{instance:08X}"),
            )
        }
        None if authored_only => {
            super::authored_reference::prepare(root)?;
            (None, "asset-editor-authored".to_string())
        }
        None => return Err("missing workbench.lotStyle.sourceLot".into()),
    };
    let output_name = workbench
        .pointer("/menu/label")
        .and_then(Value::as_str)
        .map(|label| sanitize_name(label))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "asset".into());
    let writes = workbench
        .get("writes")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let scene = if authored_only {
        serde_json::json!({
            "document": "asset.lot.json", "model": model,
            "writes": writes, "authoredGround": true,
        })
    } else {
        serde_json::json!({
            "document": "asset.lot.json", "model": model, "writes": writes,
        })
    };
    let mut reference = serde_json::json!({"package": "build/reference.package"});
    if let Some(lot) = reference_lot {
        reference["lot"] = Value::String(lot);
    }
    let schema = serde_json::json!({
        "$schema": "openscp.asset",
        "schema_version": 2,
        "id": schema_id,
        "kind": "building-asset",
        "reference": reference,
        "scene": scene,
        "semantics": {"properties": [], "lotChange": true},
        "outputs": {"package": format!("000_OpenSCP_{output_name}.package"), "mode": "full"}
    });
    Ok(SchemaCandidate {
        path: "<asset-editor-adapter>".into(),
        raw: serde_json::to_string(&schema).map_err(|e| e.to_string())?,
        document: "asset.lot.json".into(),
    })
}

fn json_u32(value: Option<&Value>, field: &str) -> Result<u32, String> {
    value
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| format!("{field} must be an unsigned 32-bit integer"))
}

pub(super) fn resolve_source_package(raw: &str) -> Result<PathBuf, String> {
    let normalized = raw.trim_start_matches(r"\\?\").replace('\\', "/");
    let candidates = [
        PathBuf::from(normalized.replace(
            "D:/ea-games/SimCity",
            "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow",
        )),
        PathBuf::from(&normalized),
        PathBuf::from(
            r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_DLC0.package",
        ),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| format!("sourceLot.packagePath is unavailable: {raw}"))
}

fn select_lot_config(
    path: &Path,
    type_id: u32,
    group: u32,
    instance: u32,
) -> Result<(u32, u32, u32), String> {
    let package = dbpf::Package::open(path).map_err(|e| format!("open reference package: {e}"))?;
    let mut candidates = package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == type_id && entry.id.group == 0x58F0_C000)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| entry.id.instance);
    let selected = candidates.first().ok_or_else(|| {
        format!("reference package has no lot-config (type {type_id:08X}); sourceLot {group:08X}:{instance:08X} is an asset dictionary")
    })?;
    Ok((selected.id.type_id, selected.id.group, selected.id.instance))
}

fn sanitize_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
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
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(&path, root, depth + 1, out)?;
            continue;
        }
        if !kind.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
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
    fn refuses_to_guess_across_different_documents() {
        let error = select(
            vec![
                candidate("schemas/a.json", "asset.a.json"),
                candidate("schemas/b.json", "asset.b.json"),
            ],
            &|_| true,
        )
        .unwrap_err();
        assert!(error.contains("multiple asset schemas"), "{error}");
        assert!(error.contains("schemas/a.json"), "{error}");
    }

    #[test]
    fn prefers_highest_iteration_when_document_matches() {
        let picked = select(
            vec![
                candidate("schemas/assets/m1.json", "asset.lot.json"),
                candidate("schemas/assets/m1.s23.json", "asset.lot.json"),
            ],
            &|_| true,
        )
        .unwrap();
        assert_eq!(picked.path, "schemas/assets/m1.s23.json");
    }

    #[test]
    fn empty_project_names_the_expected_shape() {
        let error = select(vec![], &|_| true).unwrap_err();
        assert!(error.contains("openscp.asset"), "{error}");
    }

    #[test]
    fn adapter_authored_with_reference_package_synthesizes_donor_free_schema() {
        let root = crate::test_scratch::scratch_base().join(format!(
            "openscp-adapter-authored-ok-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("assets/models")).unwrap();
        std::fs::write(root.join("assets/models/tower.glb"), b"stub").unwrap();
        let source = root.join("stock.package");
        let target = root.join("build/reference.package");
        let package = dbpf::write_uncompressed_overlay(&[]).unwrap();
        std::fs::write(&source, &package).unwrap();
        super::super::authored_reference::install(&source, &target, true).unwrap();
        super::super::authored_reference::install(&source, &target, true).unwrap();
        std::fs::write(&source, b"other").unwrap();
        assert!(super::super::authored_reference::install(&source, &target, true).is_err());
        super::super::authored_reference::install(&source, &target, false).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), package);
        let document = serde_json::json!({
            "editor": {"workbench": {
                "assetPath": "assets/models/tower.glb",
                "lotStyle": {"sourceLot": null, "authored": true},
                "writes": ["decals"]
            }}
        });
        std::fs::write(root.join("asset.lot.json"), document.to_string()).unwrap();
        let candidate = discover_or_adapt(&root).unwrap();
        let schema: serde_json::Value = serde_json::from_str(&candidate.raw).unwrap();
        assert_eq!(schema["reference"]["package"], "build/reference.package");
        assert!(schema["reference"]["lot"].is_null(), "donor-free: no reference.lot");
        assert_eq!(schema["scene"]["authoredGround"], true);
        assert_eq!(schema["outputs"]["mode"], "full");
        std::fs::write(&target, b"truncated").unwrap();
        let error = discover_or_adapt(&root).unwrap_err();
        assert!(error.starts_with("Asset Editor (asset.lot.json): open authored ground reference"));
        assert!(!error.contains("no asset schema found"));
        assert_eq!(std::fs::read(&target).unwrap(), b"truncated");
        std::fs::remove_dir_all(&root).ok();
    }
}
