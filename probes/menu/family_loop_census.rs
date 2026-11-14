//! DLC/EP1 家族闭环普查（2026-11-06）。按"组后缀族"识别，不依赖固定前缀：
//!   - 菜单：group 以 C900 结尾且含 0x0975695F(menuUnit)
//!   - 单位：group 以 C000 结尾且含 0x08E17ED7(lotLink)（排除 48xxC000 模板）
//!   - bundle：group 以 C700 结尾
//!   - sink：group 以 C500 结尾且含 0x0A810F28(resource)
//!   - PathEntry：group 以 C400 结尾
//! 闭环判据：同一 bundle 同时含 bin 行 + 0x0A2725CE + 0x0A2725CF。
//!
//! 用法：
//!   cargo run -p probes --release --example family_loop_census -- \
//!     data=<pkg> [data=...] scripts=<pkg> [scripts=...] \
//!     [registry=src-tauri/resources/database_main.s3db] \
//!     [locale_zh=...] [locale_en=...] [out=tmp/family-loop-census/out.json]

use dbpf::Package;
use sc_properties::{Key, Kind, PARENT_HASH, ParseLimits, PropertyFile, Value, locale};
use sc_registry::Registry;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const P_MENU_UNIT: u32 = 0x0975_695F;
const P_MENU_TITLE: u32 = 0x0A09_F5FA;
const P_MENU_UICAT: u32 = 0x0DB9_FC63;
const P_MENU_MODULE_MOUNT: u32 = 0x0E9F_828E;
const P_LOT_LINK: u32 = 0x08E1_7ED7;
const P_SECONDARY: u32 = 0x07E0_AD0A;
const P_ADDON: u32 = 0x0AFE_27CA;
const P_BUNDLE_ENABLE: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
const P_BUNDLE_EXCLUDE: u32 = 0x0A02_09B2;
const P_BIN_RES: u32 = 0x07E0_AD23;
const P_SINK_BIND: u32 = 0x0A27_25CE;
const P_SINK_RULE: u32 = 0x0A27_25CF;
const P_RULE_REFS: u32 = 0x0808_0D58;
const P_EVENT_BASE: u32 = 0x0832_7D48;
const P_SINK_RESOURCE: u32 = 0x0A81_0F28;

const UTIL: [(u32, &str); 3] = [
    (0x4893_9D96, "power"),
    (0x825B_4CA4, "water"),
    (0xD7C6_2954, "garbage"),
];

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

