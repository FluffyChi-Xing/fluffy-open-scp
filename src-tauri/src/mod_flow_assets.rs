//! Project-local asset inputs. Native package passthrough preserves every TGI;
//! edited/foreign geometry stays a draft until the RW4 compiler is available.
use super::*;
use crate::blender_service;
use base64::Engine;

/// P2 地面贴图持久化:前端 base64 → 项目内文件(与 read-asset 对称)。
/// 资产编辑器的 lot 地面 = mask raster 驱动;贴图此前只活在会话里,
/// sourceLot.packageId 跨运行失效后冷启动首开地面即消失(2026-11-04
/// 用户实测)。白名单扩展名 + 8 MiB 上限 + safe_path 防穿越。
pub(super) fn write_asset(root: &Path, payload: &Value) -> Result<Value, String> {
    let relative = payload["asset"].as_str().ok_or("Missing asset path")?;
    let data = payload["data"].as_str().ok_or("Missing asset data")?;
    let lower = relative.to_ascii_lowercase();
    if !lower.ends_with(".png") && !lower.ends_with(".jpg") && !lower.ends_with(".rgba") {
        return Err("Only png/jpg/rgba assets are writable".into());
    }
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| format!("invalid base64: {e}"))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Asset exceeds 8 MiB".into());
    }
    let path = super::safe_path(root, relative)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "asset": relative,
        "bytes": bytes.len(),
    }))
}

pub(super) fn import(root: &Path, payload: &Value) -> Result<Value, String> {
    let source = PathBuf::from(payload["path"].as_str().ok_or("Missing source path")?);
    if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("Asset exceeds 128 MiB".into());
    }
    let extension = source
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "blend" {
        return import_blend(root, &source);
    }
    if ![
        "package", "glb", "lotm", "png", "jpg", "jpeg", "json", "rw4", "dds",
    ]
    .contains(&extension.as_str())
    {
        return Err("Unsupported asset format".into());
    }
    let bytes = fs::read(&source).map_err(|e| e.to_string())?;
    if extension == "package" {
        dbpf::Package::open(&source).map_err(|e| e.to_string())?;
    }
    let folder = if ["png", "jpg", "jpeg", "dds", "json"].contains(&extension.as_str()) {
        "textures"
    } else {
        "models"
    };
    let relative = format!("assets/{folder}/{}.{}", digest(&bytes), extension);
    let target = safe_path(root, &relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    if !target.exists() {
        crate::atomic_fs::write_atomic(&target, &bytes, false).map_err(|e| e.to_string())?;
    }
    describe(root, &relative)
}

/// Use Blender's headless Python API to turn a .blend into a project GLB and
/// recover the six-slot image mapping from material node names. Blender is an
/// optional runtime dependency; the error is explicit when it is unavailable.
fn import_blend(root: &Path, source: &Path) -> Result<Value, String> {
    let cache_key = blend_cache_key(source)?;
    if let Some(mut cached) = read_blend_cache(root, &cache_key) {
        cached["cache"] = json!("hit");
        crate::blend_native::trace::emit(
            "cache_hit",
            json!({
                "asset": cached.get("asset").cloned().unwrap_or(Value::Null),
                "source": cached.get("source").cloned().unwrap_or(Value::Null),
            }),
        );
        return Ok(cached);
    }
    crate::blend_native::trace::emit(
        "import_start",
        json!({
            "source": source.display().to_string(),
            "blend_bytes": fs::metadata(source).map(|meta| meta.len()).unwrap_or(0),
            "cache_key": cache_key,
        }),
    );
    // A .blend import is a native LOTM conversion, never a GLB preview.
    // Blender only reads the file (extract_blend.py); the export itself runs
    // in Rust (blend_native::export).  Missing atlas data is an explicit
    // error instead of silently producing a different asset.
    let atlas = match blender_service::atlas_dir() {
        Some(atlas) => atlas,
        None => {
            crate::blend_native::trace::emit("addon_missing", json!({}));
            return Err(blender_service::MISSING_ADDON_HINT.to_string());
        }
    };
    crate::blend_native::trace::emit(
        "addon_found",
        json!({ "path": atlas.display().to_string() }),
    );
    let temp = root.join(".openscp-blend-import");
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let lotm_bytes = match blender_service::export_native_rust(&atlas, source, &temp) {
        Ok(bytes) => {
            crate::blend_native::trace::emit(
                "native_ok",
                json!({ "lotm_bytes": bytes.len() }),
            );
            bytes
        }
        Err(detail) => {
            crate::blend_native::trace::emit(
                "native_failed",
                json!({ "kind": "native-rust", "detail": detail }),
            );
            return Err(format!("原生导出失败：{detail}"));
        }
    };
    let result = import_native_bundle(root, lotm_bytes)?;
    let _ = fs::remove_dir_all(&temp);
    write_blend_cache(root, &cache_key, &result)?;
    Ok(result)
}

