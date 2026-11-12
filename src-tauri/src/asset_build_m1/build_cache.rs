//! 构建缓存（G3，用户指令 2026-11-05："已确认的无变更流程，只要跑通
//! 构建器自检就能保证结果正确；有增量/变更内容，打包必留关键日志"）。
//!
//! 两级内容寻址缓存，全部落在项目 `.build/cache/` 下：
//! - **runs/**（G3a 全链复用）：输入指纹（schema 语义 + 源包/身份包/
//!   场景文档/模型/六槽的原始字节 SHA + 处理器版本）命中且产物包
//!   sha256 自检通过时，直接返回上次结果——"无变更流程跑通自检即正确"
//!   的实现。事件流是一条 `cache-reuse` 记录（Actions 页可见）。
//! - **insert-model/**（G3b 材质缓存）：材质输入指纹（模型 + 六槽 +
//!   源包占用 + 形状/旋转/变换 + 处理器版本）命中时，材质/RW4 资源
//!   直接从缓存入池，跳过编译与图集重编码——场景小改动不再为 268MB
//!   材质付重编码代价。alloc_free_instance 是内容寻址的（SHA 前 4
//!   字节 + 池冲突递增），因此缓存命中的实例号与重编译完全一致。
//!
//! 失效策略：指纹包含处理器版本与全部相关输入；产物自检（sha256）
//! 不通过即视为 miss 并全量重建。缓存永不参与 build_hash 与产物字节。

use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::path::{Path, PathBuf};

/// 缓存布局版本：语义变化时递增（旧缓存自然失效）。
// Bump whenever the Python→Rust Building40/material migration changes the
// produced model or six-slot resources.  Reusing a pre-migration cache can
// silently restore an old RW4/texture binding and present as missing windows.
pub(super) const CACHE_VERSION: &str = "g3-8-street-placement";
pub(super) const SCENE_OUTPUT_VERSION: &str = "lot-mask-row-order-authored-locations-menu-selection-v2";

fn digest_bytes(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(digest_bytes(&bytes))
}

fn mix(parts: &[&str]) -> String {
    let mut acc = String::new();
    for part in parts {
        acc.push_str(&part.len().to_string());
        acc.push(':');
        acc.push_str(part);
        acc.push('|');
    }
    digest_bytes(acc.as_bytes())
}

// ---- G3a: 全链复用 ----

/// 一次成功构建的复用凭据。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCacheEntry {
    pub build_hash: String,
    pub schema_hash: String,
    pub source_hash: String,
    /// 相对 `.build/<build_hash>/` 的产物路径（`packages/<name>`）。
    pub package_relative: String,
    pub sha256: String,
    pub resource_count: usize,
    pub delta_mode: bool,
    pub menu_instance: Option<u32>,
    pub patched_resources: usize,
    /// 序列化的 SceneSemanticSummary（原样回放）。
    pub scene: serde_json::Value,
    /// 复用结果回放的 started/finished 基准（记录写入时刻）。
    pub submitted_at_ms: u64,
}

fn runs_dir(root: &Path) -> PathBuf {
    root.join(".build").join("cache").join("runs")
}

