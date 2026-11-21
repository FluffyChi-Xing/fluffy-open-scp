//! Native decals cross Game dictionaries, Graphics rasters and App RW4 textures.
//! Resolve missing dependencies in a temporary scope, never the global package pool.
use crate::package_service::{DecalUnitTextureDto, PackageManager, resolve_decal_textures};
use dbpf::Package;
use std::{collections::BTreeSet, path::PathBuf};

pub(crate) fn resolve(
    properties: &sc_properties::PropertyFile,
    current: &Package,
    manager: &PackageManager,
    store: &sc_store::Store,
) -> (Vec<DecalUnitTextureDto>, Vec<String>) {
    let original = resolve_decal_textures(properties, current, manager);
    if original.0.iter().all(|texture| texture.png.is_some()) {
        return original;
    }
    let open = manager.all_packages_with_ids().unwrap_or_default();
    let mut dirs = BTreeSet::new();
    for package in std::iter::once(current).chain(open.iter().map(|(_, p)| p.as_ref())) {
        for dir in package.path().ancestors().skip(1).take(4) {
            dirs.insert(dir.to_path_buf());
            dirs.insert(dir.join("SimCityData"));
        }
    }
    if let Ok(Some(settings)) = store.app_settings() {
        if let Some(root) = settings.game_data_path {
            let root = PathBuf::from(root);
            dirs.insert(root.join("SimCityData"));
            dirs.insert(root);
        }
    }
    let mut seen: BTreeSet<_> = std::iter::once(current)
        .chain(open.iter().map(|(_, p)| p.as_ref()))
        .filter_map(|p| p.path().canonicalize().ok())
        .collect();
    let mut dependencies = Vec::new();
    for dir in dirs {
        for name in [
            "SimCity_Game.package",
            "SimCity_Graphics.package",
            "SimCity_App.package",
            "SimCityDataEP1.package",
            "SimCity_DLC0.package",
        ] {
            let path = dir.join(name);
            if let Ok(path) = path.canonicalize() {
                if seen.insert(path.clone()) {
                    if let Ok(package) = Package::open(&path) {
                        dependencies.push(package);
                    }
                }
            }
        }
    }
    if dependencies.is_empty() {
        return original;
    }
    let Ok(scope) = manager.preview_scope(dependencies) else {
        return original;
    };
    let (recovered, _) = resolve_decal_textures(properties, current, &scope);
    // Preserve already resolved entries and their source precedence exactly.
    let (mut textures, mut diagnostics) = original;
    for texture in &mut textures {
        if texture.png.is_some() {
            continue;
        }
        if let Some(replacement) = recovered.iter().find(|candidate| {
            candidate.category == texture.category
                && candidate.index == texture.index
                && candidate.id_instance == texture.id_instance
                && candidate.png.is_some()
        }) {
            *texture = replacement.clone();
            let prefix = format!("decal cat{}[{}]", texture.category, texture.index);
            diagnostics.retain(|message| !message.starts_with(&prefix));
        }
    }
    (textures, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires OPENSCP_DECAL_GAME_DATA pointing to original SimCityData"]
    fn native_holes_resolve_without_opening_app_package() {
        let root = PathBuf::from(std::env::var("OPENSCP_DECAL_GAME_DATA").unwrap());
        let manager = PackageManager::new();
        let (_, graphics) = manager
            .insert(Package::open(root.join("SimCity_Graphics.package")).unwrap())
            .unwrap();
        manager
            .insert(Package::open(root.join("SimCity_Game.package")).unwrap())
            .unwrap();
        let entry = graphics
            .entries()
            .iter()
            .find(|e| {
                e.id.type_id == 0x00b1b104
                    && e.id.group == 0x42e1c000
                    && e.id.instance == 0x788a5cc7
            })
            .unwrap();
        let properties =
            sc_properties::PropertyFile::parse(&graphics.read(entry).unwrap()).unwrap();
        let old = resolve_decal_textures(&properties, &graphics, &manager);
        let missing: Vec<_> = old
            .0
            .iter()
            .filter(|t| t.id_instance == 0xdcd50c4f)
            .collect();
        assert_eq!(missing.len(), 2);
        assert!(missing.iter().all(|t| t.png.is_none()));
        let store = sc_store::Store::open(":memory:").unwrap();
        let (textures, _) = resolve(&properties, &graphics, &manager, &store);
        assert_eq!(
            textures
                .iter()
                .filter(|t| t.id_instance == 0xdcd50c4f && t.png.is_some())
                .count(),
            2
        );
        assert!(
            textures
                .iter()
                .filter(|t| t.category == 0)
                .all(|t| t.png.is_some())
        );
        // A recovered preview must not add global handles or sidebar entries.
        assert_eq!(manager.all_packages_with_ids().unwrap().len(), 2);
        for texture in old.0.iter().filter(|t| t.png.is_some()) {
            let new = textures
                .iter()
                .find(|t| t.category == texture.category && t.index == texture.index)
                .unwrap();
            assert_eq!(texture.png, new.png);
        }
    }
}
