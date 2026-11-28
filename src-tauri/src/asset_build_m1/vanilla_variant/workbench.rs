use super::*;
use crate::activity::{AppState, CommandError};

const JOBS: [u32; 3] = [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7];
const JOBS_TOTAL: u32 = 0x0A79_17C3;
const FIELDS: [(u32, &str); 12] = [
    (0x0A8A_41DA, "constructionCost"),
    (0x09AE_19D7, "maintenanceCost"),
    (0x0A79_17C8, "minimumWorkers"),
    (0x0E3F_65A9, "workersW1"),
    (0x0E3F_65B3, "workersW2"),
    (0x0E3F_65B4, "workersW3"),
    (JOBS[0], "jobsW1"),
    (JOBS[1], "jobsW2"),
    (JOBS[2], "jobsW3"),
    (0x0C0F_20DB, "productionInterval"),
    (0x0C0F_20DC, "productionAmount"),
    (0x0AD7_FAC9, "studentCapacity"),
];

fn read_chain_property(chain: &[PropertyFile], hash: u32) -> Option<&Property> {
    chain.iter().find_map(|file| file.get(hash))
}

fn numeric(property: &Property) -> Option<f64> {
    match property.scalar()? {
        Value::Float(value) if value.is_finite() => Some(f64::from(*value)),
        Value::Int32(value) => Some(f64::from(*value)),
        Value::UInt32(value) => Some(f64::from(*value)),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Selection {
    project: String,
    game_data: PathBuf,
    scripts: PathBuf,
    unit: String,
    lot: String,
}

#[tauri::command]
pub(crate) async fn vanilla_variant_register(
    state: tauri::State<'_, AppState>,
    request: Selection,
) -> Result<Json, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = crate::workspace::code_root(&store, &request.project)
            .map_err(|error| error.to_string())?;
        register(&root, request)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(|error| CommandError::new("vanilla_variant_register_failed", error))
}

#[tauri::command]
pub(crate) async fn vanilla_variant_utility_fields(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<serde_json::Map<String, Json>, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let root =
            crate::workspace::code_root(&store, &project).map_err(|error| error.to_string())?;
        let manifest: Json = serde_json::from_slice(
            &fs::read(root.join("package.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        if manifest["mod_type"] != KIND {
            return Err("project is not a vanilla variant".into());
        }
        let variant = &manifest["vanillaVariant"];
        let game_data = PathBuf::from(variant["gameData"].as_str().ok_or("missing gameData")?);
        let scripts = resolve_scripts(
            &game_data,
            PathBuf::from(variant["scripts"].as_str().unwrap_or_default()),
        )?;
        let id = parse_tgi(variant["unit"].as_str().ok_or("missing Unit")?)?;
        let packages = open_sources(&game_data, &scripts)?;
        let unit = read_property(&packages, id)?;
        let chain = rci::inheritance(&packages, &unit)?;
        Ok(utilities::register(&packages, &chain))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(|error| CommandError::new("vanilla_variant_utility_fields_failed", error))
}

#[tauri::command]
pub(crate) async fn vanilla_variant_power_fields(
    state: tauri::State<'_, AppState>,
    project: String,
) -> Result<Json, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let root =
            crate::workspace::code_root(&store, &project).map_err(|error| error.to_string())?;
        let manifest: Json = serde_json::from_slice(
            &fs::read(root.join("package.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let variant = &manifest["vanillaVariant"];
        let Some(unit) = variant["unit"].as_str().filter(|_| manifest["mod_type"] == KIND) else {
            return Ok(Json::Null);
        };
        let id = parse_tgi(unit)?;
        if !power::supports(id) {
            return Ok(Json::Null);
        }
        let game_data = PathBuf::from(variant["gameData"].as_str().ok_or("missing gameData")?);
        let scripts = resolve_scripts(
            &game_data,
            PathBuf::from(variant["scripts"].as_str().unwrap_or_default()),
        )?;
        let packages = open_sources(&game_data, &scripts)?;
        let unit = read_property(&packages, id)?;
        let chain = rci::inheritance(&packages, &unit)?;
        power::describe(&packages, id, &chain)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(|error| CommandError::new("vanilla_variant_power_fields_failed", error))
}

fn register(root: &Path, request: Selection) -> Result<Json, String> {
    let _guard = crate::mod_flow::FLOW_LOCK
        .lock()
        .map_err(|e| e.to_string())?;
    let manifest_path = root.join("package.json");
    let pending = root.join(PENDING_PROJECT);
    let existing = manifest_path.is_file();
    let input = if existing { &manifest_path } else { &pending };
    let mut manifest: Json = serde_json::from_slice(&fs::read(input).map_err(|e| e.to_string())?)
        .map_err(|error| error.to_string())?;
    if manifest["format"] != "openscp.mod" || manifest["mod_type"] != KIND {
        return Err("project is not a vanilla variant".into());
    }
    let unit_id = parse_tgi(&request.unit)?;
    let lot_id = parse_tgi(&request.lot)?;
    let scripts = resolve_scripts(&request.game_data, request.scripts)?;
    let packages = open_sources(&request.game_data, &scripts)?;
    let unit = read_property(&packages, unit_id)?;
    let lot = read_property(&packages, lot_id)?;
    if unit_id.group != UNIT
        || !matches!(unit.get(LOT_LINK).and_then(Property::scalar),
        Some(Value::Key(key)) if key.instance == lot_id.instance)
    {
        return Err("selected Lot does not belong to the source Unit".into());
    }
    let chain = rci::inheritance(&packages, &unit)?;
    let source_package = packages
        .iter()
        .find(|package| package.entry(lot_id).is_some())
        .ok_or("could not locate source scene")?
        .path();
    let mut fields = utilities::register(&packages, &chain);
    for (hash, name) in FIELDS {
        if let Some(value) = read_chain_property(&chain, hash).and_then(numeric) {
            fields.insert(
                name.into(),
                json!({"hash":format!("{hash:08X}"),"original":value,"value":value}),
            );
        }
    }
    let registration = json!({
        "gameData": request.game_data,
        "scripts": scripts,
        "unit": unit_id.to_string(),
        "lot": lot_id.to_string(),
        "packagePath": source_package,
        "title": manifest["name"].as_str().unwrap_or("Variant"),
        "sceneDocument": "asset.lot.json",
        "utilityFieldsVersion": 1,
        "powerProduction": power::describe(&packages, unit_id, &chain)?,
        "fields": fields
    });
    let dependency_report = dependencies(&packages, [(unit_id, &unit), (lot_id, &lot)], &[])?;
    manifest["vanillaVariant"] = registration.clone();
    if manifest.get("dependencies").is_none() {
        manifest["dependencies"] = json!({});
    }
    manifest["dependencies"]["vanillaVariant"] = json!({
        "source": {"unit": unit_id.to_string(), "lot": lot_id.to_string()},
        "resources": dependency_report
    });
    let schema_relative = manifest["vanillaVariantSchema"]
        .as_str()
        .unwrap_or(SCHEMA_PATH);
    if !crate::mod_flow::safe_path(root, schema_relative)?.is_file() {
        crate::mod_flow::ensure_project_layout(root, KIND)?;
        initialize_project(
            root,
            manifest["name"].as_str().unwrap_or("Variant"),
            Some(&request.game_data),
        )?;
        manifest["vanillaVariantSchema"] = json!(SCHEMA_PATH);
    }
    crate::atomic_fs::write_atomic(
        &manifest_path,
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        existing,
    )
    .map_err(|error| error.to_string())?;
    if !existing {
        let _ = fs::remove_file(pending);
    }
    Ok(registration)
}

pub(super) fn apply(
    root: &Path,
    request: &Request,
    chain: &[PropertyFile],
    unit: &mut PropertyFile,
    lot: &mut PropertyFile,
) -> Result<Option<scene::SceneDocument>, String> {
    let Some(registration) = &request.registration else {
        return Ok(None);
    };
    utilities::apply(&registration["fields"], chain, unit)?;
    let mut jobs_changed = false;
    for (hash, name) in FIELDS {
        let Some(field) = registration["fields"].get(name) else {
            continue;
        };
        let base = read_chain_property(chain, hash)
            .ok_or_else(|| format!("missing source field {name}"))?;
        let value = field["value"]
            .as_f64()
            .filter(|value| *value >= 0.0)
            .ok_or_else(|| format!("invalid value for {name}"))?;
        if numeric(base) == Some(value) {
            continue;
        }
        let value = match base.prop_type {
            PropType::Float if value <= f64::from(f32::MAX) => Value::Float(value as f32),
            PropType::Int32 if value.fract() == 0.0 && value <= f64::from(i32::MAX) => {
                Value::Int32(value as i32)
            }
            PropType::UInt32 if value.fract() == 0.0 && value <= f64::from(u32::MAX) => {
                Value::UInt32(value as u32)
            }
            _ => return Err(format!("value out of range for {name}")),
        };
        scalar(unit, hash, base.prop_type, value);
        jobs_changed |= JOBS.contains(&hash);
    }
    if jobs_changed && read_chain_property(chain, JOBS_TOTAL).is_some() {
        let total: f64 = JOBS
            .into_iter()
            .filter_map(|hash| {
                unit.get(hash)
                    .or_else(|| read_chain_property(chain, hash))
                    .and_then(numeric)
            })
            .sum();
        scalar(
            unit,
            JOBS_TOTAL,
            PropType::Float,
            Value::Float(total as f32),
        );
    }
    let relative = registration["sceneDocument"]
        .as_str()
        .ok_or("missing variant sceneDocument")?;
    let path = crate::mod_flow::safe_path(root, relative)?;
    let raw = fs::read_to_string(path).map_err(|error| format!("variant scene: {error}"))?;
    let saved: Json = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let expected = parse_tgi(&request.lot)?;
    if saved["$schema"] != "openscp.lot-asset/1"
        || saved
            .pointer("/meta/source/tgi")
            .and_then(scene::parse_json_tgi)
            != Some(expected)
    {
        return Err("saved scene belongs to a different source Lot".into());
    }
    let document = scene::parse_document(&saved)?;
    let writes = scene::SceneWrites::parse(&document.writes);
    scene::apply_units(lot, &document, &writes)?;
    scene::verify_units(lot, &document, &writes)?;
    Ok(Some(document))
}

pub(super) fn ground(
    root: &Path,
    document: &scene::SceneDocument,
    lot: &mut PropertyFile,
    identity: u32,
) -> Result<OverlayEntry, String> {
    let (mut resources, exemplar) = super::super::stages::authored_ground_snapshot(root, document)?
        .ok_or("missing authored ground snapshot")?;
    let file = PropertyFile::parse(
        &resources
            .remove(&exemplar)
            .ok_or("missing authored ground properties")?,
    )
    .map_err(|error| error.to_string())?;
    let hash = sc_properties::LOT_MASK_HASH;
    let mask = file
        .get(hash)
        .and_then(Property::scalar)
        .ok_or("missing authored ground mask")?;
    let Value::Key(mask) = mask else {
        return Err("invalid authored ground mask key".into());
    };
    let data = resources
        .remove(&ResourceId {
            type_id: mask.type_id,
            group: mask.group,
            instance: mask.instance,
        })
        .ok_or("missing authored ground raster")?;
    let id = ResourceId {
        type_id: mask.type_id,
        group: mask.group,
        instance: identity,
    };
    for (hash, _) in scene::ground_values(document) {
        let property = file.get(hash).ok_or("missing authored ground column")?;
        lot.values.retain(|previous| previous.hash != hash);
        lot.values.push(property.clone());
    }
    scalar(lot, hash, PropType::Key, Value::Key(key(id)));
    Ok(OverlayEntry::new(id, data))
}
