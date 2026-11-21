use super::{
    parse, sources,
    spec::{Dependency, PanelSchema},
};
use serde_json::{Value, json};
use std::{fs, path::Path};
pub const SCHEMA_PATH: &str = "schemas/panel/schema.json";
pub const RUNTIME_VERSION: &str = "openscp.panel-runtime/1";

pub fn dependency(manifest: &Value) -> Result<Option<Dependency>, String> {
    match manifest.pointer("/dependencies/panel") {
        None => Ok(None),
        Some(value) => {
            let d: Dependency = serde_json::from_value(value.clone())
                .map_err(|e| format!("PANEL_DEPENDENCY:{e}"))?;
            if d.runtime != RUNTIME_VERSION {
                return Err("PANEL_RUNTIME_VERSION".into());
            }
            Ok(Some(d))
        }
    }
}
pub fn read(root: &Path) -> Result<Value, String> {
    let bytes = fs::read(root.join("package.json")).map_err(|e| e.to_string())?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if manifest["format"] != "openscp.mod" {
        return Err("PANEL_MANIFEST".into());
    }
    let Some(d) = dependency(&manifest)? else {
        return Ok(
            json!({"schema":null,"schemaPath":SCHEMA_PATH,"manifestRevision":sources::digest(&bytes),"schemaRevision":null}),
        );
    };
    let path = super::super::filesystem::project_file(root, &d.schema)?;
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let schema = parse(&raw)?;
    Ok(
        json!({"schema":schema,"schemaPath":d.schema,"dependency":d,"manifestRevision":sources::digest(&bytes),"schemaRevision":sources::digest(raw.as_bytes())}),
    )
}
/// Compare both revisions before touching either file; manifest is the commit point.
/// Schema content uses a content-addressed filename, so old dependencies never see
/// half of a save if publishing package.json fails.
pub fn save(root: &Path, request: &Value) -> Result<Value, String> {
    let current = read(root)?;
    if request["manifestRevision"] != current["manifestRevision"]
        || request["schemaRevision"] != current["schemaRevision"]
    {
        return Err("PANEL_REVISION_CONFLICT".into());
    }
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(root.join("package.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if request["schema"].is_null() {
        if let Some(deps) = manifest
            .get_mut("dependencies")
            .and_then(Value::as_object_mut)
        {
            deps.remove("panel");
        }
    } else {
        let schema: PanelSchema = parse(&request["schema"].to_string())?;
        let mut d: Dependency = serde_json::from_value(request["dependency"].clone())
            .map_err(|e| format!("PANEL_DEPENDENCY:{e}"))?;
        if d.runtime != RUNTIME_VERSION {
            return Err("PANEL_RUNTIME_VERSION".into());
        }
        sources::open(root, &d.sources)?;
        let raw = serde_json::to_vec_pretty(&schema).map_err(|e| e.to_string())?;
        d.schema = format!("schemas/panel/{}.json", sources::digest(&raw));
        let folder = crate::mod_flow::safe_path(root, "schemas/panel")?;
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        crate::atomic_fs::write_atomic(&root.join(&d.schema), &raw, true)
            .map_err(|e| e.to_string())?;
        if manifest.get("dependencies").is_none() {
            manifest["dependencies"] = json!({});
        }
        manifest["dependencies"]["panel"] = serde_json::to_value(d).map_err(|e| e.to_string())?;
    }
    if sources::digest(&fs::read(root.join("package.json")).map_err(|e| e.to_string())?)
        != request["manifestRevision"].as_str().unwrap_or("")
    {
        return Err("PANEL_REVISION_CONFLICT".into());
    }
    crate::atomic_fs::write_atomic(
        &root.join("package.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        true,
    )
    .map_err(|e| e.to_string())?;
    read(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_is_optional_but_null_or_bad_dependency_is_not() {
        assert!(
            dependency(&json!({"dependencies":{"menu":{}}}))
                .unwrap()
                .is_none()
        );
        assert!(dependency(&json!({"dependencies":{"panel":null}})).is_err());
        assert!(dependency(&json!({"dependencies":{"panel":{"schema":"bad"}}})).is_err());
    }
}
