//! Project workflow for editing city EcoMaps in local-template baselines.
use dbpf::{OverlayEntry, Package, ResourceId};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sc_properties::{
    region_3d, region_bootstrap,
    region_state::{
        StateMap,
        edit::{CellEdit, feathered_stamp, patch_maps, round_stamp},
        read_state,
        wind::{WIND_DIRECTION_ID, read_wind_direction, set_wind_direction},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use crate::asset_build_m1::{M1BuildResult, events};
use crate::mod_flow::{digest, safe_path};

pub const SCHEMA_PATH: &str = "schemas/map_resources/schema.json";
pub const STAGES: usize = 3;
const FORMAT: &str = "openscp.map-resources";
const LAYERS: [u32; 6] = [
    0xD779C976, 0xD74AF22B, 0x76D10FF2, 0x0F92DC56, 0x5B9DB0F3, 0x1D716F2E,
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    package_path: PathBuf,
    group: String,
    save_directory: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stroke {
    site: String,
    layer: u32,
    center: [f64; 2],
    radius: f64,
    value: i32,
    #[serde(default)]
    profile: BrushProfile,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum BrushProfile {
    #[default]
    Flat,
    Feathered,
    Direction,
}

#[derive(Deserialize)]
struct Schema {
    #[serde(rename = "$schema")]
    format: String,
    version: u32,
    source: Source,
    fingerprint: String,
    strokes: Vec<Stroke>,
}

struct City {
    site: String,
    id: ResourceId,
    environment: u32,
    bounds: [f64; 4],
    bytes: Vec<u8>,
    maps: Vec<StateMap>,
    packed_hash: String,
    save_file: Option<PathBuf>,
}

mod save_copy;
mod selection_ratings;
const LAND_POLLUTION: u32 = 0xF69DF1C6;

fn editable_layer(id: u32, is_save: bool) -> bool {
    LAYERS.contains(&id) || (is_save && id == LAND_POLLUTION)
}

fn unpack(input: &[u8]) -> Result<Vec<u8>, String> {
    const LIMIT: u64 = 128 * 1024 * 1024;
    let mut out = Vec::new();
    GzDecoder::new(input)
        .take(LIMIT + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() as u64 > LIMIT {
        return Err("EcoGame snapshot too large".into());
    }
    Ok(out)
}

fn load(source: &Source) -> Result<(Vec<City>, String, Option<save_copy::Snapshot>), String> {
    let group = u32::from_str_radix(&source.group, 16).map_err(|e| e.to_string())?;
    let path = source
        .package_path
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let terrain = Package::open(&path).map_err(|e| e.to_string())?;
    let companion = path.with_file_name("SimCityDataEP1.package");
    let ep1 = Package::open(&companion).map_err(|e| format!("{}: {e}", companion.display()))?;
    let binding = region_bootstrap::catalog(&ep1)?
        .into_iter()
        .find(|binding| binding.region == group)
        .ok_or("Region has no local template")?;
    let save = source
        .save_directory
        .as_deref()
        .map(save_copy::Snapshot::load)
        .transpose()?;
    if let Some(snapshot) = &save {
        let (_, packed) = snapshot.latest("0").ok_or("Missing region save")?;
        if read_state(&unpack(packed)?)?.environment != group {
            return Err("Save region differs from the selected terrain region".into());
        }
    }
    let (uids, positions, environments) =
        region_3d::plot_table(&terrain, group).ok_or("Missing city slot table")?;
    if uids.len() + 1 != binding.sites.len()
        || uids.len() != positions.len()
        || uids.len() != environments.len()
    {
        return Err("City slot table does not match the local template".into());
    }
    let mut cities = Vec::new();
    for (site, key) in binding.sites.into_iter().filter(|(site, _)| site != "0") {
        let i = uids
            .iter()
            .position(|uid| uid == &site)
            .ok_or("Unrecognized city UID")?;
        let id = ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        };
        let (save_file, packed) = if let Some(snapshot) = &save {
            let Some((file, bytes)) = snapshot.latest(&site) else {
                continue;
            };
            (Some(file.clone()), bytes.clone())
        } else {
            (
                None,
                ep1.read(ep1.entry(id).ok_or("Missing city bootstrap")?)
                    .map_err(|e| e.to_string())?,
            )
        };
        let bytes = unpack(&packed)?;
        let environment = environments[i];
        let state = read_state(&bytes)?;
        if state.environment != environment || state.maps.iter().any(|map| map.size != 128) {
            return Err(format!(
                "Unsupported city state or grid geometry for {site}"
            ));
        }
        let (x, y) = positions[i];
        let (x, y) = (f64::from(x), f64::from(y));
        cities.push(City {
            site,
            id,
            environment,
            bytes,
            maps: state.maps,
            packed_hash: digest(&packed),
            save_file,
            bounds: [x - 1024., y - 1024., x + 1024., y + 1024.],
        });
    }
    let ids: Vec<_> = cities
        .iter()
        .map(|c| {
            (
                &c.site,
                c.environment,
                c.id.to_string(),
                c.bounds,
                &c.packed_hash,
            )
        })
        .collect();
    let fingerprint = digest(
        &serde_json::to_vec(&(group, ids, save.as_ref().map(|s| &s.fingerprint)))
            .map_err(|e| e.to_string())?,
    );
    Ok((cities, fingerprint, save))
}

pub fn preview(payload: &Value) -> Result<Value, String> {
    let source = serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
    let (cities, fingerprint, save) = load(&source)?;
    let sources = cities.into_iter().map(|city| {
        json!({
            "site": city.site, "environment": format!("{:08X}", city.environment),
            "bounds": city.bounds,
            "windDirection": read_wind_direction(&city.bytes).ok().flatten(),
            "maps": city.maps.into_iter().filter(|m| editable_layer(m.id, save.is_some())).collect::<Vec<_>>()
        })
    }).collect::<Vec<_>>();
    Ok(json!({ "fingerprint": fingerprint, "cities": sources }))
}

fn parse(raw: &str) -> Result<Schema, String> {
    let schema: Schema = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if schema.format != FORMAT || schema.version != 1 || schema.strokes.len() > 4096 {
        return Err("Unsupported map resources schema or too many strokes".into());
    }
    Ok(schema)
}

pub fn is_schema(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw).is_ok_and(|value| value["$schema"] == FORMAT)
}

fn edits(
    schema: &Schema,
    cities: &[City],
) -> Result<BTreeMap<String, BTreeMap<u32, Vec<CellEdit>>>, String> {
    // Resolve the ordered strokes once into absolute values for patch_maps.
    let mut changes: BTreeMap<String, BTreeMap<u32, BTreeMap<(usize, usize), i32>>> =
        BTreeMap::new();
    let mut budget = 0;
    for stroke in &schema.strokes {
        if stroke.profile == BrushProfile::Direction {
            continue;
        }
        let city = cities
            .iter()
            .find(|city| city.site == stroke.site)
            .ok_or("Unknown city UID")?;
        if !editable_layer(stroke.layer, schema.source.save_directory.is_some())
            || (stroke.layer == LAND_POLLUTION && stroke.value != 0)
            || !(0..=65534).contains(&stroke.value)
            || !(8.0..=512.0).contains(&stroke.radius)
            || (stroke.profile == BrushProfile::Feathered && stroke.value == 0)
        {
            return Err("Invalid resource, brush radius or native value".into());
        }
        let map = city
            .maps
            .iter()
            .find(|map| map.id == stroke.layer)
            .ok_or("Edit references absent state map")?;
        let feathered = stroke.profile == BrushProfile::Feathered;
        let rasterize = if feathered {
            feathered_stamp
        } else {
            round_stamp
        };
        let cells = rasterize(
            map.size,
            city.bounds,
            stroke.center,
            stroke.radius,
            stroke.value,
        )?;
        budget += cells.len();
        if budget > 4_194_304 {
            return Err("Resource brush cell limit exceeded".into());
        }
        let layer = changes
            .entry(stroke.site.clone())
            .or_default()
            .entry(stroke.layer)
            .or_default();
        for cell in cells {
            let next = layer
                .entry((cell.x, cell.y))
                .or_insert(map.values[cell.y * map.size + cell.x]);
            *next = if feathered {
                (*next).max(cell.value)
            } else {
                cell.value
            };
        }
    }
    Ok(changes
        .into_iter()
        .map(|(site, layers)| {
            (
                site,
                layers
                    .into_iter()
                    .map(|(id, cells)| {
                        let edits = cells
                            .into_iter()
                            .map(|((x, y), value)| CellEdit { x, y, value })
                            .collect();
                        (id, edits)
                    })
                    .collect(),
            )
        })
        .collect())
}

fn wind_edits(schema: &Schema, cities: &[City]) -> Result<BTreeMap<String, i32>, String> {
    let mut changes = BTreeMap::new();
    for stroke in schema
        .strokes
        .iter()
        .filter(|stroke| stroke.profile == BrushProfile::Direction)
    {
        let city = cities
            .iter()
            .find(|city| city.site == stroke.site)
            .ok_or("Unknown city UID")?;
        let [x0, y0, x1, y1] = city.bounds;
        let [x, y] = stroke.center;
        if stroke.layer != WIND_DIRECTION_ID
            || stroke.radius != 0.0
            || !(0..360).contains(&stroke.value)
            || !(x0..x1).contains(&x)
            || !(y0..y1).contains(&y)
        {
            return Err("Invalid city wind arrow".into());
        }
        changes.insert(city.site.clone(), stroke.value);
    }
    Ok(changes)
}

pub fn save(root: &Path, payload: &Value) -> Result<Value, String> {
    let path = safe_path(root, SCHEMA_PATH)?;
    let observed = match fs::read(&path) {
        Ok(bytes) => Some(digest(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if observed.as_deref() != payload["revision"].as_str() {
        return Err("The map resource schema changed on disk".into());
    }
    let raw = serde_json::to_string_pretty(&payload["schema"]).map_err(|e| e.to_string())?;
    let schema = parse(&raw)?;
    let (cities, fingerprint, _) = load(&schema.source)?;
    if schema.fingerprint != fingerprint {
        return Err("Map source changed on disk".into());
    }
    for (site, changes) in edits(&schema, &cities)? {
        let city = cities.iter().find(|city| city.site == site).unwrap();
        patch_maps(&city.bytes, city.environment, &changes)?;
    }
    for (site, angle) in wind_edits(&schema, &cities)? {
        let mut bytes = cities
            .iter()
            .find(|city| city.site == site)
            .unwrap()
            .bytes
            .clone();
        set_wind_direction(&mut bytes, angle)?;
    }
    crate::atomic_fs::write_atomic(&path, raw.as_bytes(), observed.is_some())
        .map_err(|e| e.to_string())?;
    Ok(json!({ "revision": digest(raw.as_bytes()) }))
}

pub fn project_schema(root: &Path) -> Result<Option<String>, String> {
    let Ok(bytes) = fs::read(root.join("package.json")) else {
        return Ok(None);
    };
    let Ok(manifest) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    if manifest["mod_type"] != "map_resources" {
        return Ok(None);
    }
    let raw = fs::read_to_string(safe_path(root, SCHEMA_PATH)?).map_err(|e| e.to_string())?;
    if parse(&raw)?.strokes.is_empty() {
        return Err("Save a resource edit before building".into());
    }
    Ok(Some(raw))
}

pub fn build(
    root: &Path,
    raw: &str,
    tx: Sender<events::StageEvent>,
) -> Result<M1BuildResult, String> {
    let mut log = Vec::new();
    let mut emit = |event: events::StageEvent| {
        let _ = tx.send(event.clone());
        log.push(event);
    };
    let started = events::now_ms();
    let mut stage = "map_resources::preflight";
    emit(events::started(stage, Value::Null));
    let outcome = (|| {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let schema = parse(raw)?;
        let (cities, fingerprint, mut save) = load(&schema.source)?;
        if schema.fingerprint != fingerprint {
            return Err("Map source changed on disk".into());
        }
        let changes = edits(&schema, &cities)?;
        let wind = wind_edits(&schema, &cities)?;
        emit(events::finished(
            stage,
            json!({"cities": changes.keys().chain(wind.keys()).collect::<std::collections::BTreeSet<_>>().len()}),
            events::now_ms() - started,
        ));
        stage = "map_resources::patch";
        let patch_started = events::now_ms();
        emit(events::started(stage, Value::Null));
        let mut entries = Vec::new();
        let mut patched_saves = 0;
        let mut rating_totals = selection_ratings::Totals::new();
        for city in &cities {
            let cell_edits = changes.get(&city.site);
            let direction = wind.get(&city.site);
            if cell_edits.is_none() && direction.is_none() {
                continue;
            }
            let mut amended = match cell_edits {
                Some(edits) => patch_maps(&city.bytes, city.environment, edits)?,
                None => city.bytes.clone(),
            };
            if let Some(&angle) = direction {
                set_wind_direction(&mut amended, angle)?;
                if read_wind_direction(&amended)? != Some(angle) {
                    return Err("WindDirection verification failed".into());
                }
            }
            if amended == city.bytes {
                continue;
            }
            if save.is_none() && cell_edits.is_some() {
                selection_ratings::collect(&mut rating_totals, city, &read_state(&amended)?.maps);
            }
            let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
            gzip.write_all(&amended).map_err(|e| e.to_string())?;
            let packed = gzip.finish().map_err(|e| e.to_string())?;
            if let (Some(snapshot), Some(file)) = (&mut save, &city.save_file) {
                snapshot.files.insert(file.clone(), packed);
                patched_saves += 1;
            } else {
                entries.push(OverlayEntry::new(city.id, packed));
            }
        }
        let mut package_prefix = "000_Map";
        if save.is_none() {
            let group =
                u32::from_str_radix(&schema.source.group, 16).map_err(|err| err.to_string())?;
            let (overlays, earlier_name) = selection_ratings::overlays(
                &schema.source.package_path,
                group,
                &cities,
                &rating_totals,
            )?;
            if earlier_name {
                // The retail resource manager uses the first registered
                // equal-priority package. This sorts before LauncherTweak.
                package_prefix = "000_0_Map";
            }
            entries.extend(overlays);
        }
        let count = entries.len() + patched_saves;
        if count == 0 {
            return Err("No resource cells changed".into());
        }
        emit(events::finished(
            stage,
            json!({"resourceCount": count}),
            events::now_ms() - patch_started,
        ));
        stage = "map_resources::verify";
        let output_started = events::now_ms();
        emit(events::started(stage, Value::Null));
        if let Some(snapshot) = save {
            let (path, hash) = snapshot.export(&root)?;
            emit(events::finished(
                stage,
                json!({"resourceCount": count}),
                events::now_ms() - output_started,
            ));
            return Ok((path, hash, fingerprint, count, "map_resources_save"));
        }
        let bytes = dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())?;
        let hash = digest(&bytes);
        let folder = safe_path(&root, &format!(".build/{hash}"))?;
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let path = safe_path(
            &root,
            &format!(
                ".build/{hash}/{package_prefix}_{}_Resources.package",
                schema.source.group
            ),
        )?;
        crate::atomic_fs::write_atomic(&path, &bytes, true).map_err(|e| e.to_string())?;
        let written = Package::open(&path).map_err(|e| e.to_string())?;
        for entry in &entries {
            let data = written
                .read(written.entry(entry.id).ok_or("Missing output bootstrap")?)
                .map_err(|e| e.to_string())?;
            if data != entry.data {
                return Err("Resource package verification failed".into());
            }
        }
        emit(events::finished(
            stage,
            json!({"resourceCount": entries.len()}),
            events::now_ms() - output_started,
        ));
        Ok((path, hash, fingerprint, count, "map_resources"))
    })();
    let finished = events::now_ms();
    let (path, hash, source_hash, count, scope) = outcome.inspect_err(|e: &String| {
        emit(events::failed(stage, e, finished.saturating_sub(started)));
    })?;
    Ok(M1BuildResult {
        requires_panel_runtime: false,
        path,
        source_hash,
        schema_hash: digest(raw.as_bytes()),
        build_hash: hash.clone(),
        sha256: hash,
        resource_count: count,
        delta_mode: true,
        menu_instance: None,
        patched_resources: count,
        scope,
        scene: Default::default(),
        events: log,
        started_at_ms: started,
        finished_at_ms: finished,
        duration_ms: finished.saturating_sub(started),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires installed RegionTerrain0 and an optional offline region archive"]
    fn real_region_builds_new_city_package_and_save_copy() {
        let output = std::env::var_os("OPENSCP_TEST_OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let root = output.join(format!("openscp-resource-smoke-{}", std::process::id()));
        fs::create_dir_all(root.join("schemas/map_resources")).unwrap();
        let package_path = std::env::var("OPENSCP_TEST_REGION_TERRAIN").unwrap();
        println!("Smoke build root: {}", root.display());
        let mut sources = vec![json!({"packagePath": package_path, "group": "A0B60DDE"})];
        if let Ok(directory) = std::env::var("OPENSCP_TEST_REGION_SAVE") {
            let mut source = sources[0].clone();
            source["saveDirectory"] = json!(directory);
            sources.push(source);
        }
        let terrain_path = sources[0]["packagePath"].clone();
        sources.push(json!({ "packagePath": terrain_path, "group": "E41A82B8" }));
        for source in sources {
            let details = preview(&source).unwrap();
            let site = if source["group"] == "E41A82B8" {
                "1029"
            } else {
                "1031"
            };
            let city = details["cities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|city| city["site"] == site)
                .unwrap();
            let bounds = city["bounds"].as_array().unwrap();
            let x = bounds[0].as_f64().unwrap() + 8.0;
            let y = bounds[1].as_f64().unwrap() + 8.0;
            let schema = json!({
                "$schema": FORMAT, "version": 1, "source": source, "fingerprint": details["fingerprint"],
                "strokes": [
                    {"site": site, "layer": LAYERS[0], "center": [x, y], "radius": 64, "value": 0},
                    {"site": site, "layer": LAYERS[0], "center": [x, y], "radius": 64, "value": 32768, "profile": "feathered"},
                    {"site": site, "layer": LAYERS[0], "center": [x, y], "radius": 64, "value": 2000, "profile": "feathered"},
                    {"site": site, "layer": LAYERS[0], "center": [x + 32., y], "radius": 8, "value": 0},
                    {"site": site, "layer": LAYERS[1], "center": [x, y], "radius": 8, "value": 1234},
                    {"site": site, "layer": WIND_DIRECTION_ID, "center": [x, y], "radius": 0, "value": 217, "profile": "direction"}
                ],
            });
            fs::write(root.join("package.json"), r#"{"mod_type":"map_resources"}"#).unwrap();
            let revision = fs::read(root.join(SCHEMA_PATH))
                .ok()
                .map(|bytes| digest(&bytes));
            save(&root, &json!({"schema": schema, "revision": revision})).unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            let raw = project_schema(&root).unwrap().unwrap();
            let result = build(&root, &raw, tx).unwrap();
            assert_eq!(rx.iter().count(), 6);
            let packed = if schema["source"]["saveDirectory"].is_null() {
                let written = Package::open(&result.path).unwrap();
                let parsed = parse(&raw).unwrap();
                let (baseline, _, _) = load(&parsed.source).unwrap();
                let id = baseline.iter().find(|city| city.site == site).unwrap().id;
                let entry = written.entry(id).unwrap();
                assert_eq!(result.resource_count, 3);
                let city_name = if source["group"] == "E41A82B8" {
                    0x3A4B861A
                } else {
                    0x195F151C
                };
                for script in [selection_ratings::UI_ID, selection_ratings::LAUNCHER_ID] {
                    let bytes = written.read(written.entry(script).unwrap()).unwrap();
                    let text = std::str::from_utf8(&bytes).unwrap();
                    assert_eq!(
                        selection_ratings::field(text, city_name, "coal").unwrap().1,
                        1
                    );
                }
                assert_eq!(digest(&fs::read(&result.path).unwrap()), result.sha256);
                written.read(entry).unwrap()
            } else {
                assert_eq!(result.resource_count, 1);
                let exported = save_copy::Snapshot::load(&result.path).unwrap();
                assert_eq!(exported.fingerprint, result.sha256);
                exported.latest(site).unwrap().1.clone()
            };
            let bytes = unpack(&packed).unwrap();
            assert_eq!(read_wind_direction(&bytes).unwrap(), Some(217));
            let city = read_state(&bytes).unwrap();
            assert_eq!(
                format!("{:08X}", city.environment),
                details["cities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|city| city["site"] == site)
                    .unwrap()["environment"]
                    .as_str()
                    .unwrap()
            );
            let coal = &city.maps.iter().find(|m| m.id == LAYERS[0]).unwrap().values;
            assert_eq!(coal[..5], [32768, 28800, 0, 6272, 0]);
            let ore = &city.maps.iter().find(|m| m.id == LAYERS[1]).unwrap().values;
            assert_eq!(ore[0], 1234);
            println!(
                "{} {} {} coal={:?} ore={}",
                schema["source"]["group"],
                result.sha256,
                result.path.display(),
                &coal[..5],
                ore[0]
            );
        }
    }
}