fn texts_of(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let Some(prop) = file.get(hash) else {
        return vec![];
    };
    let mut out = Vec::new();
    let mut push = |v: &Value| {
        if let Value::Text(t) = v {
            out.push(t.instance_id);
        }
    };
    match &prop.kind {
        Kind::Scalar(v) => push(v),
        Kind::Array(values) => values.iter().for_each(push),
        _ => {}
    }
    out
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
    let mut data_pkgs: Vec<(String, PathBuf)> = Vec::new();
    let mut script_pkgs: Vec<(String, PathBuf)> = Vec::new();
    let mut registry_path = PathBuf::from("src-tauri/resources/database_main.s3db");
    let mut locale_zh = None;
    let mut locale_en = None;
    let mut out = PathBuf::from("tmp/family-loop-census/family-loop.json");
    for arg in args {
        let (k, v) = arg.split_once('=').expect("key=value");
        let p = PathBuf::from(v);
        match k {
            "data" => data_pkgs.push((format!("data:{}", data_pkgs.len()), p)),
            "scripts" => script_pkgs.push((format!("scripts:{}", script_pkgs.len()), p)),
            "registry" => registry_path = p,
            "locale_zh" => locale_zh = Some(p),
            "locale_en" => locale_en = Some(p),
            "out" => out = p,
            _ => panic!("unknown arg {k}"),
        }
    }

    // 记录来源包名（用路径尾段更可读）
    let label = |p: &PathBuf| {
        p.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    // —— 装载：所有 B1B104，按内容分类 ——
    struct Rec {
        pkg: String,
        file: PropertyFile,
    }
    let mut menus: Vec<(u32, Rec)> = Vec::new();
    let mut units: HashMap<u32, Rec> = HashMap::new();
    let mut bundles: HashMap<u32, PropertyFile> = HashMap::new();
    let mut sinks: HashMap<u32, PropertyFile> = HashMap::new();
    let mut templates: HashMap<u32, PropertyFile> = HashMap::new();
    let mut group_census: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut path_filters: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new(); // filter -> entries
    let mut carriers: Vec<(String, u32, u32)> = Vec::new(); // pkg, instance, filter
    let mut filter_names: BTreeMap<u32, String> = BTreeMap::new();

    let mut load = |pkgs: &[(String, PathBuf)]| {
        for (_, path) in pkgs {
            let Ok(package) = Package::open(path) else {
                continue;
            };
            let pkg_label = label(path);
            for e in package
                .entries()
                .iter()
                .filter(|e| e.id.type_id == PROPERTY_TYPE)
            {
                *group_census
                    .entry(pkg_label.clone())
                    .or_default()
                    .entry(format!("{:08X}", e.id.group))
                    .or_default() += 1;
                let Ok(data) = package.read(e) else { continue };
                let Ok(file) = PropertyFile::parse_with_limits(&data, ParseLimits::default())
                else {
                    continue;
                };
                let group = e.id.group;
                let suffix = group & 0xFFFF;
                let rec = Rec {
                    pkg: pkg_label.clone(),
                    file,
                };
                match suffix {
                    0xC900 if rec.file.get(P_MENU_UNIT).is_some() => {
                        menus.push((e.id.instance, rec))
                    }
                    0xC000 if (group >> 16) != 0x48 && rec.file.get(P_LOT_LINK).is_some() => {
                        units.insert(e.id.instance, rec);
                    }
                    0xC000 if (group >> 16) == 0x48 => {
                        templates.insert(e.id.instance, rec.file);
                    }
                    0xC700 => {
                        bundles.insert(e.id.instance, rec.file);
                    }
                    0xC500 if rec.file.get(P_SINK_RESOURCE).is_some() => {
                        sinks.insert(e.id.instance, rec.file);
                    }
                    0xC400 => {
                        for k in keys_of(&rec.file, 0x0D64_98E8) {
                            path_filters
                                .entry(k.instance)
                                .or_default()
                                .insert(e.id.instance);
                        }
                    }
                    0xC600 => {
                        if let Some(k) = keys_of(&rec.file, 0x0EB2_2775).first() {
                            carriers.push((pkg_label.clone(), e.id.instance, k.instance));
                        }
                    }
                    _ => {}
                }
            }
        }
    };
    {
        let mut load = load;
        load(&data_pkgs);
        load(&script_pkgs);
    }

    let registry = Registry::open(&registry_path).ok();
    let name_of = |id: u32| -> Option<String> {
        registry
            .as_ref()
            .and_then(|r| r.instances().get(&id))
            .map(|rec| rec.name.trim().to_string())
            .filter(|n| !n.is_empty())
    };
    let load_locale = |p: &PathBuf| -> HashMap<u32, String> {
        let mut map = HashMap::new();
        let Ok(pkg) = Package::open(p) else {
            return map;
        };
        for e in pkg.entries() {
            if e.id.type_id != locale::LOCALE_RESOURCE_TYPE {
                continue;
            }
            let Ok(data) = pkg.read(e) else { continue };
            if let Ok(t) = locale::parse_string_table(&data) {
                for (k, v) in t {
                    map.insert(k, v);
                }
            }
        }
        map
    };
    let zh = locale_zh.map(|p| load_locale(&p)).unwrap_or_default();
    let en = locale_en.map(|p| load_locale(&p)).unwrap_or_default();

    let flatten = |file: &PropertyFile| -> PropertyFile {
        sc_properties::flatten_parent_inheritance(file.clone(), |key: &Key| {
            templates
                .get(&key.instance)
                .cloned()
                .or_else(|| units.get(&key.instance).map(|r| r.file.clone()))
        })
    };
    let sink_resource = |sink_instance: u32| -> Option<u32> {
        sinks
            .get(&sink_instance)
            .and_then(|sv| keys_of(sv, P_SINK_RESOURCE).first().map(|k| k.instance))
    };
    let triple = |bid: u32| -> Option<serde_json::Value> {
        let b = bundles.get(&bid)?;
        let bins = keys_of(b, P_BIN_RES).len();
        let sinks = keys_of(b, P_SINK_BIND);
        let srules = keys_of(b, P_SINK_RULE);
        if bins == 0 || sinks.is_empty() || srules.is_empty() {
            return None;
        }
        let res: Vec<String> = sinks
            .iter()
            .map(|s| {
                let id = sink_resource(s.instance);
                match id.and_then(|i| name_of(i)) {
                    Some(n) => n,
                    None => id.map(|i| format!("{i:08X}")).unwrap_or_else(|| "?".into()),
                }
            })
            .collect();
        let mut rules: BTreeSet<u32> = keys_of(b, P_RULE_REFS).iter().map(|k| k.instance).collect();
        for i in 0..21 {
            if count_of(b, P_EVENT_BASE + i) > 0 {
                rules.insert(P_EVENT_BASE + i);
            }
        }
        Some(json!({
            "bundle": format!("{bid:08X}"),
            "bins": bins,
            "sinks": sinks.len(),
            "sink_rules": srules.len(),
            "resources": res,
            "rules": keys_of(b, P_RULE_REFS).len(),
        }))
    };

    // —— 逐菜单输出 ——
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (menu_inst, rec) in &menus {
        let title = texts_of(&rec.file, P_MENU_TITLE)
            .first()
            .copied()
            .unwrap_or(0);
        let unit_key = keys_of(&rec.file, P_MENU_UNIT).first().cloned();
        let mut row = json!({
            "pkg": rec.pkg,
            "menu": format!("{menu_inst:08X}"),
            "title_zh": zh.get(&title).cloned().unwrap_or_default(),
            "title_en": en.get(&title).cloned().unwrap_or_default(),
            "unit": unit_key.as_ref().map(|k| format!("{:08X}", k.instance)),
            "uicats": keys_of(&rec.file, P_MENU_UICAT).iter().map(|k| format!("{:08X}", k.instance)).collect::<Vec<_>>(),
            "module_mounts": count_of(&rec.file, P_MENU_MODULE_MOUNT),
        });
        if let Some(uk) = unit_key {
            if let Some(unit) = units.get(&uk.instance) {
                row["unit_name"] = json!(name_of(uk.instance).unwrap_or_default());
                let flat = flatten(&unit.file);
                let mut own: BTreeSet<u32> = BTreeSet::new();
                for lv in P_BUNDLE_ENABLE {
                    for k in keys_of(&unit.file, lv) {
                        own.insert(k.instance);
                    }
                }
                for k in keys_of(&unit.file, P_BUNDLE_EXCLUDE) {
                    own.remove(&k.instance);
                }
                let mut triples = Vec::new();
                for bid in &own {
                    if let Some(t) = triple(*bid) {
                        triples.push(t);
                    }
                }
                row["own_bundles"] =
                    json!(own.iter().map(|b| format!("{b:08X}")).collect::<Vec<_>>());
                row["triples"] = json!(triples);
                row["utilities"] = json!(
                    UTIL.iter()
                        .filter(|(id, _)| own.contains(id))
                        .map(|(_, n)| *n)
                        .collect::<Vec<_>>()
                );
                row["secondary_units"] = json!(keys_of(&flat, P_SECONDARY).len());
                row["addon_container"] = json!(flat.get(P_ADDON).is_some());
            } else {
                row["unit_missing"] = json!(true);
            }
        }
        rows.push(row);
    }

    // —— 汇总 ——
    let with_triple = rows
        .iter()
        .filter(|r| {
            r.get("triples")
                .map(|t| t.as_array().map(|a| !a.is_empty()).unwrap_or(false))
                .unwrap_or(false)
        })
        .count();
    let missing = rows
        .iter()
        .filter(|r| r.get("unit_missing").is_some())
        .count();
    let _ = &mut filter_names;
    let filter_table: Vec<serde_json::Value> = path_filters
        .iter()
        .map(|(filter, members)| {
            let users: Vec<String> = carriers
                .iter()
                .filter(|(_, _, f)| f == filter)
                .map(|(pkg, inst, _)| format!("{pkg}:{inst:08X}"))
                .collect();
            json!({
                "filter": format!("{filter:08X}"),
                "name": name_of(*filter),
                "members": members.len(),
                "sample_members": members.iter().take(6).map(|m| format!("{m:08X}")).collect::<Vec<_>>(),
                "carriers": users,
            })
        })
        .collect();
    let doc = json!({
        "packages": {"data": data_pkgs.len(), "scripts": script_pkgs.len()},
        "filter_table": filter_table,
        "group_census": group_census,
        "menus": rows.len(),
        "menus_with_triple": with_triple,
        "units_missing": missing,
        "rows": rows,
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out, serde_json::to_vec_pretty(&doc).unwrap()).expect("write");
    println!(
        "menus={} with_triple={} unit_missing={}",
        doc["menus"], doc["menus_with_triple"], doc["units_missing"]
    );
    for r in doc["rows"].as_array().unwrap() {
        let title = r["title_en"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or(r["title_zh"].as_str())
            .unwrap_or("?");
        let tr: Vec<String> = r["triples"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| t["bundle"].as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default();
        println!(
            "  {title} | {} | triples {:?}",
            r["unit"].as_str().unwrap_or("-"),
            tr
        );
    }
    println!("out -> {}", out.display());
}
