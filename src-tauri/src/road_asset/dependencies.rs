use super::{GRAPHICS, MENU, PROP, RoadSchema, SIM, property_id};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropertyFile, Value};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

fn id(type_id: u32, group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id,
        group,
        instance,
    }
}

fn candidates(owner: ResourceId, field: u32, key: Key) -> Vec<ResourceId> {
    let mut result = Vec::new();
    let instance = key.instance;
    if key.type_id != 0 {
        result.push(id(key.type_id, key.group, instance));
    }
    if field == sc_properties::PARENT_HASH {
        result.push(property_id(key.group, instance));
        result.push(property_id(owner.group, instance));
    } else if key.type_id == 0 && key.group != 0 {
        result.push(property_id(key.group, instance));
    }
    match field {
        // Road Menu2 → PathEntry and simulation upgrade/bridge/tunnel links.
        0xD13B_B4A6 | 0x0DA7_C920 | 0x0E09_30E9 | 0x0E09_30F1 | 0x0EC1_E846 => {
            result.extend([property_id(SIM, instance), property_id(GRAPHICS, instance)]);
        }
        0x0955_8827 | 0xBCC5_3802 | 0x0D8C_29C3 => {
            result.extend([
                property_id(0x0D94_2043, instance),
                property_id(0, instance),
                id(0x2F4E_681B, 0, instance),
            ]);
        }
        0x0E53_4076..=0x0E53_4078
        | 0x0A49_AEA4
        | 0x0A49_B270
        | 0x0A49_B275
        | 0x0A49_B278
        | 0x0EE9_943C
        | 0x1024_FA5A => {
            result.extend([id(0x2F4E_681C, 0, instance), id(0x2F4E_681B, 0, instance)]);
        }
        0x0C36_D30D => result.push(property_id(0x4000_2D00, instance)),
        _ => {}
    }
    result
}

