use crate::asset_build_m1::{M1BuildResult, events};
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Text, Value};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

pub const FORMAT: &str = "openscp.road-asset/1";
pub const SCHEMA_PATH: &str = "schemas/road/schema.json";
const PROP: u32 = 0x00B1_B104;
const SIM: u32 = 0x40E0_C400;
const GRAPHICS: u32 = 0x40E1_C400;
const MENU: u32 = 0x0987_8A01;
const PATH_LINK: u32 = 0xD13B_B4A6;
const TITLE: u32 = 0x0A09_F5FA;
const DESCRIPTION: u32 = 0x0A09_F5FB;
const PATH_TITLE: u32 = 0x0EC4_AB13;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Recipe {
    Dirt,
    LowStreet,
    MediumStreet,
    HighStreet,
    MediumAvenue,
    HighAvenue,
    Streetcar,
}
impl Recipe {
    pub(crate) fn donor(self) -> (u32, u32) {
        match self {
            Self::Dirt => (0x7DD8_A627, 0x0984_F4ED),
            Self::LowStreet => (0xB7D3_F194, 0x13E5_FC33),
            Self::MediumStreet => (0x795F_7036, 0x22A8_5851),
            Self::HighStreet => (0x4115_8FE5, 0xF890_0236),
            Self::MediumAvenue => (0x6768_04D7, 0xE5CE_83A6),
            Self::HighAvenue => (0x2CD1_30D3, 0x82E6_2CA6),
            Self::Streetcar => (0x1055_F94B, 0x6FB8_287E),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RoadSchema {
    format: String,
    recipe: Recipe,
    identity: u32,
    title: String,
    description: String,
    game_package: String,
    scripts_package: String,
    #[serde(default)]
    streetlights: Streetlights,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Streetlights {
    #[default]
    Original,
    Left,
    Right,
    None,
}

// The ground high-density street uses symmetric instance components at x=±11.5.
// The zero-geometry component is already a row in the native extrusion table.
fn customize_streetlights(
    schema: &RoadSchema,
    game: &Package,
    graphics: &mut PropertyFile,
) -> Result<Option<OverlayEntry>, String> {
    if matches!(schema.streetlights, Streetlights::Original) {
        return Ok(None);
    }
    if !matches!(schema.recipe, Recipe::HighStreet) {
        return Err("road_streetlights_recipe_invalid".into());
    }
    const STATIC: u32 = 0x0953_2375;
    const COMPONENTS: u32 = 0xBCC5_3802;
    let link = graphics
        .values
        .iter_mut()
        .find(|p| p.hash == STATIC)
        .ok_or("road_graphics_donor_missing")?;
    let Kind::Scalar(Value::Key(key)) = &mut link.kind else {
        return Err("road_graphics_donor_missing".into());
    };
    if key.group != 0x0955_8B7E || key.instance != 0xF890_0236 {
        return Err("road_streetlight_donor_invalid".into());
    }
    let mut appearance = property(game, property_id(key.group, key.instance))?;
    let new_appearance = property_id(key.group, schema.identity);
    if game.entry(new_appearance).is_some() {
        return Err("road_identity_collision".into());
    }
    let placements = appearance
        .get(0x0955_882C)
        .and_then(|p| p.array())
        .ok_or("road_streetlight_component_missing")?;
    let components = appearance
        .get(COMPONENTS)
        .and_then(|p| p.array())
        .ok_or("road_component_table_missing")?;
    if placements.len() != components.len()
        || ![(0xF22D_80C4, 11.5), (0x810E_C613, -11.5)]
            .iter()
            .all(|(instance, x)| {
                components
                    .iter()
                    .zip(placements)
                    .any(|(component, position)| {
                        matches!(component, Value::Key(key) if key.instance == *instance)
                            && matches!(position, Value::Vector3(position) if position[0] == *x)
                    })
            })
    {
        return Err("road_streetlight_donor_invalid".into());
    }
    let target = appearance
        .values
        .iter_mut()
        .find(|p| p.hash == COMPONENTS)
        .ok_or("road_component_table_missing")?;
    let Kind::Array(rows) = &mut target.kind else {
        return Err("road_component_table_missing".into());
    };
    let empty = rows
        .iter()
        .find(|row| matches!(row, Value::Key(k) if k.instance == 0xC7EE_8594))
        .ok_or("road_empty_component_missing")?
        .clone();
    for (instance, remove) in [
        (
            0xF22D_80C4,
            matches!(schema.streetlights, Streetlights::Left | Streetlights::None),
        ),
        (
            0x810E_C613,
            matches!(
                schema.streetlights,
                Streetlights::Right | Streetlights::None
            ),
        ),
    ] {
        let row = rows
            .iter_mut()
            .find(|row| matches!(row, Value::Key(k) if k.instance == instance))
            .ok_or("road_streetlight_component_missing")?;
        if remove {
            *row = empty.clone();
        }
    }
    key.instance = schema.identity;
    let bytes = appearance.encode_canonical().map_err(|e| e.to_string())?;
    Ok(Some(OverlayEntry::new(new_appearance, bytes)))
}
fn default_schema(name: &str, game_data: Option<&Path>) -> Json {
    let game = game_data
        .map(|path| path.join("SimCity_Game.package"))
        .filter(|path| path.is_file());
    let scripts = game_data.and_then(Path::parent).and_then(|root| {
        fs::read_dir(root.join("SimCityUserData/EcoGame"))
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name();
                let version = name
                    .to_str()?
                    .strip_prefix("SimCity-Scripts_")?
                    .strip_suffix(".package")?
                    .parse::<u32>()
                    .ok()?;
                Some((version, entry.path()))
            })
            .max_by_key(|(version, _)| *version)
            .map(|(_, path)| path)
    });
    let hash = crate::mod_flow::digest(format!("openscp/road/{name}").as_bytes());
    json!({
        "format": FORMAT, "recipe": "highStreet",
        "identity": 0x8000_0000 | (u32::from_str_radix(&hash[..8], 16).unwrap_or_default() >> 1),
        "title": name, "description": "",
        "gamePackage": game.unwrap_or_default(), "scriptsPackage": scripts.unwrap_or_default(),
    })
}

pub(crate) fn initialize_project(
    root: &Path,
    name: &str,
    game_data: Option<&Path>,
    manifest: &mut Json,
) -> Result<(), String> {
    crate::mod_flow::ensure_project_layout(root, "road")?;
    let schema =
        serde_json::to_vec_pretty(&default_schema(name, game_data)).map_err(|e| e.to_string())?;
    let path = crate::mod_flow::safe_path(root, SCHEMA_PATH)?;
    crate::atomic_fs::write_atomic(&path, &schema, false).map_err(|e| e.to_string())?;
    manifest["engine"]["enabled"] = json!(true);
    manifest["build"] = json!({ "kind": "road", "schema": SCHEMA_PATH });
    Ok(())
}

pub fn is_schema(raw: &str) -> bool {
    serde_json::from_str::<Json>(raw).is_ok_and(|value| value["format"] == FORMAT)
}
pub fn project_schema(root: &Path) -> Result<Option<(String, String)>, String> {
    let manifest = crate::mod_flow::safe_path(root, "package.json")?;
    let bytes = match fs::read(manifest) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    // Existing import/build discovery historically accepts projects without an OpenSCP manifest.
    let Ok(manifest) = serde_json::from_slice::<Json>(&bytes) else {
        return Ok(None);
    };
    if manifest["format"] != "openscp.mod" || manifest["origin"] != "openscp" {
        return Ok(None);
    }
    let kind = manifest["build"]["kind"].as_str();
    if kind.is_none() && manifest["mod_type"] != "road" {
        return Ok(None);
    }
    if kind != Some("road") {
        return Err("asset_build_kind_unsupported".into());
    }
    let schema = manifest["build"]["schema"]
        .as_str()
        .ok_or("road_schema_missing")?;
    let raw = fs::read_to_string(crate::mod_flow::safe_path(root, schema)?)
        .map_err(|e| format!("{schema}: {e}"))?;
    parse(&raw)?;
    Ok(Some((schema.into(), raw)))
}

fn parse(raw: &str) -> Result<RoadSchema, String> {
    let schema: RoadSchema = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if schema.format != FORMAT || schema.identity == 0 || schema.title.trim().is_empty() {
        return Err("road_schema_invalid".into());
    }
    Ok(schema)
}

fn property_id(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: PROP,
        group,
        instance,
    }
}

fn payload(package: &Package, id: ResourceId) -> Result<Vec<u8>, String> {
    let entry = package
        .entry(id)
        .ok_or_else(|| format!("road_donor_missing {id}"))?;
    package.read(entry).map_err(|e| e.to_string())
}

fn property(package: &Package, id: ResourceId) -> Result<PropertyFile, String> {
    PropertyFile::parse(&payload(package, id)?).map_err(|e| e.to_string())
}

fn replace_text(file: &mut PropertyFile, hash: u32, table: u32, row: u32) -> Result<(), String> {
    let p = file
        .values
        .iter_mut()
        .find(|p| p.hash == hash)
        .ok_or("road_donor_text_missing")?;
    if !matches!(&p.kind, Kind::Array(values) if matches!(values.as_slice(), [Value::Text(_)])) {
        return Err("road_donor_text_invalid".into());
    }
    p.kind = Kind::Array(vec![Value::Text(Text {
        table_id: table,
        instance_id: row,
    })]);
    Ok(())
}

pub(crate) fn open_source(root: &Path, path: &str) -> Result<Package, String> {
    if path.trim().is_empty() {
        return Err("road_sources_required".into());
    }
    let resolved = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        crate::mod_flow::safe_path(root, path)?
    };
    Package::open(resolved).map_err(|e| e.to_string())
}

