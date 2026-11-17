use super::{RoadSchema, parse};
use crate::mod_flow::{digest, safe_path};
use dbpf::{OverlayEntry, Package};
use serde_json::{Value as Json, json};
use std::{fs, path::Path};

fn load(root: &Path) -> Result<(Json, String, String), String> {
    let manifest: Json = serde_json::from_slice(
        &fs::read(safe_path(root, "package.json")?).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest["format"] != "openscp.mod" || manifest["build"]["kind"] != "road" {
        return Err("road_manifest_invalid".into());
    }
    let path = manifest["build"]["schema"]
        .as_str()
        .ok_or("road_schema_missing")?
        .to_owned();
    let raw = fs::read_to_string(safe_path(root, &path)?).map_err(|e| e.to_string())?;
    Ok((manifest, path, raw))
}

fn revision(manifest: &Json) -> String {
    let mut editable = manifest.clone();
    if let Some(road) = editable
        .pointer_mut("/dependencies/road")
        .and_then(Json::as_object_mut)
    {
        road.remove("resolved");
    }
    digest(&serde_json::to_vec(&editable).expect("JSON value serializes"))
}

fn state(manifest: Json, raw: &str) -> Result<Json, String> {
    Ok(json!({
        "manifestRevision": revision(&manifest),
        "schemaRevision": digest(raw.as_bytes()),
        "schema": serde_json::from_str::<Json>(raw).map_err(|e| e.to_string())?,
        "manifest": manifest,
    }))
}

pub(crate) fn read(root: &Path) -> Result<Json, String> {
    let (manifest, _, raw) = load(root)?;
    state(manifest, &raw)
}

pub(super) fn sync(
    manifest: &mut Json,
    previous: &str,
    path: &str,
    schema: &RoadSchema,
) -> Result<(), String> {
    let files = [
        ("game", schema.game_package.as_str()),
        ("scripts", schema.scripts_package.as_str()),
        ("menuIcon", schema.menu.icon_png.as_str()),
        ("menuMarquee", schema.menu.marquee_png.as_str()),
    ];
    let mut inputs = vec![json!({"role": "schema", "path": path, "external": false})];
    let mut internal_files = Vec::new();
    if manifest.get("outer_dependencies").is_none() {
        manifest["outer_dependencies"] = json!([]);
    }
    let outer = manifest["outer_dependencies"]
        .as_array_mut()
        .ok_or("road_manifest_dependencies_invalid")?;
    outer.retain(|entry| entry["owner"] != "road");
    for (role, file) in files.into_iter().filter(|(_, path)| !path.is_empty()) {
        let external = Path::new(file).is_absolute();
        inputs.push(json!({"role": role, "path": file, "external": external}));
        if external {
            outer.push(json!({"owner": "road", "role": role, "source_package": file}));
        } else {
            internal_files.push(file);
        }
    }
    internal_files.sort_unstable();
    internal_files.dedup();
    if manifest.get("internal_dependencies").is_none() {
        manifest["internal_dependencies"] = json!({});
    }
    let internal = manifest["internal_dependencies"]
        .as_object_mut()
        .ok_or("road_manifest_dependencies_invalid")?;
    internal.remove(previous);
    internal.insert(path.to_owned(), json!(internal_files));
    if manifest.get("dependencies").is_none() {
        manifest["dependencies"] = json!({});
    }
    let dependencies = manifest["dependencies"]
        .as_object_mut()
        .ok_or("road_manifest_dependencies_invalid")?;
    dependencies.insert("road".into(), json!({"inputs": inputs}));
    manifest["build"]["schema"] = json!(path);
    if let Some(base) = schema.base_recipe {
        manifest["build"]["baseRecipe"] = json!(base);
    } else if let Some(build) = manifest["build"].as_object_mut() {
        build.remove("baseRecipe");
    }
    manifest["menu"] = json!({
        "title": schema.title, "description": schema.description,
        "position": schema.menu.position,
        "iconPng": schema.menu.icon_png, "marqueePng": schema.menu.marquee_png,
    });
    Ok(())
}

pub(crate) fn save(root: &Path, request: &Json) -> Result<Json, String> {
    let (mut manifest, previous, stored) = load(root)?;
    if request["manifestRevision"] != revision(&manifest)
        || request["schemaRevision"] != digest(stored.as_bytes())
    {
        return Err("road_manifest_revision_conflict".into());
    }
    let raw = serde_json::to_string_pretty(&request["schema"]).map_err(|e| e.to_string())? + "\n";
    let schema = parse(&raw)?;
    for (path, relative_only) in [
        (&schema.game_package, false),
        (&schema.scripts_package, false),
        (&schema.menu.icon_png, true),
        (&schema.menu.marquee_png, true),
    ] {
        if !path.is_empty() && (relative_only || !Path::new(path).is_absolute()) {
            safe_path(root, path)?;
        }
    }
    for key in ["author", "version", "description"] {
        if let Some(value) = request["metadata"][key].as_str() {
            manifest[key] = json!(value);
        }
    }
    let same_schema =
        serde_json::from_str::<Json>(&stored).is_ok_and(|value| value == request["schema"]);
    let (path, raw) = if same_schema {
        (previous.clone(), stored)
    } else {
        let file = format!("schemas/road/revisions/{}.json", digest(raw.as_bytes()));
        let target = safe_path(root, &file)?;
        if !target.exists() {
            fs::create_dir_all(target.parent().expect("schema has a parent"))
                .map_err(|e| e.to_string())?;
            crate::atomic_fs::write_atomic(&target, raw.as_bytes(), false)
                .map_err(|e| e.to_string())?;
        } else if fs::read(&target).map_err(|e| e.to_string())? != raw.as_bytes() {
            return Err("road_schema_revision_conflict".into());
        }
        (file, raw)
    };
    let resolved = same_schema.then(|| manifest["dependencies"]["road"]["resolved"].clone());
    sync(&mut manifest, &previous, &path, &schema)?;
    if let Some(report) = resolved.filter(|value| !value.is_null()) {
        manifest["dependencies"]["road"]["resolved"] = report;
    }
    write(root, &manifest)?;
    state(manifest, &raw)
}

fn write(root: &Path, manifest: &Json) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    crate::atomic_fs::write_atomic(&safe_path(root, "package.json")?, &bytes, true)
        .map_err(|e| e.to_string())
}

