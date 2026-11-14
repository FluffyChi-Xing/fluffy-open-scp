//! RCI/plop 路径普查探针（2026-11-06）：把每个建筑 lot 的 path 列表解析到
//! PathEntry → 类型模板 → filter，按"有菜单(plop)/无菜单(RCI)"分类统计。
//!
//! 用法：cargo run -p probes --release --example path_census -- \
//!   scripts=<pkg> game=<pkg> graphics=<pkg> [out=<dir>]
//!
//! 输出 `<out>/path-census.json` + stdout 摘要。
//! 分类规则（源码证据见 docs/re/asset-behavior-census-and-gating-20261106.md §4.7）：
//! - extrusion：条目带 0x09532375/0x75152C4A（路径挤出/交叉口渲染模板，
//!   cGraphicsPath::Init / IsReallyIntersection），不是可行走网络；
//! - pedestrian：0x0D6498E8 filter 集合含行人 filter（979CC22E）；
//! - vehicle：含通用车辆 filter（C37D661D）；
//! - no_filter：无 filter 声明（对车辆缺省全通、对行人不可见）；
//! - unknown_filter：其它 filter。
//! 注意：lot 的 entry key 组是名义值（引擎按 instance 解析），本探针按
//! instance 在 40E0C400/40E1C400 两处解析。

use dbpf::Package;
use sc_properties::{Key, Kind, PARENT_HASH, ParseLimits, PropertyFile, Value};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const GROUP_UNIT: u32 = 0x40E0_C000;
const GROUP_LOT: u32 = 0x40E1_C000;
const GROUP_MENU: u32 = 0x0987_8A01;
const GROUP_PATH_ENTRY_SIM: u32 = 0x40E0_C400;
const GROUP_PATH_ENTRY_NS: u32 = 0x40E1_C400;
const GROUP_PATH_TMPL_SIM: u32 = 0x48E0_C400;
const GROUP_PATH_TMPL_NS: u32 = 0x48E1_C400;
const GROUP_TRANSPORT_NS: u32 = 0x40E1_C600;

const P_PARENT: u32 = PARENT_HASH; // 0x00B2CCCB
const P_LOT_LINK: u32 = 0x08E1_7ED7;
const P_MENU_UNIT: u32 = 0x0975_695F;
const P_PATHS: u32 = 0x0CAA_6841;
const P_POINTS: u32 = 0x0CAA_680D;
const P_ENTRIES: u32 = 0x0CC8_FE7A;
const P_SCOPE: u32 = 0x0D9A_4E54;
const P_CONNECT: u32 = 0x0D9A_A2BB;
const P_FILTERS: u32 = 0x0D64_98E8;
const P_WIDTH: u32 = 0x09E6_794C;
const P_GEOMETRY: u32 = 0x0BD8_1A2D;
const P_EXTRUDE_STATIC: u32 = 0x0953_2375;
const P_EXTRUDE_INVALID: u32 = 0x7515_2C4A;
const P_CARRIER_FILTER: u32 = 0x0EB2_2775;

const PED_FILTER: u32 = 0x979C_C22E;
const VEHICLE_FILTER: u32 = 0xC37D_661D;

fn keys_of(file: &PropertyFile, hash: u32) -> Vec<Key> {
    let Some(prop) = file.get(hash) else {
        return vec![];
    };
    match &prop.kind {
        Kind::Scalar(Value::Key(k)) => vec![k.clone()],
        Kind::Array(values) => values
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.clone()),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

fn u32_of(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::UInt32(v)) => Some(*v),
        Kind::Scalar(Value::Int32(v)) => Some(*v as u32),
        _ => None,
    }
}

