//! Census the retail remnants of Terrain Authoring.
//!
//! This intentionally separates three things that are often conflated:
//! - menu text that names a terrain painter;
//! - the `environment` property bit that enables region authoring/camera mode;
//! - the native terrain-authoring state used by the exporter (`SetTerrainAuthoringToolActive`).
//!
//! Usage:
//! `cargo run -p probes --release --example terrain_authoring_census -- <package>...`

use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY: u32 = 0x00B1_B104;
const MENU_GROUP: u32 = 0x0987_8A01;
const MENU2_MASK: u32 = 0xFFFF;
const TITLE: u32 = 0x0A09_F5FA;
const DESC: u32 = 0x0A09_F5FB;
const ACTION: u32 = 0x08F6_72C7;
const ENV_ENABLE: u32 = 0x0E27_6F5D;
const ENV_REGION: u32 = 0xC194_9C4D;

// Locale instances retained by the retail Terrain Authoring palette. Matching
// the instance as well as the table is essential: the common development
// locale table is also used by hundreds of ordinary building entries.
const TERRAIN_TITLE_INSTANCES: &[u32] = &[
    0xDFAC_D941, // Water Table Painter
    0xDFAC_D942, // Water River Painter
    0x0D42_6EC7, // CoalDeposits Painter
    0xE641_9335, // Forest Painter
    0x7B3F_17D5, // Forest Painter (variant)
    0x0D8D_2F27, // Oil Reservoir Painter
    0x0D94_0A49, // Ore Deposit Painter
    0x9FC6_5D5B, // Soil Painter
    0xEE4B_C898, // SoilLayer Painter
    0x325A_E009, // Traffic Painter
    0xD310_ACE6, // Air Pollution Painter
    0x0E37_D783, // Region Air Pollution Painter
    0xAA3A_11A3, // Ground Pollution Painter
    0x0D3B_70F5, // Garbage Painter
];

const TERRAIN_DESC_INSTANCES: &[u32] = &[
    0xC841_B94D, // Raises/lowers the soil height.
    0x0D42_6EC8, // Raises/lowers the CoalDeposits height.
];

fn text(v: &Value) -> Option<(u32, u32)> {
    match v {
        Value::Text(t) => Some((t.table_id, t.instance_id)),
        _ => None,
    }
}

fn text_prop(file: &PropertyFile, hash: u32) -> Option<(u32, u32)> {
    match &file.get(hash)?.kind {
        Kind::Scalar(v) => text(v),
        Kind::Array(vs) => vs.iter().find_map(text),
        Kind::Empty => None,
    }
}

fn bool_prop(file: &PropertyFile, hash: u32) -> Option<bool> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Bool(v)) => Some(*v),
        _ => None,
    }
}

fn key_prop(file: &PropertyFile, hash: u32) -> Option<(u32, u32, u32)> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Key(k)) => Some((k.type_id, k.group, k.instance)),
        _ => None,
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: terrain_authoring_census <package>...");
        std::process::exit(2);
    }
    let mut menu_hits = 0usize;
    let mut env_hits = 0usize;
    for path in paths {
        let Ok(package) = dbpf::Package::open(&path) else {
            eprintln!("skip {path}");
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };

            if entry.id.group == MENU_GROUP
                || entry.id.group & MENU2_MASK == 0xC900
            {
                let title = text_prop(&file, TITLE);
                let desc = text_prop(&file, DESC);
                let interesting = title.is_some_and(|(table, instance)| {
                    table == 0x6C96_9DEE && TERRAIN_TITLE_INSTANCES.contains(&instance)
                }) || desc.is_some_and(|(table, instance)| {
                    table == 0x50AA_0BEA && TERRAIN_DESC_INSTANCES.contains(&instance)
                });
                if interesting {
                    menu_hits += 1;
                    println!(
                        "MENU {}: {:08X}:{:08X}:{:08X} title={:?} desc={:?} action={:?}",
                        path,
                        entry.id.type_id,
                        entry.id.group,
                        entry.id.instance,
                        title,
                        desc,
                        file.get(ACTION).map(|p| format!("{:?}", p.kind)),
                    );
                }
            }

            if let Some(enabled) = bool_prop(&file, ENV_ENABLE) {
                if let Some(region) = key_prop(&file, ENV_REGION) {
                    env_hits += 1;
                    println!(
                        "ENV {}: {:08X}:{:08X}:{:08X} enable={} region={:08X}:{:08X}:{:08X}",
                        path,
                        entry.id.type_id,
                        entry.id.group,
                        entry.id.instance,
                        enabled,
                        region.0,
                        region.1,
                        region.2,
                    );
                }
            }
        }
    }
    eprintln!("summary: terrain-painter menus={menu_hits}, environment links={env_hits}");
}
