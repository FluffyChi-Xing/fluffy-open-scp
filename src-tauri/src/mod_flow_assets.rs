//! Project-local asset inputs. Native package passthrough preserves every TGI;
//! edited/foreign geometry stays a draft until the RW4 compiler is available.
use super::*;
use base64::Engine;

pub(super) fn import(root: &Path, payload: &Value) -> Result<Value, String> {
    let source = PathBuf::from(payload["path"].as_str().ok_or("Missing source path")?);
    if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("Asset exceeds 128 MiB".into());
    }
    let extension = source.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
    if !["package", "glb", "lotm", "png", "jpg", "jpeg", "json", "rw4", "dds"].contains(&extension.as_str()) {
        return Err("Unsupported asset format".into());
    }
    let bytes = fs::read(&source).map_err(|e| e.to_string())?;
    if extension == "package" { dbpf::Package::open(&source).map_err(|e| e.to_string())?; }
    let folder = if ["png", "jpg", "jpeg", "dds", "json"].contains(&extension.as_str()) { "textures" } else { "models" };
    let relative = format!("assets/{folder}/{}.{}", digest(&bytes), extension);
    let target = safe_path(root, &relative)?;
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    if !target.exists() { crate::atomic_fs::write_atomic(&target, &bytes, false).map_err(|e| e.to_string())?; }
    describe(root, &relative)
}

pub(super) fn describe(root: &Path, relative: &str) -> Result<Value, String> {
    let path = safe_path(root, relative)?;
    let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > 128 * 1024 * 1024 { return Err("Asset exceeds 128 MiB".into()); }
    if path.extension().is_some_and(|s| s == "package") {
        let package = dbpf::Package::open(&path).map_err(|e| e.to_string())?;
        let resources: Vec<_> = package.entries().iter().map(|e| json!({
            "typeId":e.id.type_id,"group":e.id.group,"instance":e.id.instance
        })).collect();
        return Ok(json!({"asset":relative,"path":path,"size":size,"resources":resources}));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(json!({"asset":relative,"path":path,"size":size,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)}))
}

pub(super) fn validate(root: &Path, schema: &Schema) -> Option<&'static str> {
    let file = schema.config["asset"].as_str().unwrap_or("");
    if schema.kind == "texture-input" {
        return if file.is_empty() || safe_path(root, file).is_ok_and(|p| p.is_file()) { None }
        else { Some("Texture input is missing") };
    }
    if !file.ends_with(".package") {
        return Some("模型草稿可预览；构建目前需要原生 .package，GLB/LOTM → RW4 编译尚未接入");
    }
    if !schema.config["document"].as_str().unwrap_or("").is_empty() {
        return Some("Property schema 已保存；编辑后的 Property/RW4 编译尚未接入，不能按未编辑原包构建");
    }
    if safe_path(root, file).ok().and_then(|p| dbpf::Package::open(p).ok()).is_none() {
        return Some("Select a valid native asset package");
    }
    None
}

pub(super) fn validate_links(schemas: &[Schema], deps: &BTreeMap<String, Vec<String>>, out: &mut Vec<Diagnostic>) {
    for building in schemas.iter().filter(|s| s.kind == "building-asset") {
        for slot in 0..6 {
            let key = format!("slot{slot}");
            let source = building.config[&key].as_str().unwrap_or("");
            let input = schemas.iter().find(|s| s.id == source && s.kind == "texture-input");
            if input.is_none() || !deps.get(&building.id).is_some_and(|v| v.iter().any(|id| id == source)) {
                diagnostic(out, Some(&building.id), "slot", &format!("{key} must connect to a texture input"));
            } else if !input.unwrap().config["asset"].as_str().unwrap_or("").is_empty() {
                diagnostic(out, Some(&building.id), "compile", "贴图预览已绑定；替换材质槽的 RW4 写回尚未接入，禁止静默忽略贴图构建");
            }
        }
    }
}
