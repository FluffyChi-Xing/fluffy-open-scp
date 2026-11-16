use super::*;
use crate::activity::{AppState, CommandError};
use std::collections::{BTreeMap, BTreeSet};

const MENU: u32 = 0x0987_8A01;
const MENU_UNIT: u32 = 0x0975_695F;
const MENU_GRAPHICS: u32 = 0x0E5D_9DE0;
const RW4: u32 = 0x2F4E_681B;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Lookup {
    game_data: PathBuf,
    scripts: PathBuf,
    menu: Option<String>,
    tgi: Option<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct BuildingChoice {
    name: String,
    unit: String,
    lot: String,
    primary: bool,
    matched: bool,
}

#[tauri::command]
pub(crate) async fn vanilla_variant_lookup(
    state: tauri::State<'_, AppState>,
    request: Lookup,
) -> Result<Vec<BuildingChoice>, CommandError> {
    let registry = crate::package_service::bundled_registry_path(&state.app);
    tauri::async_runtime::spawn_blocking(move || {
        let registry = registry.and_then(|path| sc_registry::Registry::open(path).ok());
        let scripts = resolve_scripts(&request.game_data, request.scripts)?;
        let packages = open_sources(&request.game_data, &scripts)?;
        lookup(
            &packages,
            request.menu.as_deref(),
            request.tgi.as_deref(),
            registry.as_ref(),
        )
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(|error| CommandError::new("vanilla_variant_lookup_failed", error))
}

fn scalar_key(file: &PropertyFile, hash: u32) -> Option<Key> {
    match file.get(hash)?.scalar()? {
        Value::Key(key) => Some(*key),
        _ => None,
    }
}

fn state_ids(file: &PropertyFile) -> impl Iterator<Item = ResourceId> + '_ {
    file.get(STATES)
        .and_then(Property::array)
        .into_iter()
        .flatten()
        .filter_map(|value| match value {
            Value::Key(key) if key.instance != 0 => Some(property_id(key.group, key.instance)),
            _ => None,
        })
}

fn is_scene(file: &PropertyFile) -> bool {
    MODELS.iter().any(|hash| scalar_key(file, *hash).is_some())
}

fn lookup(
    packages: &[Package],
    menu: Option<&str>,
    tgi: Option<&str>,
    registry: Option<&sc_registry::Registry>,
) -> Result<Vec<BuildingChoice>, String> {
    let mut index: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut properties = HashMap::new();
    for package in packages {
        for entry in package
            .entries()
            .iter()
            .filter(|entry| entry.id.type_id == TYPE)
        {
            index
                .entry(entry.id.instance)
                .or_default()
                .insert(entry.id.group);
            properties.entry(entry.id).or_insert((package, entry));
        }
    }
    let read = |id| {
        properties
            .get(&id)
            .and_then(|(package, entry)| read_property_entry(package, entry).ok())
    };
    let (menu_unit, preferred_group, needle) = match (menu, tgi) {
        (Some(menu), None) => {
            let menu = scene::parse_hex_u32(menu).ok_or("invalid menu instance")?;
            let Some(groups) = index.get(&menu) else {
                return Ok(Vec::new());
            };
            // Menu2 takes precedence. Some native building submenus use a
            // dedicated group, so inspect their explicit menu→Unit key too.
            let candidates = std::iter::once(MENU).chain(
                groups
                    .iter()
                    .copied()
                    .filter(|group| *group != MENU && *group != UNIT),
            );
            let Some(file) = candidates
                .filter_map(|group| read(property_id(group, menu)))
                .find(|file| scalar_key(file, MENU_UNIT).is_some())
            else {
                return Ok(Vec::new());
            };
            let target = scalar_key(&file, MENU_UNIT).unwrap();
            (
                Some(target.instance),
                scalar_key(&file, MENU_GRAPHICS).map(|key| key.group),
                None,
            )
        }
        (None, Some(input)) => {
            let parts = input
                .trim()
                .split(':')
                .map(scene::parse_hex_u32)
                .collect::<Option<Vec<_>>>()
                .ok_or("expected a complete TGI")?;
            let [type_id, group, instance] = parts.as_slice() else {
                return Err("expected a complete TGI".into());
            };
            if !matches!(*type_id, TYPE | RW4) {
                return Err("expected a Property or RW4 TGI".into());
            }
            let id = ResourceId {
                type_id: *type_id,
                group: *group,
                instance: *instance,
            };
            if !packages.iter().any(|package| package.entry(id).is_some()) {
                return Ok(Vec::new());
            }
            (None, None, Some(id))
        }
        _ => return Err("choose one building lookup mode".into()),
    };
    let mut choices = Vec::new();
    let instances = match menu_unit.or_else(|| {
        needle
            .filter(|id| id.type_id == TYPE && id.group == UNIT)
            .map(|id| id.instance)
    }) {
        Some(instance) => vec![instance],
        None => index
            .iter()
            .filter_map(|(instance, groups)| groups.contains(&UNIT).then_some(*instance))
            .collect(),
    };
    // Only actual gameplay Unit → NonSim roots are considered. Following an
    // arbitrary Parent edge would conflate all lots that share an RCI template.
    let mut model_hits = HashMap::new();
    for instance in instances {
        let unit_id = property_id(UNIT, instance);
        let Some(unit) = read(unit_id) else {
            continue;
        };
        let Some(primary) = scalar_key(&unit, LOT_LINK) else {
            continue;
        };
        if primary.instance != instance {
            continue;
        }
        let Some(groups) = index.get(&instance) else {
            continue;
        };
        let mut variants = Vec::new();
        for &group in groups.iter().filter(|group| **group != UNIT) {
            let lot_id = property_id(group, instance);
            let Some(lot) = read(lot_id) else {
                continue;
            };
            if !is_scene(&lot) {
                continue;
            }
            let primary = group == primary.group;
            // An alternate master owns a state table. Individual state
            // records may reuse the owner's instance, for example group 43E1C000.
            if !primary && lot.get(STATES).is_none() {
                continue;
            }
            let matched = match needle {
                Some(id) if id.type_id == TYPE => {
                    id == lot_id || state_ids(&lot).any(|state| state == id)
                }
                Some(id) => {
                    let references_model = |file: &PropertyFile| {
                        MODELS.iter().any(|hash| {
                            scalar_key(file, *hash).is_some_and(|key| {
                                key.instance == id.instance && key.group == id.group
                            })
                        })
                    };
                    references_model(&lot)
                        || state_ids(&lot).any(|state| {
                            *model_hits.entry(state).or_insert_with(|| {
                                read(state).is_some_and(|file| references_model(&file))
                            })
                        })
                }
                None => preferred_group == Some(group),
            };
            let name = registry
                .and_then(|r| r.instances().get(&instance))
                .map(|r| r.name.clone())
                .unwrap_or_else(|| format!("{instance:08X}"));
            variants.push(BuildingChoice {
                name,
                unit: unit_id.to_string(),
                lot: lot_id.to_string(),
                primary,
                matched,
            });
        }
        if menu_unit.is_some()
            || needle == Some(unit_id)
            || variants.iter().any(|item| item.matched)
        {
            choices.extend(variants);
        }
    }
    choices.sort_by(|a, b| {
        a.unit
            .cmp(&b.unit)
            .then(b.matched.cmp(&a.matched))
            .then(b.primary.cmp(&a.primary))
            .then(a.lot.cmp(&b.lot))
    });
    Ok(choices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_menu_and_state_tgi_resolve_the_owner_and_keep_distinct_appearances() {
        let unit = property_id(UNIT, 0x14FF_6F1D);
        let default = property_id(0x40B9_A10B, unit.instance);
        let alternate = property_id(0x40B9_A133, unit.instance);
        let shared_state = property_id(0x43E1_C000, 0x39B6_4608);
        let other_unit = property_id(UNIT, 0xDEAD_BEEF);
        let other_lot = property_id(default.group, other_unit.instance);
        let menu = property_id(MENU, 0x7C13_00D6);

        let entry = |id, scalar_keys: &[(u32, ResourceId)], state: Option<ResourceId>| {
            let mut file = PropertyFile::default();
            for &(hash, value) in scalar_keys {
                scalar(&mut file, hash, PropType::Key, Value::Key(key(value)));
            }
            if let Some(state) = state {
                file.values.push(scene::array_property(
                    STATES,
                    PropType::Key,
                    vec![Value::Key(key(state))],
                ));
            }
            OverlayEntry::new(id, encode(&file).unwrap())
        };
        let model = ResourceId {
            type_id: RW4,
            group: 0,
            instance: 0x39B6_4608,
        };
        let resources = [
            entry(unit, &[(LOT_LINK, default)], None),
            entry(default, &[(MODELS[0], model)], Some(default)),
            entry(alternate, &[(MODELS[0], model)], Some(shared_state)),
            entry(shared_state, &[(MODELS[0], model)], None),
            entry(other_unit, &[(LOT_LINK, other_lot)], None),
            entry(other_lot, &[(MODELS[0], model)], Some(other_lot)),
            entry(menu, &[(MENU_UNIT, unit), (MENU_GRAPHICS, alternate)], None),
            OverlayEntry::new(model, vec![0]),
        ];
        let path = std::env::temp_dir().join(format!(
            "openscp-vanilla-lookup-{}.package",
            std::process::id()
        ));
        fs::write(&path, dbpf::write_uncompressed_overlay(&resources).unwrap()).unwrap();
        let packages = [Package::open(&path).unwrap()];
        let menu_results = lookup(&packages, Some("7C1300D6"), None, None).unwrap();
        let tgi_results =
            lookup(&packages, None, Some("00b1b104:43e1c000:39b64608"), None).unwrap();
        let model_results =
            lookup(&packages, None, Some("2f4e681b:00000000:39b64608"), None).unwrap();
        drop(packages);
        fs::remove_file(path).unwrap();

        for results in [menu_results, tgi_results] {
            assert_eq!(results.len(), 2);
            assert!(results.iter().all(|result| result.unit == unit.to_string()));
            assert_eq!(results[0].lot, alternate.to_string());
            assert!(results[0].matched);
            assert_eq!(results[1].lot, default.to_string());
            assert!(results[1].primary);
        }
        assert_eq!(
            model_results
                .iter()
                .map(|result| &result.unit)
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
    }
}