pub(super) fn report(
    schema: &RoadSchema,
    game: &Package,
    scripts: &Package,
    entries: &[OverlayEntry],
    sha256: &str,
) -> Result<Json, String> {
    let base = game
        .path()
        .parent()
        .ok_or("road_game_package_parent_missing")?;
    let mut paths = std::fs::read_dir(base)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("SimCity") && name.ends_with(".package"))
        })
        .collect::<Vec<_>>();
    if let Ok(locales) = std::fs::read_dir(base.join("Locale")) {
        for locale in locales.filter_map(Result::ok) {
            if let Ok(files) = std::fs::read_dir(locale.path()) {
                paths.extend(
                    files
                        .filter_map(Result::ok)
                        .map(|file| file.path())
                        .filter(|path| path.extension().is_some_and(|ext| ext == "package")),
                );
            }
        }
    }
    if let Some((_, version)) = scripts
        .path()
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('_'))
    {
        if let Some(dir) = scripts
            .path()
            .parent()
            .and_then(|dir| std::fs::read_dir(dir).ok())
        {
            let suffix = format!("-Scripts_{version}.package");
            paths.extend(
                dir.filter_map(Result::ok)
                    .map(|file| file.path())
                    .filter(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.ends_with(&suffix))
                    }),
            );
        }
    }
    paths.sort();
    paths.dedup();
    let additional = paths
        .iter()
        .filter(|path| path.as_path() != game.path() && path.as_path() != scripts.path())
        .filter_map(|path| Package::open(path).ok())
        .collect::<Vec<_>>();
    let sources = [game, scripts]
        .into_iter()
        .chain(additional.iter())
        .collect::<Vec<_>>();
    let emitted: HashMap<_, _> = entries
        .iter()
        .map(|entry| (entry.id, entry.data.as_slice()))
        .collect();
    let (donor_menu, donor_path) = schema.recipe.donor();
    let mut queue = [
        property_id(MENU, donor_menu),
        property_id(GRAPHICS, donor_path),
        property_id(SIM, donor_path),
    ]
    .into_iter()
    .chain(emitted.keys().copied())
    .collect::<Vec<_>>();
    let mut visited = HashSet::new();
    let mut resources = BTreeMap::new();
    let mut unresolved = BTreeMap::new();
    let mut packages = BTreeSet::new();

    while let Some(owner) = queue.pop() {
        if !visited.insert(owner) {
            continue;
        }
        let label = owner.to_string();
        let external = sources
            .iter()
            .find_map(|package| package.entry(owner).map(|entry| (*package, entry)));
        let bytes = if let Some(bytes) = emitted.get(&owner) {
            resources.insert(label.clone(), json!({"tgi": label, "mode": "bundled"}));
            bytes.to_vec()
        } else if let Some((package, entry)) = external {
            let path = package.path().to_string_lossy();
            packages.insert(path.to_string());
            resources.insert(
                label.clone(),
                json!({"tgi": label, "mode": "external", "package": path}),
            );
            if !matches!(owner.type_id, PROP | 0x2F4E_681B) {
                continue;
            }
            match package.read(entry) {
                Ok(bytes) => bytes,
                Err(error) => {
                    unresolved.insert(
                        format!("{label}:payload"),
                        json!({
                            "from": label, "reference": "DBPF", "detail": error.to_string(),
                        }),
                    );
                    continue;
                }
            }
        } else {
            continue;
        };
        let exists = |dependency| {
            emitted.contains_key(&dependency)
                || sources
                    .iter()
                    .any(|package| package.entry(dependency).is_some())
        };
        if owner.type_id == 0x2F4E_681B {
            let models = HashMap::from([(owner, bytes)]);
            let edges = match crate::asset_build_m1::resource_closure::discover(&models, false) {
                Ok(edges) => edges,
                Err(error) => {
                    unresolved.insert(
                        format!("{label}:materials"),
                        json!({
                            "from": label, "reference": "RW4", "detail": error,
                        }),
                    );
                    continue;
                }
            };
            for edge in edges {
                let targets = edge
                    .candidates
                    .iter()
                    .copied()
                    .filter(|target| exists(*target))
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    unresolved.insert(
                        format!("{label}:{}", edge.label),
                        json!({
                            "from": label, "reference": edge.label,
                        }),
                    );
                }
                queue.extend(targets);
            }
            continue;
        }
        if owner.type_id != PROP {
            continue;
        }
        let file = match PropertyFile::parse(&bytes) {
            Ok(file) => file,
            Err(error) => {
                unresolved.insert(
                    format!("{label}:property"),
                    json!({
                        "from": label, "reference": "Property", "detail": error.to_string(),
                    }),
                );
                continue;
            }
        };
        for property in file.values {
            let values = match property.kind {
                Kind::Scalar(value) => vec![value],
                Kind::Array(values) => values,
                Kind::Empty => continue,
            };
            for value in values {
                let (reference, candidates) = match value {
                    Value::Key(key) if key.instance != 0 => (
                        format!("{:08X}:{:08X}:{:08X}", key.type_id, key.group, key.instance),
                        candidates(owner, property.hash, key),
                    ),
                    Value::Text(text) if text.table_id != 0 => (
                        format!("{:08X}:{:08X}", text.table_id, text.instance_id),
                        vec![id(0x0A98_EAF0, 0x02FA_BF01, text.table_id)],
                    ),
                    _ => continue,
                };
                let targets = candidates
                    .into_iter()
                    .filter(|target| exists(*target))
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    let field = format!("{:08X}", property.hash);
                    unresolved.insert(
                        format!("{label}:{field}:{reference}"),
                        json!({
                            "from": label, "field": field, "reference": reference,
                        }),
                    );
                }
                queue.extend(targets);
            }
        }
    }
    Ok(json!({
        "packageSha256": sha256,
        "packages": packages,
        "resources": resources.into_values().collect::<Vec<_>>(),
        "unresolved": unresolved.into_values().collect::<Vec<_>>(),
    }))
}