fn lower(
    schema: &RoadSchema,
    game: &Package,
    scripts: &Package,
) -> Result<Vec<OverlayEntry>, String> {
    let (menu_id, donor) = schema.recipe.donor();
    let id = schema.identity;
    let output_menu = property_id(MENU, id);
    let output_sim = property_id(SIM, id);
    let output_graphics = property_id(GRAPHICS, id);
    let output_locale = ResourceId {
        type_id: 0x0A98_EAF0,
        group: 0x02FA_BF01,
        instance: id,
    };
    let targets = [output_menu, output_sim, output_graphics, output_locale];
    if [game, scripts].iter().any(|pkg| {
        pkg.entries().iter().any(|e| {
            targets.contains(&e.id)
                || (e.id.type_id == PROP && e.id.instance == id && e.id.group & 0xFFFF == 0xC400)
        })
    }) {
        return Err("road_identity_collision".into());
    }

    let mut menu = property(game, property_id(MENU, menu_id))?;
    let categories = menu
        .get(0x0DB9_FC63)
        .and_then(|p| p.array())
        .ok_or("road_menu_category_missing")?;
    if !categories
        .iter()
        .any(|v| matches!(v, Value::Key(k) if k.instance == 0xAC12_C904))
    {
        return Err("road_menu_category_missing".into());
    }
    let link = menu
        .values
        .iter_mut()
        .find(|p| p.hash == PATH_LINK)
        .ok_or("road_menu_path_missing")?;
    match &mut link.kind {
        Kind::Scalar(Value::Key(key)) if key.instance == donor => key.instance = id,
        _ => return Err("road_menu_path_invalid".into()),
    }
    replace_text(&mut menu, TITLE, id, 1)?;
    replace_text(&mut menu, DESCRIPTION, id, 2)?;

    let simulation = payload(scripts, property_id(SIM, donor))?;
    let appearance = payload(game, property_id(GRAPHICS, donor))?;
    let sim = PropertyFile::parse(&simulation).map_err(|e| e.to_string())?;
    let mut graphics = PropertyFile::parse(&appearance).map_err(|e| e.to_string())?;
    replace_text(&mut graphics, PATH_TITLE, id, 1)?;
    let parent = match sim.get(sc_properties::PARENT_HASH).and_then(|p| p.scalar()) {
        Some(Value::Key(key)) if key.instance != 0 => key,
        _ => return Err("road_simulation_parent_missing".into()),
    };
    let layout = property(scripts, property_id(parent.group, parent.instance))?;
    let rows = layout
        .get(0x0BD8_1A2D)
        .and_then(|p| p.array())
        .ok_or("road_lane_table_missing")?;
    if rows.is_empty()
        || !rows.iter().all(|value| matches!(value, Value::Key(_)))
        || !rows
            .iter()
            .any(|value| matches!(value, Value::Key(key) if key.instance == 0x54AB_DF8C))
        || (0x0BD8_1A2E..=0x0BD8_1A2F)
            .chain([0x0BD8_1A31])
            .any(|hash| {
                layout.get(hash).and_then(|p| p.array()).map(|v| v.len()) != Some(rows.len())
            })
    {
        return Err("road_lane_table_invalid".into());
    }
    for column in [0x0953_2375, 0x0953_2377, 0x7515_2C4A] {
        match graphics.get(column).and_then(|p| p.scalar()) {
            Some(Value::Key(key)) if key.instance != 0 => {
                property(game, property_id(key.group, key.instance))?;
            }
            _ => return Err("road_graphics_donor_missing".into()),
        }
    }

    let custom = customize_streetlights(schema, game, &mut graphics)?;
    let appearance = graphics.encode_canonical().map_err(|e| e.to_string())?;
    let title = serde_json::to_string(schema.title.trim()).map_err(|e| e.to_string())?;
    let description =
        serde_json::to_string(schema.description.trim()).map_err(|e| e.to_string())?;
    let locale = format!(
        "\u{feff}{{\r\n    \"0x00000001\" : {title},\r\n    \"0x00000002\" : {description}\r\n}}"
    );
    let menu = menu.encode_canonical().map_err(|e| e.to_string())?;
    let mut entries = vec![
        OverlayEntry::new(output_menu, menu),
        OverlayEntry::new(output_sim, simulation),
        OverlayEntry::new(output_graphics, appearance),
        OverlayEntry::new(output_locale, locale.into_bytes()),
    ];
    entries.extend(custom);
    Ok(entries)
}