/// 输入指纹：schema 语义哈希 + 全部原始输入文件字节哈希 + 处理器版本。
/// 只做顺序读+SHA（不解压、不编译），源包 197MB 量级也在亚秒级。
pub(crate) fn input_fingerprint(
    root: &Path,
    plan: &super::plan::PropertyBuildPlan,
    scene_document: &str,
) -> String {
    let mut parts: Vec<String> = vec![
        CACHE_VERSION.into(),
        SCENE_OUTPUT_VERSION.into(),
        plan.processor_version.into(),
        plan.intermediate.schema_hash.clone(),
    ];
    if let Some(package) = plan.intermediate.references.package.as_deref() {
        parts.push(digest_file(&root.join(package)).unwrap_or_default());
    }
    if let Some(identity) = plan
        .menu
        .as_ref()
        .and_then(|menu| menu.identity_package.as_deref())
    {
        parts.push(digest_file(&root.join(identity)).unwrap_or_default());
    }
    if let Some(hash) = digest_file(&root.join(scene_document)) {
        parts.push(hash);
    }
    if plan.intermediate.scene.authored_ground {
        for path in [super::stages::AUTHORED_GROUND_META, super::stages::AUTHORED_GROUND_MASK] {
            parts.push(format!("{path}={}", digest_file(&root.join(path)).unwrap_or_default()));
        }
    }
    if let Ok(bytes) = std::fs::read(root.join(scene_document)) {
        if let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(path) = document
                .pointer("/editor/workbench/lotStyle/sourceLot/packagePath")
                .and_then(serde_json::Value::as_str)
            {
                let digest = super::filesystem::input_file(root, path)
                    .ok()
                    .and_then(|path| digest_file(&path))
                    .unwrap_or_default();
                parts.push(format!("groundSource={digest}"));
            }
        }
    }
    if let Some(model) = plan.intermediate.scene.model.as_deref() {
        parts.push(digest_file(&root.join(model)).unwrap_or_default());
    }
    for (slot, path) in &plan.intermediate.scene.slots {
        parts.push(format!(
            "{slot}={}",
            digest_file(&root.join(path)).unwrap_or_default()
        ));
    }
    let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    mix(&refs)
}

/// 命中且产物自检通过 → Some(凭据)。任何不一致都返回 None（视为 miss）。
pub(crate) fn lookup_run(root: &Path, fingerprint: &str) -> Option<RunCacheEntry> {
    let path = runs_dir(root).join(format!("{fingerprint}.json"));
    let bytes = std::fs::read(&path).ok()?;
    let entry: RunCacheEntry = serde_json::from_slice(&bytes).ok()?;
    let package = root
        .join(".build")
        .join(&entry.build_hash)
        .join(&entry.package_relative);
    let actual = digest_file(&package)?;
    (actual == entry.sha256).then_some(entry)
}

pub(crate) fn store_run(root: &Path, fingerprint: &str, entry: &RunCacheEntry) {
    let dir = runs_dir(root);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let bytes = match serde_json::to_vec_pretty(entry) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let _ = crate::atomic_fs::write_atomic(&dir.join(format!("{fingerprint}.json")), &bytes, true);
}

// ---- G3b: insert-model 材质缓存 ----

/// 材质输入指纹：模型 + 六槽载荷 + 源包占用 + 编译形状/旋转/变换。
/// 场景文档不在键内——场景编辑不应触发材质重编码。
#[allow(clippy::too_many_arguments)]
pub(crate) fn material_fingerprint(
    root: &Path,
    plan: &super::plan::PropertyBuildPlan,
    model_sha256: &str,
    slot_payload_sha256: &str,
    rotation_degrees: u32,
    transform_repr: &str,
) -> String {
    let source = plan
        .intermediate
        .references
        .package
        .as_deref()
        .and_then(|package| digest_file(&root.join(package)))
        .unwrap_or_default();
    let parts = [
        CACHE_VERSION,
        plan.processor_version,
        model_sha256,
        slot_payload_sha256,
        &source,
        plan.intermediate.scene.model_shape.as_deref().unwrap_or(""),
        &rotation_degrees.to_string(),
        transform_repr,
    ];
    mix(&parts)
}

/// 缓存的 insert-model 产出：模型身份 + 材质资源（tgi → 载荷文件）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialCacheEntry {
    /// LOD 模型 [typeId, group, instance] ×4（None = 无模型路径）。
    pub model_ids: Option<[[u32; 3]; 4]>,
    pub material_resource_count: usize,
    /// (typeId, instance, payload file name)；载荷存同目录 `r/<name>`。
    pub entries: Vec<(u32, u32, String)>,
}

fn material_cache_dir(root: &Path, fingerprint: &str) -> PathBuf {
    root.join(".build")
        .join("cache")
        .join("insert-model")
        .join(fingerprint)
}

pub(crate) fn lookup_material(root: &Path, fingerprint: &str) -> Option<MaterialCacheEntry> {
    let dir = material_cache_dir(root, fingerprint);
    let manifest: MaterialCacheEntry =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
    // 自检：所有载荷文件必须齐备，缺一即 miss。
    for (_, _, file) in &manifest.entries {
        if !dir.join("r").join(file).is_file() {
            return None;
        }
    }
    Some(manifest)
}

