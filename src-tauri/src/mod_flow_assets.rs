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

/// 菜单图片上传：源文件直接落到固定默认路径（打包只认这个名字，原图文件名
/// 不参与）。icon = assets/images/menu-icon.png（必须 PNG）；marquee =
/// assets/images/menu-marquee.png|.jpg（PNG/JPG）。重传覆盖。
/// 菜单图片预览读取：固定路径 → base64 data URL。预览不走 asset 协议
/// （tauri.conf 的 assetProtocol.scope 仅 $TEMP，项目目录图片必 403），
/// 与 upload 对称、无 scope 依赖。
pub(super) fn read_menu_image(root: &Path, payload: &Value) -> Result<Value, String> {
    let kind = payload["kind"].as_str().unwrap_or("icon");
    let relative = match kind {
        "icon" => "assets/images/menu-icon.png",
        "marquee" => "assets/images/menu-marquee.png",
        _ => return Err("Unknown menu image kind".into()),
    };
    let path = safe_path(root, relative)?;
    if !path.is_file() {
        return Ok(json!({ "asset": relative, "data": serde_json::Value::Null }));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let mime = "image/png";
    Ok(json!({
        "asset": relative,
        "data": format!("data:{mime};base64,{encoded}"),
    }))
}

pub(super) fn import_menu_image(root: &Path, payload: &Value) -> Result<Value, String> {
    let source = PathBuf::from(payload["path"].as_str().ok_or("Missing source path")?);
    let kind = payload["kind"].as_str().unwrap_or("icon");
    if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Menu image exceeds 32 MiB".into());
    }
    let extension = source
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let relative = match (kind, extension.as_str()) {
        ("icon", "png") => "assets/images/menu-icon.png".to_string(),
        ("marquee", "png") | ("marquee", "jpg") | ("marquee", "jpeg") => {
            "assets/images/menu-marquee.jpg".to_string()
        }
        _ => return Err("Menu icon must be PNG; marquee must be PNG or JPG".into()),
    };
    let mut bytes = fs::read(&source).map_err(|e| e.to_string())?;
    // rollover 要求 JPG：任意格式读入 → RGB 重编码（85 质量）。图标保持 PNG 原样。
    if kind == "marquee" {
        let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let mut jpg: Vec<u8> = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg, 85)
            .encode_image(&image::DynamicImage::ImageRgb8(image.to_rgb8()))
            .map_err(|e| e.to_string())?;
        bytes = jpg;
    }
    let target = safe_path(root, &relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    crate::atomic_fs::write_atomic(&target, &bytes, true).map_err(|e| e.to_string())?;
    describe(root, &relative)
}

pub(super) fn import(root: &Path, payload: &Value, blend_atlas: Option<&Path>) -> Result<Value, String> {
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
        return import_blend(root, &source, blend_atlas);
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

/// Read the Blend file with Blender and convert it to LOTM with Rust.
fn import_blend(root: &Path, source: &Path, blend_atlas: Option<&Path>) -> Result<Value, String> {
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
    let atlas = blend_atlas
        .filter(|path| path.join("catalog.json").is_file())
        .ok_or("blend_atlas_missing")?;
    crate::blend_native::trace::emit(
        "atlas_found",
        json!({ "path": atlas.display().to_string() }),
    );
    let temp = root.join(".openscp-blend-import");
    // Previous failed imports leave this dir around; wipe so extract/bundle
    // writes never collide with stale temps (Windows os error 183).
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let lotm_bytes = match blender_service::export_native_rust(&atlas, source, &temp) {
        Ok(bytes) => {
            crate::blend_native::trace::emit("native_ok", json!({ "lotm_bytes": bytes.len() }));
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

// Invalidate results written before the standalone Rust import configuration.
const BLEND_CACHE_VERSION: &str = "openscp.blender.2026-11-12.3";

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
    if result["source"] != "blend-native"
        || result["nativeAdapter"]["status"] != "native-rust"
        || result["lotm"].as_str() != Some(asset)
    {
        return None;
    }
    if !safe_path(root, asset).ok()?.is_file() {
        return None;
    }
    Some(result)
}

fn write_blend_cache(root: &Path, key: &str, result: &Value) -> Result<(), String> {
    let dir = blend_cache_dir(root, key);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    // replace=true: cache miss after deleting the LOTM still leaves result.json;
    // create-new then surfaces Windows os error 183 and blocks every re-import.
    crate::atomic_fs::write_atomic(&dir.join("result.json"), &bytes, true)
        .map_err(|error| error.to_string())
}

/// Persist the Rust exporter's LOTM as the project model.
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
    // Same digest ⇒ same bytes; replace keeps re-imports idempotent.
    crate::atomic_fs::write_atomic(&target, &lotm_bytes, true).map_err(|e| e.to_string())?;
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

    /// Manual end-to-end check of the .blend → LOTM path.
    ///
    /// ```text
    /// OPENSCP_TEST_BLEND=<file.blend> \
    /// cargo test -p fluffy-open-scp native_blend_import_writes_lotm_only -- --ignored
    /// ```
    #[test]
    #[ignore = "requires Blender and the staged atlas data"]
    fn native_blend_import_writes_lotm_only() {
        let source =
            std::env::var("OPENSCP_TEST_BLEND").expect("set OPENSCP_TEST_BLEND to a .blend file");
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
        let atlas = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/blend-atlas");
        let payload = json!({ "path": source });
        let result = import(&root, &payload, Some(&atlas)).expect("native import");
        let asset = result["asset"].as_str().expect("asset");
        assert!(asset.ends_with(".lotm"), "asset must be LOTM: {asset}");
        assert_eq!(result["source"], "blend-native");
        assert_eq!(result["nativeAdapter"]["status"], "native-rust");
        let bytes = fs::read(root.join(asset)).unwrap();
        crate::asset_build_m1::lotm::parse(&bytes).expect("valid LOTM container");
        assert_eq!(import(&root, &payload, Some(&atlas)).unwrap()["cache"], "hit");
        fs::remove_file(root.join(asset)).unwrap();
        let reimported = import(&root, &payload, Some(&atlas)).unwrap();
        assert_eq!(reimported["asset"], asset);
        assert!(reimported.get("cache").is_none());
        assert_eq!(fs::read(root.join(asset)).unwrap(), bytes);
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