pub(super) fn record_build(
    root: &Path,
    raw: &str,
    schema: &RoadSchema,
    game: &Package,
    scripts: &Package,
    entries: &[OverlayEntry],
    sha256: &str,
) -> Result<(), String> {
    if !safe_path(root, "package.json")?.is_file() {
        return Ok(());
    }
    {
        let _lock = crate::mod_flow::FLOW_LOCK
            .lock()
            .map_err(|_| "Workflow lock failed")?;
        if super::project_schema(root)?.is_none_or(|(_, current)| current != raw) {
            return Ok(());
        }
    }
    let report = super::dependencies::report(schema, game, scripts, entries, sha256)?;
    let _lock = crate::mod_flow::FLOW_LOCK
        .lock()
        .map_err(|_| "Workflow lock failed")?;
    let (mut manifest, path, current) = load(root)?;
    if current != raw {
        return Ok(());
    }
    sync(&mut manifest, &path, &path, &parse(raw)?)?;
    manifest["dependencies"]["road"]["resolved"] = report;
    write(root, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_menu_and_dependencies_with_the_new_schema_revision() {
        let root = std::env::temp_dir().join(format!(
            "openscp-road-manifest-{}",
            super::super::events::now_ms()
        ));
        fs::create_dir(&root).unwrap();
        let mut manifest = json!({
            "format": "openscp.mod", "origin": "openscp", "mod_type": "road",
            "outer_dependencies": [{"owner": "other", "source_package": "kept.package"}],
        });
        super::super::initialize_project(&root, "Example", None, &mut manifest).unwrap();
        write(&root, &manifest).unwrap();
        let state = read(&root).unwrap();
        let mut schema = state["schema"].clone();
        schema["title"] = json!("Tree boulevard");
        schema["gamePackage"] = json!("inputs/SimCity_Game.package");
        schema["menu"] = json!({"position": 123, "iconPng": "assets/images/menu.png"});
        let request = json!({
            "manifestRevision": state["manifestRevision"], "schemaRevision": state["schemaRevision"],
            "schema": schema, "metadata": {"author": "Example"},
        });
        let revised = save(&root, &request).unwrap();
        let path = revised["manifest"]["build"]["schema"].as_str().unwrap();
        assert!(path.starts_with("schemas/road/revisions/"));
        assert_eq!(revised["manifest"]["menu"]["position"], 123);
        assert_eq!(revised["manifest"]["menu"]["title"], "Tree boulevard");
        assert_eq!(
            revised["manifest"]["internal_dependencies"][path],
            json!(["assets/images/menu.png", "inputs/SimCity_Game.package"])
        );
        assert_eq!(
            revised["manifest"]["outer_dependencies"][0]["source_package"],
            "kept.package"
        );
        assert_eq!(
            save(&root, &request).unwrap_err(),
            "road_manifest_revision_conflict"
        );
        assert_eq!(read(&root).unwrap(), revised);
        fs::remove_dir_all(root).unwrap();
    }
}
