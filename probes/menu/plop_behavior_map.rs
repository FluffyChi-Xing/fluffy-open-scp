//! 城市一级菜单 plop 行为闭环普查（2026-11-06）。
//!
//! 用法：
//!   cargo run -p probes --release --example plop_behavior_map -- \
//!     scripts=<scripts.pkg> game=<game.pkg> sheet=<xlsx|json 输出路径> \
//!     [locale_zh=<zh-tw Data.package>] [locale_en=<en-us Data.package>]
//!
//! 输出每个一级菜单（按 Menu2 parent 链的根节点分组）下的工具记录：
//! 标题（中/英）、目标 Unit、核心闭环 bundle（同时带 sink 绑定
//! 0x0A2725CE 与规则绑定 0x0A2725CF 的 bundle）、sink 资源、规则数、
//! 水电三件套是否在继承链中、扩展字段（次级单位/addon/模块挂载）。
//!
//! 分类节点（48E1EB00）本身没有标题，一级菜单名由该组下工具标题归纳；
//! 探针输出根 id + 成员标题，便于人工命名。

use dbpf::Package;
use sc_properties::{locale, Key, Kind, ParseLimits, PropertyFile, Value, PARENT_HASH};
use sc_registry::Registry;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const GROUP_UNIT: u32 = 0x40E0_C000;
const GROUP_LOT: u32 = 0x40E1_C000;
const GROUP_MENU: u32 = 0x0987_8A01;
const GROUP_MENU_NODE: u32 = 0x48E1_EB00;
const GROUP_BUNDLE: u32 = 0x40E0_C700;
const GROUP_SINK: u32 = 0x40E0_C500;
const GROUP_PATH_ENTRY: u32 = 0x40E0_C400;

const P_PARENT: u32 = PARENT_HASH;
const P_MENU_UNIT: u32 = 0x0975_695F;
const P_MENU_UICAT: u32 = 0x0DB9_FC63;
const P_MENU_TITLE: u32 = 0x0A09_F5FA;
const P_TOOL_TYPE: u32 = 0x08F6_72C7;
const P_MODULE_MOUNT: u32 = 0x0E9F_828E;
const P_BUNDLE_ENABLE: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
const P_BUNDLE_EXCLUDE: u32 = 0x0A02_09B2;
const P_BIN_RES: u32 = 0x07E0_AD23;
const P_SINK_BIND: u32 = 0x0A27_25CE;
const P_SINK_RULE: u32 = 0x0A27_25CF;
const P_RULE_REFS: u32 = 0x0808_0D58;
const P_EVENT_BASE: u32 = 0x0832_7D48;
const P_SINK_RESOURCE: u32 = 0x0A81_0F28;
const P_SINK_CONNECTED: u32 = 0x0DA7_921B;
const P_SINK_INVERT: u32 = 0x0B55_5629;
const P_SECONDARY: u32 = 0x07E0_AD0A;
const P_ADDON: u32 = 0x0AFE_27CA;
const P_LOT_LINK: u32 = 0x08E1_7ED7;

const UTIL_BUNDLES: [(u32, &str); 3] = [
    (0x4893_9D96, "power"),
    (0x825B_4CA4, "water"),
    (0xD7C6_2954, "garbage"),
];