pub(crate) fn read_material_payload(root: &Path, fingerprint: &str, file: &str) -> Option<Vec<u8>> {
    std::fs::read(material_cache_dir(root, fingerprint).join("r").join(file)).ok()
}

pub(crate) fn store_material(
    root: &Path,
    fingerprint: &str,
    entry: &MaterialCacheEntry,
    payloads: &[(u32, u32, Vec<u8>)],
) {
    let dir = material_cache_dir(root, fingerprint);
    if std::fs::create_dir_all(dir.join("r")).is_err() {
        return;
    }
    for ((_, _, file), (_, _, bytes)) in entry.entries.iter().zip(payloads) {
        if crate::atomic_fs::write_atomic(&dir.join("r").join(file), bytes, true).is_err() {
            return;
        }
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(entry) {
        let _ = crate::atomic_fs::write_atomic(&dir.join("manifest.json"), &bytes, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let root = crate::test_scratch::scratch_base().join(format!(
            "openscp-build-cache-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn repainting_authored_mask_invalidates_run_cache() {
        let root = temp_root("authored-mask");
        let mask = root.join(super::super::stages::AUTHORED_GROUND_MASK);
        std::fs::create_dir_all(mask.parent().unwrap()).unwrap();
        let plan = super::super::plan::plan_from_schema(
            r#"{"$schema":"openscp.asset","schema_version":2,"id":"paint",
                "kind":"building-asset","reference":{"package":"reference.package"},
                "scene":{"document":"asset.lot.json","authoredGround":true},
                "outputs":{"package":"paint.package","mode":"full"}}"#,
        ).unwrap();
        std::fs::write(&mask, [1, 0, 0, 0]).unwrap();
        let before = input_fingerprint(&root, &plan, "asset.lot.json");
        std::fs::write(&mask, [0, 1, 0, 0]).unwrap();
        assert_ne!(before, input_fingerprint(&root, &plan, "asset.lot.json"));
    }

    #[test]
    fn run_cache_roundtrip_and_corruption_miss() {
        let root = temp_root("run");
        let build = root.join(".build").join("hash1").join("packages");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join("out.package"), b"payload").unwrap();
        let entry = RunCacheEntry {
            build_hash: "hash1".into(),
            schema_hash: "s".into(),
            source_hash: "src".into(),
            package_relative: "packages/out.package".into(),
            sha256: digest_bytes(b"payload"),
            resource_count: 3,
            delta_mode: true,
            menu_instance: None,
            patched_resources: 0,
            scene: serde_json::json!({}),
            submitted_at_ms: 0,
        };
        store_run(&root, "fp1", &entry);
        assert!(lookup_run(&root, "fp1").is_some(), "roundtrip hit");
        // 产物被改动 → 自检失败 → miss。
        std::fs::write(build.join("out.package"), b"tampered").unwrap();
        assert!(
            lookup_run(&root, "fp1").is_none(),
            "tampered output is a miss"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn material_cache_roundtrip_and_missing_payload_miss() {
        let root = temp_root("material");
        let payloads = vec![(0x2F4E_681Bu32, 42u32, vec![1u8, 2, 3])];
        let entry = MaterialCacheEntry {
            model_ids: Some([
                [0x2F4E_681B, 0, 11],
                [0x2F4E_681B, 0, 12],
                [0x2F4E_681B, 0, 13],
                [0x2F4E_681B, 0, 14],
            ]),
            material_resource_count: 1,
            entries: vec![(0x2F4E_681B, 42, "a.bin".into())],
        };
        store_material(&root, "fp", &entry, &payloads);
        let hit = lookup_material(&root, "fp").expect("roundtrip hit");
        assert_eq!(hit.model_ids.map(|ids| ids[0][2]), Some(11));
        assert_eq!(
            read_material_payload(&root, "fp", "a.bin"),
            Some(vec![1, 2, 3])
        );
        // 载荷丢失 → miss。
        std::fs::remove_file(material_cache_dir(&root, "fp").join("r").join("a.bin")).unwrap();
        assert!(lookup_material(&root, "fp").is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
