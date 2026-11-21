//! Builds new placeable identities from installed assets, bypassing geometry compilation.
mod rci;
mod power;
pub(crate) mod selection;
mod utilities;
pub(crate) mod workbench;
use super::{
    M1BuildResult, connector, events, filesystem::Staging, menu, package::SceneSemanticSummary,
    scene,
};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::lot_unit as lot_hash;
use sc_properties::{Key, Kind, PropType, Property, PropertyFile, Value};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

const TYPE: u32 = 0x00B1_B104;
const UNIT: u32 = 0x40E0_C000;
const PARENT: u32 = 0x00B2_CCCB;
const LOT_LINK: u32 = 0x08E1_7ED7;
const STATES: u32 = 0x0CA5_CF4A;
const KIND: &str = "vanilla_variant";
pub(crate) const SCHEMA_PATH: &str = "schemas/vanilla_variant/asset.schema.json";
pub(crate) const PENDING_PROJECT: &str = ".vanilla-variant-project.json";
const MODELS: [u32; 4] = sc_properties::LOD_MODEL_HASHES;

pub(crate) fn initialize_project(
    root: &Path,
    title: &str,
    game_data: Option<&Path>,
) -> Result<(), String> {
    let scripts = game_data.and_then(|path| resolve_scripts(path, PathBuf::new()).ok());
    let schema = json!({
        "mod_type": KIND,
        "vanillaVariant": {
            "gameData": game_data.map(|path| path.to_string_lossy()).unwrap_or_default(),
            "scripts": scripts.as_deref().map(|path| path.to_string_lossy()).unwrap_or_default(),
            "unit": "",
            "lot": "",
            "title": title,
            "constructionCost": 0,
            "maintenanceCost": 0,
            "sceneEdits": []
        }
    });
    crate::atomic_fs::write_atomic(
        &root.join(SCHEMA_PATH),
        &serde_json::to_vec_pretty(&schema).map_err(|error| error.to_string())?,
        false,
    )
    .map_err(|error| error.to_string())
}