fn keys_of(file: &PropertyFile, hash: u32) -> Vec<Key> {
    let Some(prop) = file.get(hash) else { return vec![] };
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

fn texts_of(file: &PropertyFile, hash: u32) -> Vec<(u32, u32)> {
    let Some(prop) = file.get(hash) else { return vec![] };
    let mut out = Vec::new();
    let mut push = |v: &Value| {
        if let Value::Text(t) = v { out.push((t.table_id, t.instance_id)); }
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
    let mut scripts = None;
    let mut game = None;
    let mut locale_zh = None;
    let mut locale_en = None;
    let mut sheet = PathBuf::from("tmp/plop-behavior-map-20261106/plop-map.json");
    let mut registry_path = PathBuf::from("src-tauri/resources/database_main.s3db");
    for arg in args {
        let (k, v) = arg.split_once('=').expect("key=value");
        let p = PathBuf::from(v);
        match k {
            "scripts" => scripts = Some(p),
            "game" => game = Some(p),
            "locale_zh" => locale_zh = Some(p),
            "locale_en" => locale_en = Some(p),
            "sheet" => sheet = p,
            "registry" => registry_path = p,
            _ => panic!("unknown arg {k}"),
        }
    }
    let scripts = scripts.expect("scripts=");
    let game = game.expect("game=");

    // —— 装载 ——
    let load = |path: &PathBuf, groups: &[u32]| -> Vec<(u32, u32, PropertyFile)> {
        let pkg = Package::open(path).unwrap_or_else(|e| panic!("open {path:?}: {e}"));
        let mut out = Vec::new();
        for e in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if !groups.contains(&e.id.group) {
                continue;
            }
            let Ok(data) = pkg.read(e) else { continue };
            let Ok(f) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else { continue };
            out.push((e.id.group, e.id.instance, f));
        }
        out
    };
    const GROUP_TEMPLATE: u32 = 0x48E0_C000;
    let mut unit_map: HashMap<u32, PropertyFile> = HashMap::new();
    let mut template_map: HashMap<u32, PropertyFile> = HashMap::new();
    let mut bundle_map: HashMap<u32, PropertyFile> = HashMap::new();
    let mut sink_map: HashMap<u32, PropertyFile> = HashMap::new();
    for (group, inst, f) in load(&scripts, &[GROUP_UNIT, GROUP_TEMPLATE, GROUP_BUNDLE, GROUP_SINK]) {
        match group {
            GROUP_UNIT => { unit_map.insert(inst, f); }
            GROUP_TEMPLATE => { template_map.insert(inst, f); }
            GROUP_BUNDLE => { bundle_map.insert(inst, f); }
            GROUP_SINK => { sink_map.insert(inst, f); }
            _ => {}
        }
    }
    let registry = Registry::open(&registry_path).ok();
    let name_of = |id: u32| -> Option<String> {
        registry.as_ref()
            .and_then(|r| r.instances().get(&id))
            .map(|rec| rec.name.trim().to_string())
            .filter(|n| !n.is_empty())
    };
    let mut menu_records: Vec<(u32, PropertyFile)> = Vec::new();
    let mut menu_nodes: HashMap<u32, PropertyFile> = HashMap::new();
    for (group, inst, f) in load(&game, &[GROUP_MENU, GROUP_MENU_NODE]) {
        match group {
            GROUP_MENU => menu_records.push((inst, f)),
            GROUP_MENU_NODE => { menu_nodes.insert(inst, f); }
            _ => {}
        }
    }
    // 路网 entry（判断有无车辆 path 用）
    let mut path_entries: BTreeSet<u32> = BTreeSet::new();
    for (_, inst, _) in load(&scripts, &[GROUP_PATH_ENTRY]) {
        path_entries.insert(inst);
    }
    let mut lot_map: HashMap<u32, PropertyFile> = HashMap::new();
    for (group, inst, f) in load(&game, &[GROUP_LOT]) {
        if group == GROUP_LOT { lot_map.insert(inst, f); }
    }

    // —— locale ——
    let load_locale = |path: &PathBuf| -> HashMap<u32, String> {
        let mut map = HashMap::new();
        let Ok(pkg) = Package::open(path) else { return map };
        for e in pkg.entries() {
            if e.id.type_id != locale::LOCALE_RESOURCE_TYPE { continue; }
            let Ok(data) = pkg.read(e) else { continue };
            if let Ok(t) = locale::parse_string_table(&data) {
                for (k, v) in t { map.insert(k, v); }
            }
        }
        map
    };
    let zh = locale_zh.map(|p| load_locale(&p)).unwrap_or_default();
    let en = locale_en.map(|p| load_locale(&p)).unwrap_or_default();

    // —— 继承展平 ——
    let flatten_unit = |file: &PropertyFile| -> PropertyFile {
        sc_properties::flatten_parent_inheritance(file.clone(), |key: &Key| {
            if key.group == GROUP_TEMPLATE {
                template_map.get(&key.instance).cloned()
            } else {
                unit_map.get(&key.instance).cloned()
            }
        })
    };

    // —— 菜单树根 ——
    let root_of = |mut node: u32| -> u32 {
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(node) { return node; }
            let Some(n) = menu_nodes.get(&node) else { return node };
            match keys_of(n, P_PARENT).first() {
                Some(p) if p.instance != 0 => node = p.instance,
                _ => return node,
            }
        }
    };

    // —— 核心 bundle 判定 ——
    let bundle_summary = |bid: u32| -> serde_json::Value {
        let Some(b) = bundle_map.get(&bid) else {
            return json!({"instance": format!("{bid:08X}"), "missing": true});
        };
        let sinks: Vec<u32> = keys_of(b, P_SINK_BIND).iter().map(|k| k.instance).collect();
        let sink_rules: Vec<u32> = keys_of(b, P_SINK_RULE).iter().map(|k| k.instance).collect();
        let bins: Vec<u32> = keys_of(b, P_BIN_RES).iter().map(|k| k.instance).collect();
        let mut rules: BTreeSet<u32> = keys_of(b, P_RULE_REFS).iter().map(|k| k.instance).collect();
        let mut events = 0usize;
        for i in 0..21 {
            if count_of(b, P_EVENT_BASE + i) > 0 { events += 1; }
        }
        let sink_res: Vec<String> = sinks.iter().map(|s| {
            let instance = sink_map.get(s).and_then(|sv| {
                keys_of(sv, P_SINK_RESOURCE).first().map(|k| k.instance)
            });
            match instance {
                Some(id) => match name_of(id) {
                    Some(n) => format!("{id:08X}({n})"),
                    None => format!("{id:08X}"),
                },
                None => "?".into(),
            }
        }).collect();
        json!({
            "instance": format!("{bid:08X}"),
            "bins": bins.len(),
            "sinks": sinks.iter().map(|s| format!("{s:08X}")).collect::<Vec<_>>(),
            "sink_resources": sink_res,
            "sink_rules": sink_rules.iter().map(|s| format!("{s:08X}")).collect::<Vec<_>>(),
            "rules": rules.iter().map(|r| format!("{r:08X}")).collect::<Vec<_>>(),
            "event_slots": events,
            "is_core_loop": !sinks.is_empty() && !sink_rules.is_empty() && !bins.is_empty(),
        })
    };

    // —— 遍历工具记录 ——
    #[derive(Default)]
    struct ToolRow {
        menu: u32,
        root: u32,
        title_zh: String,
        title_en: String,
        tool_type: Option<u32>,
        unit: Option<u32>,
        unit_parent: Option<u32>,
        core_bundles: Vec<serde_json::Value>,
        all_bundles: Vec<u32>,
        utilities: Vec<&'static str>,
        secondary: usize,
        addon: bool,
        module_mounts: usize,
        lot_paths: Option<usize>,
        has_vehicle_path: Option<bool>,
        uicats: Vec<u32>,
        unit_name: String,
        own_bundles: Vec<serde_json::Value>,
    }
    let mut rows: Vec<ToolRow> = Vec::new();
    for (menu_inst, file) in &menu_records {
        let parent = keys_of(file, P_PARENT).first().map(|k| k.instance).unwrap_or(0);
        let root = root_of(parent);
        let title = texts_of(file, P_MENU_TITLE).first().map(|(_, id)| *id).unwrap_or(0);
        let uicats: Vec<u32> = keys_of(file, P_MENU_UICAT).iter().map(|k| k.instance).collect();
        let mut row = ToolRow {
            menu: *menu_inst,
            root,
            title_zh: zh.get(&title).cloned().unwrap_or_default(),
            title_en: en.get(&title).cloned().unwrap_or_default(),
            tool_type: keys_of(file, P_TOOL_TYPE).first().map(|k| k.instance),
            module_mounts: count_of(file, P_MODULE_MOUNT),
            uicats,
            ..Default::default()
        };
        if let Some(uk) = keys_of(file, P_MENU_UNIT).first() {
            row.unit = Some(uk.instance);
            row.unit_name = name_of(uk.instance).unwrap_or_default();
            if let Some(unit_file) = unit_map.get(&uk.instance) {
                row.unit_parent = keys_of(unit_file, P_PARENT).first().map(|k| k.instance);
                let flat = flatten_unit(unit_file);
                let mut enabled: BTreeSet<u32> = BTreeSet::new();
                for level in P_BUNDLE_ENABLE {
                    for k in keys_of(&flat, level) { enabled.insert(k.instance); }
                }
                for k in keys_of(&flat, P_BUNDLE_EXCLUDE) { enabled.remove(&k.instance); }
                row.all_bundles = enabled.iter().copied().collect();
                // 自有 bundle（只看 Unit 自身文件，区分"继承"）
                let mut own: BTreeSet<u32> = BTreeSet::new();
                for level in P_BUNDLE_ENABLE {
                    for k in keys_of(unit_file, level) { own.insert(k.instance); }
                }
                for k in keys_of(unit_file, P_BUNDLE_EXCLUDE) { own.remove(&k.instance); }
                for bid in &own {
                    row.own_bundles.push(bundle_summary(*bid));
                }
                for (ub, name) in UTIL_BUNDLES {
                    if enabled.contains(&ub) { row.utilities.push(name); }
                }
                row.secondary = keys_of(&flat, P_SECONDARY).len();
                row.addon = flat.get(P_ADDON).is_some();
                for bid in &row.all_bundles {
                    let s = bundle_summary(*bid);
                    if s["is_core_loop"].as_bool().unwrap_or(false) {
                        row.core_bundles.push(s);
                    }
                }
                if let Some(lot_key) = keys_of(&flat, P_LOT_LINK).first() {
                    if let Some(lot) = lot_map.get(&lot_key.instance) {
                        let entries: Vec<u32> =
                            keys_of(lot, 0x0CC8_FE7A).iter().map(|k| k.instance).collect();
                        row.lot_paths = Some(entries.len());
                    }
                }
            }
        }
        rows.push(row);
    }

    // —— 分组 ——
    let mut groups: BTreeMap<u32, Vec<&ToolRow>> = BTreeMap::new();
    for r in &rows {
        if r.unit.is_some() {
            groups.entry(r.root).or_default().push(r);
        }
    }
    let mut out_groups: Vec<serde_json::Value> = Vec::new();
    for (root, tools) in &groups {
        out_groups.push(json!({
            "root": format!("{root:08X}"),
            "tool_count": tools.len(),
            "member_titles": tools.iter().map(|t| json!({
                "zh": t.title_zh, "en": t.title_en, "unit": t.unit.map(|u| format!("{u:08X}"))
            })).collect::<Vec<_>>(),
            "tools": tools.iter().map(|t| json!({
                "menu": format!("{:08X}", t.menu),
                "title_zh": t.title_zh,
                "title_en": t.title_en,
                "tool_type": t.tool_type.map(|v| format!("{v:08X}")),
                "unit": t.unit.map(|v| format!("{v:08X}")),
                "unit_parent": t.unit_parent.map(|v| format!("{v:08X}")),
                "core_bundles": t.core_bundles,
                "utilities": t.utilities,
                "secondary_units": t.secondary,
                "addon_container": t.addon,
                "module_mounts": t.module_mounts,
                "bundle_count": t.all_bundles.len(),
                "lot_path_entries": t.lot_paths,
            })).collect::<Vec<_>>(),
        }));
    }

    let mut category_names: BTreeMap<String, String> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.unit.is_some()) {
        for cat in &r.uicats {
            if let Some(n) = name_of(*cat) { category_names.entry(format!("{cat:08X}")).or_insert(n); }
        }
    }
    let mut category_groups: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.unit.is_some()) {
        for cat in &r.uicats {
            category_groups.entry(format!("{cat:08X}")).or_default().push(json!({
                "menu": format!("{:08X}", r.menu),
                "title_zh": r.title_zh,
                "title_en": r.title_en,
                "unit": r.unit.map(|v| format!("{v:08X}")),
                "unit_name": r.unit_name,
                "own_tool": r.tool_type.is_some(),
                "core_bundles": r.core_bundles.iter().map(|b| b["instance"].clone()).collect::<Vec<_>>(),
                "own_bundles": r.own_bundles,
                "utilities": r.utilities,
                "secondary_units": r.secondary,
                "addon_container": r.addon,
                "module_mounts": r.module_mounts,
                "lot_path_entries": r.lot_paths,
            }));
        }
    }
    if let Some(parent) = sheet.parent() { std::fs::create_dir_all(parent).ok(); }
    let doc = json!({
        "menu_records": menu_records.len(),
        "tool_records": rows.iter().filter(|r| r.tool_type.is_some()).count(),
        "plop_records": rows.iter().filter(|r| r.unit.is_some()).count(),
        "groups": out_groups,
        "category_groups": category_groups,
        "category_names": category_names,
    });
    std::fs::write(&sheet, serde_json::to_vec_pretty(&doc).unwrap()).expect("write");

    println!("menu={} tools={} plop_entries={} groups={}", menu_records.len(),
        rows.iter().filter(|r| r.tool_type.is_some()).count(),
        rows.iter().filter(|r| r.unit.is_some()).count(), groups.len());
    for (root, tools) in &groups {
        let names: Vec<String> = tools.iter().filter(|t| !t.title_en.is_empty())
            .map(|t| t.title_en.clone()).take(6).collect();
        println!("root {root:08X} ({} tools): {}", tools.len(), names.join(" | "));
    }
    println!("sheet -> {}", sheet.display());
}
