//! Preflight the shared Game.js dependency before copying an authored panel.
use crate::{activity::CommandError, asset_build_m1::panel::{hash, PANEL_GAME_JS, RUNTIME_OUTPUT}};
use dbpf::{Package, ResourceId};
use sc_properties::{PropertyFile, Value};
use std::path::{Path, PathBuf};

fn read_resource(path: &Path, id: ResourceId) -> Result<Option<Vec<u8>>, String> {
    let package = Package::open(path).map_err(|error| error.to_string())?;
    package.entry(id).map(|entry| package.read(entry).map_err(|error| error.to_string())).transpose()
}

fn has_authored_panel(artifact: &Path) -> Result<bool, CommandError> {
    let error = |message: String| CommandError::new("deploy_invalid_artifact", message);
    let package = Package::open(artifact).map_err(|reason| error(reason.to_string()))?;
    for entry in package.entries().iter().filter(|entry| entry.id.type_id == 0x00B1_B104 && entry.id.group == 0x40E1_C000) {
        let raw = package.read(entry).map_err(|reason| error(reason.to_string()))?;
        let Ok(asset) = PropertyFile::parse(&raw) else { continue; };
        if asset.get(0x0D7E_ADAC).and_then(|property| property.scalar()) == Some(&Value::UInt32(hash("OpenSCPPanel_headerNone"))) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn dependency_path(artifact: &Path) -> Option<PathBuf> {
    let output = artifact.parent()?;
    let adjacent = output.join("openscp-panel-runtime-v1.package");
    let compiled = output.parent()?.join(RUNTIME_OUTPUT);
    [compiled, adjacent].into_iter().find(|path| path.is_file())
}

pub(super) fn verify(artifact: &Path, game_data: &Path) -> Result<(), CommandError> {
    if !has_authored_panel(artifact)? { return Ok(()); }
    let bad_runtime = |path: &Path| CommandError::new("deploy_panel_runtime_source", path.display().to_string());
    let required_path = dependency_path(artifact)
        .ok_or_else(|| CommandError::new("deploy_panel_runtime_missing", artifact.display().to_string()))?;
    let required = read_resource(&required_path, PANEL_GAME_JS).ok().flatten()
        .ok_or_else(|| bad_runtime(&required_path))?;
    let base_path = game_data.join("SimCity_Game.package");
    let stock = read_resource(&base_path, PANEL_GAME_JS).ok().flatten()
        .ok_or_else(|| bad_runtime(&base_path))?;
    if !crate::asset_build_m1::panel::runtime_bundle::required_source_matches(&stock,&required) {
        return Err(bad_runtime(&base_path));
    }
    let mut installed = false;
    let mut conflicts = Vec::new();
    for path in crate::game_assets::package_paths(game_data) {
        if path == base_path { continue; }
        if let Ok(Some(script)) = read_resource(&path, PANEL_GAME_JS) {
            if crate::asset_build_m1::panel::runtime_bundle::compatible(&stock,&required,&script).is_ok() { installed = true; }
            else { conflicts.push(path.display().to_string()); }
        }
    }
    if !conflicts.is_empty() {
        conflicts.sort();
        return Err(CommandError::new("deploy_panel_runtime_conflict", conflicts.join("\n")));
    }
    if !installed {
        return Err(CommandError::new("deploy_panel_runtime_missing", required_path.display().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbpf::{write_uncompressed_overlay, OverlayEntry};
    use sc_properties::PropType;
    use std::fs;

    fn write(path: &Path, id: ResourceId, bytes: &[u8]) {
        fs::write(path, write_uncompressed_overlay(&[OverlayEntry::new(id, bytes.to_vec())]).unwrap()).unwrap();
    }

    #[test]
    fn detects_missing_mismatched_and_conflicting_runtime_before_deployment() {
        let root = std::env::temp_dir().join(format!("openscp-panel-preflight-{}", std::process::id()));
        let game = root.join("SimCityData");
        let output = root.join("output");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&output).unwrap();
        let artifact = output.join("building.package");
        let lot = ResourceId { type_id: 0x00B1_B104, group: 0x40E1_C000, instance: 1 };
        let mut asset = PropertyFile::default();
        crate::asset_build_m1::property_edit::set_scalar(&mut asset, 0x0D7E_ADAC, PropType::UInt32, Value::UInt32(hash("OpenSCPPanel_headerNone")));
        write(&artifact, lot, &asset.encode_canonical().unwrap());
        let stock = game.join("SimCity_Game.package");
        let companion = output.join("openscp-panel-runtime-v1.package");
        let installed = game.join("panel-runtime.package");
        let conflict = game.join("old-panel.package");
        write(&stock, PANEL_GAME_JS, b"stock");
        assert_eq!(verify(&artifact, &game).unwrap_err().code, "deploy_panel_runtime_missing");
        write(&companion, PANEL_GAME_JS, b"other version plus extension");
        assert_eq!(verify(&artifact, &game).unwrap_err().code, "deploy_panel_runtime_source");
        write(&companion, PANEL_GAME_JS, b"stock plus extension");
        assert_eq!(verify(&artifact, &game).unwrap_err().code, "deploy_panel_runtime_missing");
        write(&installed, PANEL_GAME_JS, b"stock plus extension");
        verify(&artifact, &game).unwrap();
        write(&conflict, PANEL_GAME_JS, b"stock plus obsolete extension");
        let error = verify(&artifact, &game).unwrap_err();
        assert_eq!(error.code, "deploy_panel_runtime_conflict");
        assert!(error.message.contains("old-panel.package"));
        // Different bytes with a validated compatible receipt must not force
        // rebuilding every building. Any opaque competing overlay still fails.
        let composed=crate::asset_build_m1::panel::runtime_bundle::compose(b"stock").unwrap();
        let compatible=String::from_utf8(composed.clone()).unwrap().replace("\"schema\":", "\"schema\" : ").into_bytes();
        write(&companion,PANEL_GAME_JS,&composed);
        write(&installed,PANEL_GAME_JS,&compatible);
        write(&conflict,PANEL_GAME_JS,&composed);
        verify(&artifact,&game).unwrap();
        write(&conflict,PANEL_GAME_JS,b"stock opaque unrelated overlay");
        assert_eq!(verify(&artifact,&game).unwrap_err().code,"deploy_panel_runtime_conflict");
        for file in [artifact, stock, companion, installed, conflict] { fs::remove_file(file).unwrap(); }
        for dir in [output, game, root] { fs::remove_dir(dir).unwrap(); }
    }
}