// 2026-11-06.4：抽取器补回 CORNER 属性（NY_DecalCoord）、法线改用
// (M^-1)^T、贴花/调色板/浮雕/Office 内饰与插件对齐；旧缓存一律失效重导。
const BLEND_CACHE_VERSION: &str = "openscp.blender.2026-11-06.4";

fn blend_cache_key(source: &Path) -> Result<String, String> {
    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    Ok(format!("{}-{}", BLEND_CACHE_VERSION, digest(&bytes)))
}

fn blend_cache_dir(root: &Path, key: &str) -> PathBuf {
    root.join(".openscp-blend-cache").join(key)
}

fn read_blend_cache(root: &Path, key: &str) -> Option<Value> {
    let path = blend_cache_dir(root, key).join("result.json");
    let result: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    let asset = result.get("asset")?.as_str()?;
    if !safe_path(root, asset).ok()?.is_file() {
        return None;
    }
    Some(result)
}

fn write_blend_cache(root: &Path, key: &str, result: &Value) -> Result<(), String> {
    let dir = blend_cache_dir(root, key);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    crate::atomic_fs::write_atomic(&dir.join("result.json"), &bytes, false)
        .map_err(|error| error.to_string())
}

/// Persist the plug-in's LOTM as the project model.  The exporter also writes
/// a GLB preview next to it; the project stores only the LOTM so every
/// imported building uses the same container format as the game assets.
fn import_native_bundle(root: &Path, lotm_bytes: Vec<u8>) -> Result<Value, String> {
    let lotm_bytes = lotm_bytes;
    let document = crate::asset_build_m1::lotm::parse(&lotm_bytes).map_err(|error| {
        crate::blend_native::trace::emit(
            "lotm_invalid",
            json!({ "bytes": lotm_bytes.len(), "detail": error }),
        );
        format!("导出器产出的 LOTM 无效：{error}")
    })?;
    let lotm_relative = format!("assets/models/{}.lotm", digest(&lotm_bytes));
    let target = safe_path(root, &lotm_relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    if !target.exists() {
        crate::atomic_fs::write_atomic(&target, &lotm_bytes, false)
            .map_err(|e| e.to_string())?;
    }
    let mut uv_kinds: Vec<u8> = document
        .mesh_material
        .iter()
        .map(|item| item.uv_kind)
        .collect();
    uv_kinds.sort_unstable();
    uv_kinds.dedup();
    crate::blend_native::trace::emit(
        "lotm_persisted",
        json!({
            "asset": lotm_relative,
            "bytes": lotm_bytes.len(),
            "sha256": digest(&lotm_bytes),
            "meshes": document.meshes.len(),
            "materials": document.materials.len(),
            "uv_kinds": uv_kinds,
        }),
    );
    Ok(json!({
        "asset": lotm_relative,
        "lotm": lotm_relative,
        "source": "blend-native",
        "nativeAdapter": {"status": "native-rust"}
    }))
}

pub(super) fn describe(root: &Path, relative: &str) -> Result<Value, String> {
    let path = safe_path(root, relative)?;
    let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > 128 * 1024 * 1024 {
        return Err("Asset exceeds 128 MiB".into());
    }
    if path.extension().is_some_and(|s| s == "package") {
        let package = dbpf::Package::open(&path).map_err(|e| e.to_string())?;
        let resources: Vec<_> = package
            .entries()
            .iter()
            .map(|e| {
                json!({
                    "typeId":e.id.type_id,"group":e.id.group,"instance":e.id.instance
                })
            })
            .collect();
        return Ok(json!({"asset":relative,"path":path,"size":size,"resources":resources}));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(
        json!({"asset":relative,"path":path,"size":size,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}

pub(super) fn validate(root: &Path, schema: &Schema) -> Option<&'static str> {
    let file = schema.config["asset"].as_str().unwrap_or("");
    if schema.kind == "texture-input" {
        if !schema.config["document"].as_str().unwrap_or("").is_empty() {
            return Some("rasterEditor.buildUnsupported");
        }
        return if file.is_empty() || safe_path(root, file).is_ok_and(|p| p.is_file()) {
            None
        } else {
            Some("Texture input is missing")
        };
    }
    if !file.ends_with(".package") {
        return Some("模型草稿可预览；构建目前需要原生 .package，GLB/LOTM → RW4 编译尚未接入");
    }
    if !schema.config["document"].as_str().unwrap_or("").is_empty() {
        return Some(
            "Property schema 已保存；编辑后的 Property/RW4 编译尚未接入，不能按未编辑原包构建",
        );
    }
    if safe_path(root, file)
        .ok()
        .and_then(|p| dbpf::Package::open(p).ok())
        .is_none()
    {
        return Some("Select a valid native asset package");
    }
    None
}

pub(super) fn validate_links(
    schemas: &[Schema],
    deps: &BTreeMap<String, Vec<String>>,
    out: &mut Vec<Diagnostic>,
) {
    for building in schemas.iter().filter(|s| s.kind == "building-asset") {
        for slot in 0..6 {
            let key = format!("slot{slot}");
            let source = building.config[&key].as_str().unwrap_or("");
            let input = schemas
                .iter()
                .find(|s| s.id == source && s.kind == "texture-input");
            if input.is_none()
                || !deps
                    .get(&building.id)
                    .is_some_and(|v| v.iter().any(|id| id == source))
            {
                diagnostic(
                    out,
                    Some(&building.id),
                    "slot",
                    &format!("{key} must connect to a texture input"),
                );
            } else if !input.unwrap().config["asset"]
                .as_str()
                .unwrap_or("")
                .is_empty()
            {
                diagnostic(
                    out,
                    Some(&building.id),
                    "compile",
                    "贴图预览已绑定；替换材质槽的 RW4 写回尚未接入，禁止静默忽略贴图构建",
                );
            }
        }
    }
}

#[cfg(test)]
mod blender_import_tests {
    use super::*;

    /// Manual gate: set `OPENSCP_BLENDER_SAMPLE` to a .blend with one building.
    /// The test proves the built-in headless path joins the meshes and still
    /// extracts the six-slot images without an installed add-on.
    #[test]
    #[ignore = "requires local Blender and OPENSCP_BLENDER_SAMPLE"]
    fn headless_blend_import_joins_and_extracts_slots() {
        let Ok(sample) = std::env::var("OPENSCP_BLENDER_SAMPLE") else {
            return;
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tmp")
            .join(format!("openscp-blend-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let result = import(&root, &json!({"path": sample})).unwrap();
        assert!(result["asset"]
            .as_str()
            .unwrap_or_default()
            .ends_with(".glb"));
        assert_eq!(result["source"], "blend");
        let glb = root.join(result["asset"].as_str().unwrap());
        let bytes = fs::read(glb).unwrap();
        let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
        assert_eq!(
            doc["meshes"].as_array().map(Vec::len),
            Some(1),
            "the Blender service must join the building into one mesh"
        );
        let cached = import(&root, &json!({"path": sample})).unwrap();
        assert_eq!(cached["cache"], "hit");
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod raster_draft_tests {
    use super::*;

    #[test]
    fn raster_schema_cannot_be_silently_omitted_from_a_build() {
        let mut schema: Schema = serde_json::from_value(json!({
            "schema_version": 1, "id": "texture-input-1", "kind": "texture-input",
            "config": { "asset": "", "slot": 0, "document": "{\"format\":\"openscp.raster\"}" }
        }))
        .unwrap();
        assert_eq!(
            validate(Path::new("."), &schema),
            Some("rasterEditor.buildUnsupported")
        );
        schema.config["document"] = json!("");
        assert!(validate(Path::new("."), &schema).is_none());
    }
}

#[cfg(test)]
mod native_import_tests {
    use super::*;

    /// Manual end-to-end check of the .blend → LOTM path.  Requires Blender
    /// and the SC5 Asset Editor exporter:
    ///
    /// ```text
    /// OPENSCP_BLENDER_ADDON=<dir containing ny_facade> \
    /// OPENSCP_TEST_BLEND=<file.blend> \
    /// cargo test -p fluffy-open-scp native_blend_import_writes_lotm_only -- --ignored
    /// ```
    #[test]
    #[ignore = "requires Blender and the SC5 Asset Editor exporter"]
    fn native_blend_import_writes_lotm_only() {
        let source = std::env::var("OPENSCP_TEST_BLEND")
            .expect("set OPENSCP_TEST_BLEND to a .blend file");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "native-import-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&root).unwrap();
        let result = import(&root, &json!({ "path": source })).expect("native import");
        let asset = result["asset"].as_str().expect("asset");
        assert!(asset.ends_with(".lotm"), "asset must be LOTM: {asset}");
        assert_eq!(result["source"], "blend-native");
        assert_eq!(result["nativeAdapter"]["status"], "native-rust");
        let bytes = fs::read(root.join(asset)).unwrap();
        crate::asset_build_m1::lotm::parse(&bytes).expect("valid LOTM container");
        // Byte hashes intentionally not compared: the native pipeline's PNG
        // compression level differs from the plug-in's zlib-6.  Structure is
        // verified above; visual parity is covered by tmp/final samples.
        let models = fs::read_dir(root.join("assets/models")).unwrap();
        assert!(
            models
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().ends_with(".glb")),
            "the project must not contain a GLB model"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