pub fn project_schema(root: &Path) -> Result<Option<(String, String)>, String> {
    let Ok(bytes) = fs::read(root.join("package.json")) else {
        return Ok(None);
    };
    let Ok(manifest) = serde_json::from_slice::<Json>(&bytes) else {
        return Ok(None);
    };
    if manifest["mod_type"].as_str() != Some(KIND) {
        return Ok(None);
    }
    let relative = manifest["vanillaVariantSchema"]
        .as_str()
        .unwrap_or(SCHEMA_PATH);
    let path = crate::mod_flow::safe_path(root, relative)?;
    let raw = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    preflight(root, &raw)?;
    Ok(Some((relative.to_owned(), raw)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Request {
    game_data: PathBuf,
    scripts: PathBuf,
    unit: String,
    lot: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon_png: Option<String>,
    #[serde(default)]
    marquee_png: Option<String>,
    construction_cost: u32,
    maintenance_cost: u32,
    #[serde(default)]
    scene_edits: Vec<SceneEdit>,
    #[serde(default)]
    lot_mask: Option<String>,
    #[serde(skip)]
    registration: Option<Json>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SceneEdit {
    LightIntensity { row: usize, value: f32 },
    PropPosition { set: u8, row: usize, xyz: [f32; 3] },
    EffectPosition { row: usize, xyz: [f32; 3] },
}

pub(super) fn preflight(root: &Path, raw: &str) -> Result<Option<Request>, String> {
    let schema: Json = serde_json::from_str(raw).map_err(|e| format!("invalid build JSON: {e}"))?;
    let declared = schema.get("mod_type").and_then(Json::as_str);
    let manifest = fs::read(root.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Json>(&bytes).ok());
    let project_kind = manifest.as_ref().and_then(|p| p["mod_type"].as_str());
    if declared != Some(KIND) && project_kind != Some(KIND) {
        return Ok(None);
    }
    if project_kind != Some(KIND)
        || declared.is_some_and(|value| value != KIND)
        || manifest.as_ref().and_then(|p| p["format"].as_str()) != Some("openscp.mod")
    {
        return Err(
            "vanilla variant requires an openscp.mod package.json with matching mod_type".into(),
        );
    }
    if schema.get("vanillaVariant").is_none() {
        return Err("vanilla variant schema is missing vanillaVariant".into());
    }
    let mut values = schema["vanillaVariant"].clone();
    let registration = manifest
        .as_ref()
        .and_then(|manifest| manifest.get("vanillaVariant"));
    if let Some(source) = registration {
        for field in [
            "gameData", "scripts", "unit", "lot", "title", "description", "iconPng",
            "marqueePng",
        ] {
            values[field] = source[field].clone();
        }
    }
    let mut request: Request =
        serde_json::from_value(values).map_err(|e| format!("invalid vanillaVariant: {e}"))?;
    request.registration = registration.cloned();
    Ok(Some(request))
}

fn property_id(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: TYPE,
        group,
        instance,
    }
}

fn parse_tgi(text: &str) -> Result<ResourceId, String> {
    let fields = text
        .split(':')
        .map(|s| u32::from_str_radix(s, 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    match fields.as_slice() {
        [TYPE, group, instance] => Ok(property_id(*group, *instance)),
        _ => Err(format!("expected a full Property TGI: {text}")),
    }
}

fn read_property(packages: &[Package], id: ResourceId) -> Result<PropertyFile, String> {
    let (package, entry) = packages
        .iter()
        .find_map(|p| p.entry(id).map(|e| (p, e)))
        .ok_or_else(|| format!("missing source Property {id}"))?;
    read_property_entry(package, entry)
}

fn read_property_entry(package: &Package, entry: &dbpf::IndexEntry) -> Result<PropertyFile, String> {
    let id = entry.id;
    if u64::from(entry.decompressed_size) > crate::package_service::RESOURCE_DATA_MAX {
        return Err(format!("source Property too large: {id}"));
    }
    let bytes = package.read(entry).map_err(|e| e.to_string())?;
    PropertyFile::parse(&bytes).map_err(|e| format!("{id}: {e}"))
}

fn source_paths(game_data: &Path, scripts: &Path) -> [PathBuf; 6] {
    [
        scripts.to_path_buf(),
        game_data.join("SimCity_Game.package"),
        game_data.join("SimCity_Graphics.package"),
        game_data.join("SimCityDataEP1.package"),
        game_data.join("SimCity_App.package"),
        game_data.join("SimCity_DLC0.package"),
    ]
}

fn resolve_scripts(game_data: &Path, scripts: PathBuf) -> Result<PathBuf, String> {
    if !scripts.as_os_str().is_empty() {
        return Ok(scripts);
    }
    let parent = game_data.parent().ok_or("invalid SimCityData directory")?;
    let directory = parent.join("SimCityUserData/EcoGame");
    fs::read_dir(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let version = path
                .file_name()?
                .to_str()?
                .strip_prefix("SimCity-Scripts_")?
                .strip_suffix(".package")?
                .parse::<u64>()
                .ok()?;
            Some((version, path))
        })
        .max()
        .map(|(_, path)| path)
        .ok_or_else(|| "select the original Scripts package for this game installation".into())
}

fn open_sources(game_data: &Path, scripts: &Path) -> Result<Vec<Package>, String> {
    if !scripts.is_file() || !game_data.join("SimCity_Game.package").is_file() {
        return Err("select the original Scripts package and SimCityData directory".into());
    }
    source_paths(game_data, scripts)
        .iter()
        .filter(|path| path.is_file())
        .map(|path| Package::open(path).map_err(|error| format!("{}: {error}", path.display())))
        .collect()
}

fn encode(file: &PropertyFile) -> Result<Vec<u8>, String> {
    file.encode_canonical().map_err(|e| e.to_string())
}

fn scalar(file: &mut PropertyFile, hash: u32, prop_type: PropType, value: Value) {
    file.values.retain(|property| property.hash != hash);
    file.values.push(Property {
        hash,
        prop_type,
        kind: Kind::Scalar(value),
        encoding: Default::default(),
    });
}

fn row(file: &mut PropertyFile, hash: u32, index: usize) -> Result<&mut Value, String> {
    file.values
        .iter_mut()
        .find(|property| property.hash == hash)
        .and_then(|property| match &mut property.kind {
            Kind::Array(rows) => rows.get_mut(index),
            _ => None,
        })
        .ok_or_else(|| format!("missing scene column {hash:08X} row {index}"))
}

fn translate(
    file: &mut PropertyFile,
    hash: u32,
    index: usize,
    xyz: [f32; 3],
) -> Result<(), String> {
    let Value::Transform(transform) = row(file, hash, index)? else {
        return Err(format!("scene column {hash:08X} must be a Transform array"));
    };
    let offset = match transform.matrix.len() {
        3 | 4 => 0,
        12 => 9,
        _ => return Err("invalid transform layout".into()),
    };
    transform.matrix[offset..offset + 3].copy_from_slice(&xyz);
    Ok(())
}

fn apply_edits(lot: &mut PropertyFile, edits: &[SceneEdit]) -> Result<(), String> {
    for edit in edits {
        match *edit {
            SceneEdit::LightIntensity { row: index, value }
                if value.is_finite() && value >= 0.0 =>
            {
                let Value::Float(intensity) = row(lot, lot_hash::LIGHT_DIFFUSE_LEVELS, index)?
                else {
                    return Err("light intensity must be Float".into());
                };
                *intensity = value;
            }
            SceneEdit::PropPosition { set, row, xyz }
                if usize::from(set) < lot_hash::PROP_BINS && xyz.iter().all(|v| v.is_finite()) =>
            {
                translate(
                    lot,
                    lot_hash::PROP_TRANSFORM_BASE + u32::from(set),
                    row,
                    xyz,
                )?;
            }
            SceneEdit::EffectPosition { row, xyz } if xyz.iter().all(|v| v.is_finite()) => {
                translate(lot, lot_hash::EFFECT_TRANSFORMS, row, xyz)?;
            }
            _ => return Err("invalid scene edit".into()),
        }
    }
    Ok(())
}

fn key(id: ResourceId) -> Key {
    Key {
        type_id: id.type_id,
        group: id.group,
        instance: id.instance,
    }
}

fn route_event(
    log: &mut Vec<events::StageEvent>,
    progress: &Option<Sender<events::StageEvent>>,
    event: events::StageEvent,
) {
    if let Some(progress) = progress {
        let _ = progress.send(event.clone());
    }
    log.push(event);
}

pub(super) fn build(
    root: &Path,
    raw: &str,
    request: Request,
    progress: Option<Sender<events::StageEvent>>,
) -> Result<M1BuildResult, String> {
    let started = events::now_ms();
    let mut log = Vec::new();
    route_event(
        &mut log,
        &progress,
        events::started("vanilla-variant", Json::Null),
    );
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let unit_id = parse_tgi(&request.unit)?;
    let lot_id = parse_tgi(&request.lot)?;
    if unit_id.group != UNIT || request.title.trim().is_empty() {
        return Err("a simulation Unit and a nonempty variant title are required".into());
    }
    if request.construction_cost > 16_777_216 || request.maintenance_cost > 16_777_216 {
        return Err("costs must be representable exactly as Float32".into());
    }
    let packages = open_sources(&request.game_data, &request.scripts)?;
    let mut unit = read_property(&packages, unit_id)?;
    let mut lot = read_property(&packages, lot_id)?;
    let source = json!({
        "unit": { "tgi": unit_id.to_string(), "canonicalPropertySha256": digest(&encode(&unit)?) },
        "lot": { "tgi": lot_id.to_string(), "canonicalPropertySha256": digest(&encode(&lot)?) }
    });
    let link = unit.get(LOT_LINK).and_then(Property::scalar);
    if !matches!(link, Some(Value::Key(k)) if k.instance == lot_id.instance) {
        return Err("selected graphics variant does not belong to the source Unit".into());
    }
    let mut collision_index = connector::Resources::new();
    for id in packages
        .iter()
        .flat_map(|p| p.entries().iter().map(|entry| entry.id))
    {
        if id.type_id == TYPE {
            collision_index.insert(id, Vec::new());
        }
    }
    collision_index.insert(lot_id, encode(&lot)?);
    for path in [&request.icon_png, &request.marquee_png].into_iter().flatten() {
        if !path.trim().is_empty() {
            crate::mod_flow::safe_path(&root, path)?;
        }
    }
    let spec = serde_json::from_value(json!({
        "title": request.title, "description": request.description, "categoryInstance": "07049250",
        "menuParentInstance": "F970A822", "buildingCategoryInstance": "C710B6E9",
        "iconPng": request.icon_png, "marqueePng": request.marquee_png
    }))
    .map_err(|e| format!("park menu specification: {e}"))?;
    let generated = menu::generate(
        &spec,
        &collision_index,
        lot_id,
        None,
        &scene::SceneWrites::default(),
        None,
        None,
        None,
        &root,
        false,
    )?;
    let chain = rci::inheritance(&packages, &unit)?;
    let growable = rci::is_growable(&chain, lot_id);
    if growable {
        unit = rci::materialize_unit(&packages, unit, &chain)?;
    }
    let new_unit = property_id(UNIT, generated.instance);
    let synthetic_lot = property_id(menu::lot_group(), generated.instance);
    let new_lot = if growable {
        property_id(lot_id.group, generated.instance)
    } else {
        synthetic_lot
    };
    scalar(&mut unit, LOT_LINK, PropType::Key, Value::Key(key(new_lot)));
    let workbench_scene = if request.registration.is_none() {
        scalar(
            &mut unit,
            0x0A8A_41DA,
            PropType::Float,
            Value::Float(request.construction_cost as f32),
        );
        scalar(
            &mut unit,
            0x09AE_19D7,
            PropType::Float,
            Value::Float(request.maintenance_cost as f32),
        );
        apply_edits(&mut lot, &request.scene_edits)?;
        None
    } else {
        workbench::apply(&root, &request, &chain, &mut unit, &mut lot)?
    };
    let mut output: Vec<_> = generated
        .entries
        .into_iter()
        .filter(|(id, _)| *id != new_unit && *id != synthetic_lot)
        .map(|(id, bytes)| OverlayEntry::new(id, bytes))
        .collect();
    if let Some(document) = workbench_scene
        .as_ref()
        .filter(|document| matches!(document.lot_mask, Some(scene::SceneLotMaskKind::Authored)))
    {
        output.push(workbench::ground(
            &root,
            document,
            &mut lot,
            generated.instance,
        )?);
    } else if let Some(path) = request.lot_mask.as_deref() {
        let payload =
            fs::read(super::filesystem::project_file(&root, path)?).map_err(|e| e.to_string())?;
        let raster = ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: generated.instance,
        };
        validate_lot_mask(&packages, &lot, &payload)?;
        scalar(
            &mut lot,
            sc_properties::LOT_MASK_HASH,
            PropType::Key,
            Value::Key(key(raster)),
        );
        output.push(OverlayEntry::new(raster, payload));
    }
    if let Some(property) = lot.values.iter_mut().find(|p| p.hash == STATES) {
        let Kind::Array(states) = &mut property.kind else {
            return Err("invalid graphics state table".into());
        };
        for state in states {
            let Value::Key(reference) = state else {
                return Err("invalid graphics state key".into());
            };
            if reference.instance == lot_id.instance && reference.group == lot_id.group {
                *reference = key(new_lot);
            }
        }
    }
    // Replace the synthetic names on both leaves with the new menu's own
    // localization reference instead of retaining the source display name.
    let menu_bytes = output
        .iter()
        .find(|e| e.id.type_id == TYPE && e.id.instance == generated.instance)
        .ok_or("generated menu missing")?;
    let menu_file = PropertyFile::parse(&menu_bytes.data).map_err(|e| e.to_string())?;
    if let Some(name) = menu_file
        .get(menu::HASH_TITLE)
        .and_then(|p| p.scalar().or_else(|| p.array()?.first()))
    {
        if matches!(name, Value::Text(_)) {
            for file in [&mut unit, &mut lot] {
                file.values.retain(|p| p.hash != 0x09FB_78CB);
                file.values.push(Property {
                    hash: 0x09FB_78CB,
                    prop_type: PropType::Text,
                    kind: Kind::Array(vec![name.clone()]),
                    encoding: Default::default(),
                });
            }
        }
    }
    let rci_overrides = if growable {
        rci::prepare(&packages, &chain, &mut unit, &lot, new_lot, &mut output)?
    } else {
        Vec::new()
    };
    let power_edits = if let Some(registration) = &request.registration {
        power::apply(&packages, unit_id, new_unit, &registration["powerProduction"], &mut unit, &mut output)?
    } else {
        Vec::new()
    };
    output.push(OverlayEntry::new(new_unit, encode(&unit)?));
    output.push(OverlayEntry::new(new_lot, encode(&lot)?));
    let mut dependency_report = json!({
        "source": source,
        "rciBundleOverrides": rci_overrides,
        "resources": dependencies(&packages, [(new_unit, &unit), (new_lot, &lot)], &output)?
    });
    if !power_edits.is_empty() {
        dependency_report["powerProduction"] = json!(power_edits);
    }
    let mut installed = crate::game_assets::package_paths(&request.game_data);
    installed.extend(
        source_paths(&request.game_data, &request.scripts)
            .into_iter()
            .filter(|p| p.is_file()),
    );
    installed.sort();
    installed.dedup();
    for path in installed {
        let package = Package::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(entry) = output
            .iter()
            .find(|entry| package.entry(entry.id).is_some())
        {
            return Err(format!(
                "vanilla variant would overwrite {} in {}",
                entry.id,
                path.display()
            ));
        }
    }
    let bytes = dbpf::write_uncompressed_overlay(&output).map_err(|e| e.to_string())?;
    let sha256 = digest(&bytes);
    let schema_hash = digest(raw.as_bytes());
    let report_bytes = serde_json::to_vec_pretty(&dependency_report).map_err(|e| e.to_string())?;
    let source_hash = digest(&report_bytes);
    let build_hash = digest(format!("{schema_hash}{source_hash}{sha256}").as_bytes());
    let staged = Staging::create(&root, &build_hash)?;
    let relative = format!("packages/OpenSCP_{:08X}.package", generated.instance);
    staged.write(&relative, &bytes)?;
    staged.write("dependencies.json", &report_bytes)?;
    staged.publish()?;
    let manifest_path = root.join("package.json");
    let mut manifest: Json =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if manifest.get("dependencies").is_none() {
        manifest["dependencies"] = json!({});
    }
    let manifest_deps = manifest["dependencies"]
        .as_object_mut()
        .ok_or("dependencies must be an object")?;
    manifest_deps.insert("vanillaVariant".into(), dependency_report);
    crate::atomic_fs::write_atomic(
        &manifest_path,
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        true,
    )
    .map_err(|e| e.to_string())?;
    let finished = events::now_ms();
    route_event(
        &mut log,
        &progress,
        events::finished(
            "vanilla-variant",
            json!({"modelCompilation": false}),
            finished - started,
        ),
    );
    Ok(M1BuildResult {
        panel_runtime: None,
        path: staged.destination.join(relative),
        schema_hash,
        build_hash,
        source_hash,
        sha256,
        resource_count: output.len(),
        delta_mode: true,
        menu_instance: Some(generated.instance),
        patched_resources: 2,
        scope: KIND,
        scene: SceneSemanticSummary::default(),
        events: log,
        started_at_ms: started,
        finished_at_ms: finished,
        duration_ms: finished - started,
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_lot_mask(packages: &[Package], lot: &PropertyFile, bytes: &[u8]) -> Result<(), String> {
    let Some(Value::Key(mask)) = lot
        .get(sc_properties::LOT_MASK_HASH)
        .and_then(Property::scalar)
    else {
        return Err("source LotMask is missing".into());
    };
    let source = packages
        .iter()
        .find_map(|p| {
            let id = crate::package_service::find_raster_entry(p, *mask)?;
            p.read(p.entry(id)?).ok()
        })
        .ok_or("source LotMask raster is missing")?;
    let original = rw4::RasterImage::parse(&source).map_err(|e| e.to_string())?;
    let replacement = rw4::RasterImage::parse(bytes).map_err(|e| e.to_string())?;
    if (original.width, original.height, original.pixel_format)
        != (
            replacement.width,
            replacement.height,
            replacement.pixel_format,
        )
    {
        return Err("replacement LotMask must preserve the original dimensions and format".into());
    }
    replacement
        .decode_top_mip_rgba()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn dependencies(
    packages: &[Package],
    roots: [(ResourceId, &PropertyFile); 2],
    output: &[OverlayEntry],
) -> Result<Vec<Json>, String> {
    use sc_properties::swarm::{EFFECT_COLLECTION_TYPE, SwarmCollection};

    let mut effect_exports = HashMap::new();
    let mut decal_entries = HashMap::new();
    for package in packages {
        for entry in package.entries() {
            if u64::from(entry.decompressed_size) > crate::package_service::RESOURCE_DATA_MAX {
                continue;
            }
            if entry.id.type_id == EFFECT_COLLECTION_TYPE {
                if let Ok(bytes) = package.read(entry)
                    && let Ok(collection) = SwarmCollection::parse(&bytes)
                {
                    for export in collection.exports {
                        effect_exports.entry(export.name_hash).or_insert(entry.id);
                    }
                }
            } else if entry.id.type_id == TYPE
                && sc_properties::decal::is_decal_dictionary_group(entry.id.group)
                && let Ok(bytes) = package.read(entry)
                && let Ok(dict) = sc_properties::DecalDictionary::parse(&bytes)
            {
                for decal in dict.entries {
                    if let Some(id) = decal.id {
                        decal_entries
                            .entry(id.instance)
                            .or_insert((entry.id, decal.raster));
                    }
                }
            }
        }
    }
    let mut edges = Vec::new();
    let mut queue = roots
        .into_iter()
        .map(|(id, file)| (id, file.clone()))
        .collect::<Vec<_>>();
    let mut visited = std::collections::HashSet::new();
    while let Some((owner, file)) = queue.pop() {
        if !visited.insert(owner) {
            continue;
        }
        if visited.len() > 4096 {
            return Err("variant property dependency graph exceeds limit".into());
        }
        for property in &file.values {
            let values = match &property.kind {
                Kind::Scalar(value) => std::slice::from_ref(value),
                Kind::Array(values) => values.as_slice(),
                Kind::Empty => continue,
            };
            for (index, value) in values.iter().enumerate() {
                let Value::Key(reference) = value else {
                    continue;
                };
                if reference.instance == 0 {
                    continue;
                }
                let hash = property.hash;
                let decal_field = (lot_hash::DECAL_ID_BASE
                    ..lot_hash::DECAL_ID_BASE + lot_hash::DECAL_CATEGORIES as u32)
                    .contains(&hash);
                let prop_slot = (lot_hash::PROP_ID_BASE
                    ..lot_hash::PROP_ID_BASE + lot_hash::PROP_BINS as u32)
                    .contains(&hash)
                    || (lot_hash::PROP_SLOT_BASE
                        ..lot_hash::PROP_SLOT_BASE + lot_hash::PROP_BINS as u32)
                        .contains(&hash);
                let dependency_kind = match hash {
                    PARENT => "inheritance",
                    LOT_LINK | STATES => "graphics",
                    0x0A02_09A2..=0x0A02_09A5 => "behavior",
                    lot_hash::EFFECT_IDS | lot_hash::EFFECT_REF_IDS | lot_hash::EFFECT_WORLDS => {
                        "effect"
                    }
                    lot_hash::SPAWNER_IDS | lot_hash::SPAWNER_AGENT => "spawner",
                    _ if decal_field => "decal",
                    _ if prop_slot => "prop",
                    _ if MODELS.contains(&hash) => "model",
                    _ if hash == sc_properties::LOT_MASK_HASH => "ground",
                    _ => "propertyKey",
                };
                let decal = decal_field
                    .then(|| decal_entries.get(&reference.instance))
                    .flatten();
                let typed = match hash {
                    PARENT | LOT_LINK | STATES => {
                        Some(property_id(reference.group, reference.instance))
                    }
                    0x0A02_09A2..=0x0A02_09A5 => Some(property_id(
                        if reference.group == 0 {
                            0x40E0_C700
                        } else {
                            reference.group
                        },
                        reference.instance,
                    )),
                    lot_hash::EFFECT_IDS => effect_exports.get(&reference.instance).copied(),
                    _ if decal_field => decal.map(|(id, _)| *id),
                    _ if MODELS.contains(&hash) || reference.type_id != 0 => Some(ResourceId {
                        type_id: reference.type_id,
                        group: reference.group,
                        instance: reference.instance,
                    }),
                    _ => None,
                };
                let locate = |id| packages.iter().find_map(|p| p.entry(id).map(|e| (p, e)));
                let target = if hash == sc_properties::LOT_MASK_HASH {
                    packages
                        .iter()
                        .find_map(|p| crate::package_service::find_raster_entry(p, *reference))
                        .or(typed)
                } else {
                    typed
                };
                let bundled = target.is_some_and(|id| output.iter().any(|e| e.id == id));
                let resolved = target.and_then(locate);
                if let Some((package, entry)) = resolved {
                    if entry.id.type_id == TYPE && !decal_field && !visited.contains(&entry.id) {
                        queue.push((entry.id, read_property(packages, entry.id)?));
                    }
                    let mut edge = json!({"from":owner.to_string(),"field":format!("{hash:08X}"),"row":index,
                        "kind":dependency_kind,"to":entry.id.to_string(),"package":package.path().file_name(),"mode":"vanilla"});
                    if let Some((_, Some(raster))) = decal {
                        edge["raster"] = json!(packages.iter()
                            .find_map(|package| crate::package_service::find_raster_entry(package, *raster)
                                .map(|id| json!({"tgi":id.to_string(),"package":package.path().file_name()}))));
                    }
                    edges.push(edge);
                } else {
                    // Original ancestors and alternate graphics records can contain
                    // dormant model and mask references absent from this installation.
                    let direct = roots.iter().any(|(id, _)| *id == owner);
                    let required = matches!(hash, PARENT | LOT_LINK | STATES)
                        || direct
                            && (matches!(hash, 0x0A02_09A2 | 0x0A02_09A5)
                                || MODELS.contains(&hash)
                                || hash == sc_properties::LOT_MASK_HASH);
                    if required && !bundled {
                        return Err(format!(
                            "unresolved required field {hash:08X} on {owner}: {:08X}",
                            reference.instance
                        ));
                    }
                    edges.push(json!({"from":owner.to_string(),"field":format!("{hash:08X}"),"row":index,
                        "key":format!("{:08X}:{:08X}:{:08X}",reference.type_id,reference.group,reference.instance),
                        "kind":dependency_kind,
                        "mode":if bundled {"variant"}
                            else if prop_slot || matches!(hash, lot_hash::SPAWNER_IDS | lot_hash::EFFECT_REF_IDS) {"runtimeKey"}
                            else {"unresolved"}}));
                }
            }
        }
    }
    Ok(edges)
}