fn count_of(file: &PropertyFile, hash: u32) -> usize {
    match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(values)) => values.len(),
        Some(Kind::Scalar(_)) => 1,
        _ => 0,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pkgs: Vec<(String, PathBuf)> = Vec::new();
    let mut out_dir = PathBuf::from("tmp/path-census-20261106");
    for arg in args {
        if let Some(rest) = arg.strip_prefix("out=") {
            out_dir = rest.into();
        } else {
            let (label, path) = arg.split_once('=').expect("label=path");
            pkgs.push((label.to_string(), PathBuf::from(path)));
        }
    }
    assert!(!pkgs.is_empty());

    // 只驻留 path 相关组 + lot/unit/menu/transport-ns。
    let allowed = |group: u32| {
        matches!(
            group,
            GROUP_UNIT
                | GROUP_LOT
                | GROUP_MENU
                | GROUP_PATH_ENTRY_SIM
                | GROUP_PATH_ENTRY_NS
                | GROUP_PATH_TMPL_SIM
                | GROUP_PATH_TMPL_NS
                | GROUP_TRANSPORT_NS
        )
    };
    let mut db: HashMap<(u32, u32), (usize, PropertyFile)> = HashMap::new();
    let mut group_census: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (idx, (label, path)) in pkgs.iter().enumerate() {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path:?}: {e}"));
        for entry in package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            *group_census
                .entry(label.clone())
                .or_default()
                .entry(format!("{:08X}", entry.id.group))
                .or_default() += 1;
            if !allowed(entry.id.group) {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
                continue;
            };
            db.entry((entry.id.group, entry.id.instance))
                .or_insert((idx, file));
        }
    }
    let label_of = |idx: usize| pkgs[idx].0.clone();

    // 模板继承展平（path entry → 48xxC400 类型模板；lot → lot 父链）。
    let flatten = |file: &PropertyFile| {
        sc_properties::flatten_parent_inheritance_traced(file.clone(), |key: &Key| {
            db.get(&(key.group, key.instance)).map(|(_, f)| f.clone())
        })
        .0
    };
    // 按 instance 解析 path entry（40E0C400 优先，其次 40E1C400 镜像）。
    let resolve_entry = |instance: u32| -> Option<(u32, &(usize, PropertyFile))> {
        if let Some(hit) = db.get(&(GROUP_PATH_ENTRY_SIM, instance)) {
            return Some((GROUP_PATH_ENTRY_SIM, hit));
        }
        db.get(&(GROUP_PATH_ENTRY_NS, instance))
            .map(|hit| (GROUP_PATH_ENTRY_NS, hit))
    };

    // —— path entry 分类表 ——
    #[derive(Clone)]
    struct EntryInfo {
        group: u32,
        pkg: String,
        parent: Option<u32>,
        filters: Vec<u32>,
        width: Option<f32>,
        has_geometry: bool,
        class: &'static str,
    }
    let mut entries: BTreeMap<u32, EntryInfo> = BTreeMap::new();
    let mut filter_members: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new(); // filter -> entries
    let mut class_hist: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut all_instances: BTreeSet<u32> = BTreeSet::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_PATH_ENTRY_SIM && *group != GROUP_PATH_ENTRY_NS {
            continue;
        }
        all_instances.insert(*instance);
        let flat = flatten(file);
        let filters: Vec<u32> = keys_of(&flat, P_FILTERS)
            .iter()
            .map(|k| k.instance)
            .collect();
        let width = match flat.get(P_WIDTH).map(|p| &p.kind) {
            Some(Kind::Scalar(Value::Float(f))) => Some(*f as f32),
            _ => None,
        };
        let has_geometry = flat.get(P_GEOMETRY).is_some();
        let is_extrusion =
            flat.get(P_EXTRUDE_STATIC).is_some() || flat.get(P_EXTRUDE_INVALID).is_some();
        let class = if filters.contains(&PED_FILTER) {
            "pedestrian"
        } else if filters.contains(&VEHICLE_FILTER) {
            "vehicle"
        } else if filters.is_empty() {
            "no_filter"
        } else {
            "other_filter"
        };
        *class_hist.entry(class).or_default() += 1;
        for filter in &filters {
            filter_members.entry(*filter).or_default().insert(*instance);
        }
        entries.insert(
            *instance,
            EntryInfo {
                group: *group,
                pkg: label_of(*idx),
                parent: keys_of(&flat, P_PARENT).first().map(|k| k.instance),
                filters,
                width,
                has_geometry,
                class,
            },
        );
    }

    // —— 载体（nonSim transport）filter ——
    let mut carriers: Vec<serde_json::Value> = Vec::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_TRANSPORT_NS {
            continue;
        }
        if let Some(filter) = keys_of(file, P_CARRIER_FILTER).first() {
            carriers.push(json!({
                "pkg": label_of(*idx),
                "instance": format!("{instance:08X}"),
                "filter": format!("{:08X}", filter.instance),
                "members": filter_members.get(&filter.instance).map(|s| s.len()).unwrap_or(0),
            }));
        }
    }

    // —— 菜单覆盖 ——
    let mut menu_units: BTreeSet<u32> = BTreeSet::new();
    for ((group, _), (_, file)) in db.iter() {
        if *group != GROUP_MENU {
            continue;
        }
        for key in keys_of(file, P_MENU_UNIT) {
            menu_units.insert(key.instance);
        }
    }

    // —— 建筑 → lot → path 分类 ——
    #[derive(Default)]
    struct UnitPaths {
        instance: u32,
        pkg: String,
        has_menu: bool,
        parent: Option<u32>,
        lot_ok: bool,
        entries: Vec<u32>,
        classes: BTreeMap<&'static str, usize>,
        scopes: BTreeMap<String, usize>,
        connects: BTreeMap<String, usize>,
        pairs: usize,
        points: usize,
        entry_count: usize,
        width_min: Option<f32>,
        width_max: Option<f32>,
        missing_entries: Vec<u32>,
    }
    let scope_name = |v: u32| match v {
        0x5757_2DAC => "Global".to_string(),
        0xF44F_784A => "Local".to_string(),
        other => format!("{other:08X}"),
    };
    let connect_name = |v: u32| match v {
        0xF0F6_1CED => "First".to_string(),
        0xC186_DA37 => "None".to_string(),
        0xAA2B_6E31 => "Last".to_string(),
        0xCC13_F2B0 => "FirstAndLast".to_string(),
        other => format!("{other:08X}"),
    };

    let mut units: Vec<UnitPaths> = Vec::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_UNIT {
            continue;
        }
        let mut rec = UnitPaths {
            instance: *instance,
            pkg: label_of(*idx),
            has_menu: menu_units.contains(instance),
            parent: keys_of(file, P_PARENT).first().map(|k| k.instance),
            ..Default::default()
        };
        let flat = flatten(file);
        if let Some(lot_key) = keys_of(&flat, P_LOT_LINK).first() {
            if let Some((_, lot_file)) = db.get(&(GROUP_LOT, lot_key.instance)) {
                rec.lot_ok = true;
                let lot_flat = flatten(lot_file);
                let entry_keys = keys_of(&lot_flat, P_ENTRIES);
                rec.entry_count = entry_keys.len();
                rec.pairs = count_of(&lot_flat, P_PATHS) / 2;
                rec.points = count_of(&lot_flat, P_POINTS);
                let scopes: Vec<u32> = keys_of(&lot_flat, P_SCOPE)
                    .iter()
                    .map(|k| k.instance)
                    .collect();
                let connects: Vec<u32> = keys_of(&lot_flat, P_CONNECT)
                    .iter()
                    .map(|k| k.instance)
                    .collect();
                for scope in &scopes {
                    *rec.scopes.entry(scope_name(*scope)).or_default() += 1;
                }
                for connect in &connects {
                    *rec.connects.entry(connect_name(*connect)).or_default() += 1;
                }
                for key in &entry_keys {
                    rec.entries.push(key.instance);
                    match resolve_entry(key.instance) {
                        None => rec.missing_entries.push(key.instance),
                        Some((_, (_, entry_file))) => {
                            // 合并 sim 与 nonSim 两处记录的展平属性（引擎按
                            // instance 解析，视觉/挤出规格在 nonSim 侧）。
                            let mut flat_merged = PropertyFile::default();
                            for group in [GROUP_PATH_ENTRY_SIM, GROUP_PATH_ENTRY_NS] {
                                if let Some((_, f)) = db.get(&(group, key.instance)) {
                                    for prop in flatten(f).values {
                                        if flat_merged.values.iter().all(|p| p.hash != prop.hash) {
                                            flat_merged.values.push(prop);
                                        }
                                    }
                                }
                            }
                            let _ = entry_file;
                            let filters: Vec<u32> = keys_of(&flat_merged, P_FILTERS)
                                .iter()
                                .map(|k| k.instance)
                                .collect();
                            let class = if filters.contains(&PED_FILTER) {
                                "pedestrian"
                            } else if filters.contains(&VEHICLE_FILTER) {
                                "vehicle"
                            } else if filters.is_empty() {
                                "no_filter"
                            } else {
                                "other_filter"
                            };
                            *rec.classes.entry(class).or_default() += 1;
                            if let Some(Kind::Scalar(Value::Float(w))) =
                                flat_merged.get(P_WIDTH).map(|p| &p.kind)
                            {
                                let w = *w as f32;
                                rec.width_min = Some(rec.width_min.map_or(w, |m| m.min(w)));
                                rec.width_max = Some(rec.width_max.map_or(w, |m| m.max(w)));
                            }
                        }
                    }
                }
            }
        }
        units.push(rec);
    }

    // —— 聚合：RCI(无菜单) vs plop(有菜单) ——
    let summarize = |subset: &[&UnitPaths]| -> serde_json::Value {
        let mut class_units: BTreeMap<&'static str, usize> = BTreeMap::new();
        let (mut no_path, mut has_ped, mut has_veh, mut ped_only, mut veh_any, mut global_any) =
            (0, 0, 0, 0, 0, 0);
        let mut width_missing = 0usize;
        for u in subset {
            if !u.lot_ok || u.entry_count == 0 {
                no_path += 1;
                continue;
            }
            let has_p = u.classes.get("pedestrian").copied().unwrap_or(0) > 0;
            let has_v = u.classes.get("vehicle").copied().unwrap_or(0)
                + u.classes.get("no_filter").copied().unwrap_or(0)
                > 0;
            if has_p {
                has_ped += 1;
            }
            if has_v {
                has_veh += 1;
            }
            if has_p && !has_v {
                ped_only += 1;
            }
            if has_v {
                veh_any += 1;
            }
            if u.scopes.get("Global").copied().unwrap_or(0) > 0 {
                global_any += 1;
            }
            if u.width_min.is_none() {
                width_missing += 1;
            }
            for class in u.classes.keys() {
                *class_units.entry(*class).or_default() += 1;
            }
        }
        let total = subset.len();
        json!({
            "units": total,
            "no_path": no_path,
            "with_pedestrian": has_ped,
            "with_vehicle_or_no_filter": has_veh,
            "pedestrian_only": ped_only,
            "global_scope_any": global_any,
            "width_missing": width_missing,
            "units_by_class": class_units,
            "entry_hist": {
                "0": subset.iter().filter(|u| u.entry_count == 0).count(),
                "1": subset.iter().filter(|u| u.entry_count == 1).count(),
                "2": subset.iter().filter(|u| u.entry_count == 2).count(),
                "3": subset.iter().filter(|u| u.entry_count == 3).count(),
                "4+": subset.iter().filter(|u| u.entry_count >= 4).count(),
            },
        })
    };
    let rci: Vec<&UnitPaths> = units.iter().filter(|u| !u.has_menu).collect();
    let plop: Vec<&UnitPaths> = units.iter().filter(|u| u.has_menu).collect();
    let summary = json!({
        "rci_no_menu": summarize(&rci),
        "plop_with_menu": summarize(&plop),
        "all": summarize(&units.iter().collect::<Vec<_>>()),
    });

    // —— 明细（只保留有 lot 的，便于复查）——
    let details: Vec<serde_json::Value> = units
        .iter()
        .filter(|u| u.lot_ok)
        .map(|u| {
            json!({
                "instance": format!("{:08X}", u.instance),
                "pkg": u.pkg,
                "menu": u.has_menu,
                "parent": u.parent.map(|p| format!("{p:08X}")),
                "entries": u.entries.iter().map(|e| format!("{e:08X}")).collect::<Vec<_>>(),
                "classes": u.classes,
                "scopes": u.scopes,
                "connects": u.connects,
                "pairs": u.pairs,
                "points": u.points,
                "entry_count": u.entry_count,
                "width_min": u.width_min,
                "width_max": u.width_max,
                "missing_entries": u.missing_entries.iter().map(|e| format!("{e:08X}")).collect::<Vec<_>>(),
            })
        })
        .collect();

    let census = json!({
        "inputs": pkgs.iter().map(|(l, p)| json!({"label": l, "path": p.display().to_string()})).collect::<Vec<_>>(),
        "group_census": group_census,
        "path_entry_total": entries.len(),
        "path_entry_class_hist": class_hist,
        "path_entries": entries.iter().map(|(i, e)| json!({
            "instance": format!("{i:08X}"),
            "group": format!("{:08X}", e.group),
            "pkg": e.pkg,
            "parent": e.parent.map(|p| format!("{p:08X}")),
            "filters": e.filters.iter().map(|f| format!("{f:08X}")).collect::<Vec<_>>(),
            "width": e.width,
            "has_geometry": e.has_geometry,
            "class": e.class,
        })).collect::<Vec<_>>(),
        "filter_table": filter_members.iter().map(|(f, members)| json!({
            "filter": format!("{f:08X}"),
            "member_count": members.len(),
            "members": members.iter().take(12).map(|m| format!("{m:08X}")).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "carriers": carriers,
        "summary": summary,
        "units": details,
    });
    std::fs::create_dir_all(&out_dir).expect("out dir");
    let out = out_dir.join("path-census.json");
    std::fs::write(&out, serde_json::to_vec_pretty(&census).expect("json")).expect("write");

    println!("== path entry 分类 ==");
    for (class, n) in &class_hist {
        println!("  {class:<20} {n}");
    }
    println!("== plop（有菜单） ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&summary["plop_with_menu"]).unwrap()
    );
    println!("== RCI（无菜单） ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&summary["rci_no_menu"]).unwrap()
    );
    println!("census -> {}", out.display());
}