pub fn build(
    root: &Path,
    raw: &str,
    progress: Option<Sender<events::StageEvent>>,
) -> Result<M1BuildResult, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let started = events::now_ms();
    let stage = "road::compile";
    let mut steps = Vec::new();
    let mut emit = |event: events::StageEvent| {
        if let Some(tx) = &progress {
            let _ = tx.send(event.clone());
        }
        steps.push(event);
    };
    emit(events::started(stage, Json::Null));
    let outcome = (|| -> Result<_, String> {
        let schema = parse(raw)?;
        let game = open_source(&root, &schema.game_package)?;
        let scripts = open_source(&root, &schema.scripts_package)?;
        let entries = lower(&schema, &game, &scripts)?;
        let bytes = dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())?;
        let sha256 = crate::mod_flow::digest(&bytes);
        let folder = format!(".build/{sha256}");
        let path = crate::mod_flow::safe_path(&root, &format!("{folder}/road.package"))?;
        fs::create_dir_all(crate::mod_flow::safe_path(&root, &folder)?)
            .map_err(|e| e.to_string())?;
        crate::atomic_fs::write_atomic(&path, &bytes, true).map_err(|e| e.to_string())?;
        let written = Package::open(&path).map_err(|e| e.to_string())?;
        for entry in &entries {
            if payload(&written, entry.id)? != entry.data {
                return Err("road_readback_mismatch".into());
            }
        }
        Ok((schema, path, sha256, entries.len()))
    })();
    let finished = events::now_ms();
    let duration = finished.saturating_sub(started);
    match outcome {
        Ok((schema, path, sha256, count)) => {
            emit(events::finished(
                stage,
                json!({ "resourceCount": count }),
                duration,
            ));
            Ok(M1BuildResult {
                path,
                schema_hash: crate::mod_flow::digest(raw.as_bytes()),
                build_hash: sha256.clone(),
                source_hash: String::new(),
                sha256,
                resource_count: count,
                delta_mode: true,
                menu_instance: Some(schema.identity),
                patched_resources: 0,
                scope: "road",
                scene: Default::default(),
                events: steps,
                started_at_ms: started,
                finished_at_ms: finished,
                duration_ms: duration,
            })
        }
        Err(error) => {
            emit(events::failed(stage, &error, duration));
            Err(error)
        }
    }
}
