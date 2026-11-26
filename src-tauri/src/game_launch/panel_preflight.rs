//! Preflight the shared Game.js dependency before copying an authored panel.
use crate::{activity::CommandError, asset_build_m1::panel::{hash, PANEL_GAME_JS, RUNTIME_OUTPUT}};
use dbpf::{Package, ResourceId};
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn read_resource(path: &Path, id: ResourceId) -> Result<Option<Vec<u8>>, String> {
    let package = Package::open(path).map_err(|error| error.to_string())?;
    package.entry(id).map(|entry| package.read(entry).map_err(|error| error.to_string())).transpose()
}

fn authored_bindings(artifact: &Path) -> Result<BTreeSet<u32>, CommandError> {
    let error = |message: String| CommandError::new("deploy_invalid_artifact", message);
    let package = Package::open(artifact).map_err(|reason| error(reason.to_string()))?;
    if let Some(entry) = package.entry(PANEL_GAME_JS)
        && package.read(entry).is_ok_and(|raw| raw.windows(b"OpenSCPPanelRuntimeVersion".len()).any(|w| w == b"OpenSCPPanelRuntimeVersion"))
    {
        return Err(CommandError::new("deploy_panel_runtime_separate", artifact.display().to_string()));
    }
    let mut bindings = BTreeSet::new();
    for entry in package.entries().iter().filter(|entry| entry.id.type_id == 0x00B1_B104 && entry.id.group == 0x40E1_C000) {
        let raw = package.read(entry).map_err(|reason| error(reason.to_string()))?;
        let Ok(asset) = PropertyFile::parse(&raw) else { continue; };
        let header = hash("OpenSCPPanel_headerNone");
        if asset.get(0x0D7E_ADAC).and_then(|property| property.scalar()) == Some(&Value::UInt32(header)) {
            bindings.insert(header);
            for field in [0x0CAB_B2FC, 0x0CAF_BDC3] {
                for binding in asset.get(field).and_then(|p| p.array()).unwrap_or_default() {
                    let Value::UInt32(binding) = binding else {
                        return Err(error(format!("Invalid authored panel binding {field:08X}")));
                    };
                    bindings.insert(*binding);
                }
            }
        }
    }
    Ok(bindings)
}

fn dependency_path(artifact: &Path) -> Option<PathBuf> {
    let output = artifact.parent()?;
    let adjacent = output.join("openscp-panel-runtime-v1.package");
    let compiled = output.parent()?.join(RUNTIME_OUTPUT);
    [compiled, adjacent].into_iter().find(|path| path.is_file())
}

pub(super) fn verify(artifact: &Path, game_data: &Path) -> Result<(), CommandError> {
    let bindings = authored_bindings(artifact)?;
    if bindings.is_empty() { return Ok(()); }
    let bad_runtime = |path: &Path| CommandError::new("deploy_panel_runtime_source", path.display().to_string());
    let base_path = game_data.join("SimCity_Game.package");
    let stock = read_resource(&base_path, PANEL_GAME_JS).ok().flatten()
        .ok_or_else(|| bad_runtime(&base_path))?;
    let legacy = if let Some(path) = dependency_path(artifact) {
        let required = read_resource(&path, PANEL_GAME_JS).ok().flatten()
            .ok_or_else(|| bad_runtime(&path))?;
        if !crate::asset_build_m1::panel::runtime_bundle::required_source_matches(&stock, &required) {
            return Err(bad_runtime(&base_path));
        }
        Some(required)
    } else {
        None
    };
    let mut providers = Vec::new();
    for path in crate::game_assets::package_paths(game_data) {
        if path != base_path && let Ok(Some(script)) = read_resource(&path, PANEL_GAME_JS) {
            providers.push((path, script));
        }
    }
    if providers.len() > 1 {
        providers.sort_by(|a, b| a.0.cmp(&b.0));
        let paths = providers.iter().map(|(path, _)| path.display().to_string()).collect::<Vec<_>>();
        return Err(CommandError::new("deploy_panel_runtime_conflict", paths.join("\n")));
    }
    let (path, script) = providers.pop().ok_or_else(|| {
        CommandError::new("deploy_panel_runtime_missing", game_data.display().to_string())
    })?;
    use crate::asset_build_m1::panel::runtime_bundle;
    if let Some(required) = legacy {
        runtime_bundle::compatible(&stock, &required, &script)
            .map_err(|_| CommandError::new("deploy_panel_runtime_conflict", path.display().to_string()))?;
    } else {
        runtime_bundle::verify_bindings(&stock, &script, &bindings).map_err(|e| {
            CommandError::new(
                if e == "PANEL_RUNTIME_CAPABILITY" { "deploy_panel_runtime_capability" } else { "deploy_panel_runtime_source" },
                path.display().to_string(),
            )
        })?;
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
        assert_eq!(verify(&artifact,&game).unwrap_err().code,"deploy_panel_runtime_conflict");
        write(&conflict, PANEL_GAME_JS, &compatible);
        assert_eq!(verify(&artifact, &game).unwrap_err().code, "deploy_panel_runtime_conflict");
        write(&conflict,PANEL_GAME_JS,b"stock opaque unrelated overlay");
        assert_eq!(verify(&artifact,&game).unwrap_err().code,"deploy_panel_runtime_conflict");
        fs::remove_file(&conflict).unwrap();
        fs::remove_file(&companion).unwrap();
        verify(&artifact, &game).unwrap();
        crate::asset_build_m1::property_edit::set_array(&mut asset, 0x0CAB_B2FC, PropType::UInt32, vec![Value::UInt32(42)]);
        write(&artifact, lot, &asset.encode_canonical().unwrap());
        assert_eq!(verify(&artifact, &game).unwrap_err().code, "deploy_panel_runtime_capability");
        assert_eq!(verify(&installed, &game).unwrap_err().code, "deploy_panel_runtime_separate");
        for file in [artifact, stock, installed] { fs::remove_file(file).unwrap(); }
        for dir in [output, game, root] { fs::remove_dir(dir).unwrap(); }
    }
}
