//! 行为普查探针（2026-11-06）：扫描 SimCityData 各包 + EcoGame 脚本库中
//! `0x00B1B104` 属性记录，输出建筑行为族普查 JSON：
//! - 每个行为族（水电/岗位/地价/吸引力/容量/维护/税…）在多少 Unit
//!   （组 `40E0C000`，模板继承展平后）上出现，叶级声明 vs 模板继承；
//! - 行为族 × bundle（`0x0A0209A5`）共现分布 —— 行为是否依赖规则容器；
//! - 行为族 × 菜单入口（`09878A01` 的 `0x0975695F` menuUnit）覆盖 ——
//!   行为是否依赖 Menu2（RCI 单位应无菜单也有行为）；
//! - lot 路网编码普查（`0x0D9AA2BB` roadConnect / `0x0D9A4E54` scope /
//!   `0x0CC8FE7A` PathEntry 表 / `0x0D7970ED` 城市看板）—— 原版水电消费者
//!   如何编码"接入路网"（Global+First）；
//! - 模板层（`48E0C000`）行为默认值、主表（`40800200`）声明、bundle 规则
//!   引用（`0x08080D58..` / `0x08327D48..` 事件槽）。
//! 用法：cargo run -p probes --release --example behavior_census -- \
//!   scripts=<pkg> game=<pkg> graphics=<pkg> dlc0=<pkg> ep1=<pkg> [out=<dir>]
//! 参数顺序即 (group,instance) 解析优先级（首参最高；建议 scripts 在前，
//! 对应脚本库 priority 100 > 普通包 0）。输出 `<out>/census.json`
//! （默认 `tmp/behavior-census-20261106/`）+ stdout 摘要。

use dbpf::Package;
use sc_properties::{
    Key, Kind, PARENT_HASH, ParseLimits, PropertyFile, Value, flatten_parent_inheritance_traced,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const GROUP_UNIT: u32 = 0x40E0_C000;
const GROUP_TEMPLATE: u32 = 0x48E0_C000;
const GROUP_LOT: u32 = 0x40E1_C000;
const GROUP_MENU: u32 = 0x0987_8A01;
const GROUP_BUNDLE: u32 = 0x40E0_C700;
const GROUP_MASTER: u32 = 0x4080_0200;

// —— Unit 属性 hash（docs/re/building-eco-chain-and-s17-diagnosis-20261106.md §2/§3）——
// bundle 启用实为四级数组 0x0A0209A2..A5（GB/cEcoUnitEntry/FillFromProps.c:261-275），
// 排除 0x0A0209B2；资源 bin 三列 0x07E0AD23(资源)/0x07E0AD24(容量)/0x08635E12(值)。
const P_BUNDLE_ENABLE_LEVELS: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
const P_BUNDLE_EXCLUDE: u32 = 0x0A02_09B2;
const P_BIN_RESOURCES: u32 = 0x07E0_AD23;
const P_LOT_LINK: u32 = 0x08E1_7ED7;
/// PDB 实证资源 ID（BuildTotalsJSValue 等；无字典，运行时注册）。
const RES_POWER: u32 = 0x8E8B_4E10;
const RES_WATER: u32 = 0x9E3C_3DFA;
const RES_GARBAGE: u32 = 0x7CB2_3A4E;
// —— Lot 属性 hash ——
const P_ROAD_CONNECT: u32 = 0x0D9A_A2BB;
const P_PATH_SCOPE: u32 = 0x0D9A_4E54;
const P_PATH_ENTRY_TABLE: u32 = 0x0CC8_FE7A;
const P_PATH_SEGMENTS: u32 = 0x0CAA_6841;
const P_BOARD: u32 = 0x0D79_70ED;
// —— Menu2 属性 hash ——
const P_MENU_UNIT: u32 = 0x0975_695F;
const P_MENU_CATEGORY_ID: u32 = 0x0975_695E;
const P_MENU_UICATEGORIES: u32 = 0x0DB9_FC63;
const P_MENU_MODULE_MOUNT: u32 = 0x0E9F_828E;
const P_MENU_TITLE: u32 = 0x0A09_F5FA;
// —— 主表 / bundle ——
const P_MASTER_VERSION: u32 = 0x09EE_B11C;
const BUNDLE_RULE_REFS: [u32; 3] = [0x0808_0D58, 0x0808_0D59, 0x0808_0D5A];
const BUNDLE_EVENT_SLOT_BASE: u32 = 0x0832_7D48;
const BUNDLE_EVENT_SLOT_COUNT: u32 = 21;

// PDB 已定案的枚举值（include/SC/tPathScope_ord2502.h、tRoadConnect_ord2503.h）
const V_SCOPE_LOCAL: u32 = 0xF44F_784A;
const V_SCOPE_GLOBAL: u32 = 0x5757_2DAC;
const V_ROAD_NONE: u32 = 0xC186_DA37;
const V_ROAD_FIRST: u32 = 0xF0F6_1CED;
const V_ROAD_LAST: u32 = 0xAA2B_6E31;
const V_ROAD_FIRST_AND_LAST: u32 = 0xCC13_F2B0;

/// 行为族 → 构成属性 hash（任一存在即记族存在）。
const FAMILIES: &[(&str, &[u32])] = &[
    (
        "power_consumer",
        &[0x0AFB_9882, 0x0B88_904E, 0x0C09_DAA0, 0x0C09_DAB2],
    ),
    ("water_consumer", &[0x0C09_DA65, 0x0C09_DA5A, 0x0C09_DA6E]),
    ("garbage_producer", &[0x0C21_6562]),
    (
        "jobs",
        &[
            0x0A79_17C3,
            0x0A79_17C5,
            0x0A79_17C6,
            0x0A79_17C7,
            0x0A79_17C8,
        ],
    ),
    ("tax", &[0x0A79_17C4]),
    (
        "land_value",
        &[0x0E58_5AE0, 0x0E58_5AE1, 0x0E58_5B00, 0x0E58_5B01],
    ),
    ("attraction", &[0x0E54_2F39]),
    ("capacity", &[0x0AD7_FAC9]),
    ("maintenance", &[0x09AE_19D7]),
    ("cost", &[0x0A8A_41DA]),
    ("category", &[0x0AFE_27B1]),
    ("secondary_units", &[0x07E0_AD0A]),
    ("expenses_keys", &[0x0AD6_5F07, 0x0AD6_5F08]),
    ("tourism_income", &[0x0EE2_A50D, 0x0EE2_A50E, 0x0EE2_A50F]),
    ("unlock", &[0x0BF5_D2E3, 0x0BF5_D2E4]),
    ("quota_shared", &[0x0E51_C90F]),
    ("module_limit", &[0x0CC8_BE51]),
];

/// 主表扫描组声明 hash → 语义名（docs §2 主表行）。
const MASTER_DECLS: &[(u32, &str)] = &[
    (0x07E0_AC00, "unit"),
    (0x07E0_AC01, "resource"),
    (0x07E0_AC02, "sim_properties"),
    (0x07E0_AC03, "map"),
    (0x0913_8113, "zone"),
    (0x0913_8114, "path_entry"),
    (0x0A30_A557, "point"),
    (0x0A30_A558, "transport"),
    (0x0A30_A559, "bundle"),
    (0x0C94_7ABC, "ns_props"),
];

/// 只驻留模拟侧属性组（单位/模板/lot/bundle/PathEntry/主表/菜单 + 各自
/// parent 组），避免把 decal/图集等高频组拉进内存。
fn group_allowed(group: u32) -> bool {
    matches!(group,
        0x40E0_0000..=0x40E0_FFFF
            | 0x40E1_0000..=0x40E1_FFFF
            | 0x4080_0000..=0x4080_FFFF
            | 0x48E0_0000..=0x48E0_FFFF
            | 0x48E1_0000..=0x48E1_FFFF
            | GROUP_MENU)
}

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
        Kind::Scalar(Value::Bool(v)) => Some(*v as u32),
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

fn families_of(file: &PropertyFile) -> BTreeSet<&'static str> {
    FAMILIES
        .iter()
        .filter(|(_, hashes)| hashes.iter().any(|h| file.get(*h).is_some()))
        .map(|(name, _)| *name)
        .collect()
}

fn hex_vec(values: &[u32]) -> Vec<String> {
    values.iter().map(|v| format!("{v:08X}")).collect()
}

#[derive(Clone)]
struct LotRec {
    pkg: String,
    road_connect: Vec<u32>,
    scopes: Vec<u32>,
    path_entries: usize,
    path_segments: usize,
    board: Option<u32>,
}

impl LotRec {
    /// 任一位置的 (scope, connect) == (Global, First) —— 原版"接入路网"编码。
    fn global_first(&self) -> bool {
        self.scopes
            .iter()
            .zip(self.road_connect.iter())
            .any(|(s, c)| *s == V_SCOPE_GLOBAL && *c == V_ROAD_FIRST)
    }
    fn local_none_only(&self) -> bool {
        !self.road_connect.is_empty()
            && !self.global_first()
            && self
                .scopes
                .iter()
                .zip(self.road_connect.iter())
                .all(|(s, c)| *s == V_SCOPE_LOCAL && *c == V_ROAD_NONE)
    }
    fn to_json(&self) -> serde_json::Value {
        json!({
            "pkg": self.pkg,
            "road_connect": hex_vec(&self.road_connect),
            "scopes": hex_vec(&self.scopes),
            "path_entries": self.path_entries,
            "path_segments": self.path_segments,
            "board": self.board.map(|b| format!("{b:08X}")),
            "global_first": self.global_first(),
            "local_none_only": self.local_none_only(),
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pkgs: Vec<(String, PathBuf)> = Vec::new();
    let mut out_dir = PathBuf::from("tmp/behavior-census-20261106");
    for arg in args {
        if let Some(rest) = arg.strip_prefix("out=") {
            out_dir = rest.into();
            continue;
        }
        let (label, path) = arg.split_once('=').expect("参数形如 label=path");
        pkgs.push((label.to_string(), PathBuf::from(path)));
    }
    assert!(!pkgs.is_empty(), "至少一个 label=path 输入");

    // —— 装载：全包 00B1B104 计数；允许组驻留内存（首参优先）——
    let mut db: HashMap<(u32, u32), (usize, PropertyFile)> = HashMap::new();
    let mut group_census: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    // 空表普查（用户 2026-11-06 观察 1）：4 字节 = 计数头 0 条属性的空 property。
    // empty_by_group: (pkg, group) -> [空表数, 总表数, 1-3 属性表数]
    let mut empty_by_group: BTreeMap<(String, u32), (usize, usize, usize)> = BTreeMap::new();
    let mut load_stats: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (idx, (label, path)) in pkgs.iter().enumerate() {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path:?}: {e}"));
        let mut total = 0usize;
        let mut read_fail = 0usize;
        let mut parse_fail = 0usize;
        let mut resident = 0usize;
        for entry in package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            total += 1;
            *group_census
                .entry(label.clone())
                .or_default()
                .entry(format!("{:08X}", entry.id.group))
                .or_default() += 1;
            if !group_allowed(entry.id.group) {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                read_fail += 1;
                continue;
            };
            let Ok(file) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
                parse_fail += 1;
                continue;
            };
            resident += 1;
            let stats = empty_by_group
                .entry((label.clone(), entry.id.group))
                .or_insert((0, 0, 0));
            stats.1 += 1;
            if file.values.is_empty() {
                stats.0 += 1;
            } else if file.values.len() <= 3 {
                stats.2 += 1;
            }
            db.entry((entry.id.group, entry.id.instance))
                .or_insert((idx, file));
        }
        load_stats.insert(
            label.clone(),
            json!({"total_b1b104": total, "resident": resident, "read_fail": read_fail, "parse_fail": parse_fail}),
        );
        eprintln!(
            "[load] {label}: {total} 条 B1B104（驻留 {resident}，读失败 {read_fail}，解析失败 {parse_fail}）"
        );
    }

    let pkg_label = |idx: usize| pkgs[idx].0.clone();

    // —— lot 普查（40E1C000，含 parent 展平）——
    let mut lots: HashMap<u32, LotRec> = HashMap::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_LOT {
            continue;
        }
        let (flat, _) = flatten_parent_inheritance_traced(file.clone(), |key: &Key| {
            db.get(&(key.group, key.instance)).map(|(_, f)| f.clone())
        });
        lots.insert(
            *instance,
            LotRec {
                pkg: pkg_label(*idx),
                road_connect: keys_of(&flat, P_ROAD_CONNECT)
                    .iter()
                    .map(|k| k.instance)
                    .collect(),
                scopes: keys_of(&flat, P_PATH_SCOPE)
                    .iter()
                    .map(|k| k.instance)
                    .collect(),
                path_entries: keys_of(&flat, P_PATH_ENTRY_TABLE).len(),
                path_segments: count_of(&flat, P_PATH_SEGMENTS),
                board: keys_of(&flat, P_BOARD).first().map(|k| k.instance),
            },
        );
    }

    // —— 菜单普查（09878A01）——
    let mut menu_units: HashMap<u32, Vec<u32>> = HashMap::new(); // unit instance -> menu instances
    let mut menu_parent_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut menu_uicat_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut menu_kcategory_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut menu_title_table_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut menu_total = 0usize;
    let mut menu_with_module_mount = 0usize;
    for ((group, menu_instance), (_, file)) in db.iter() {
        if *group != GROUP_MENU {
            continue;
        }
        menu_total += 1;
        if let Some(parent) = keys_of(file, PARENT_HASH).first() {
            *menu_parent_hist
                .entry(format!("{:08X}", parent.instance))
                .or_default() += 1;
        }
        for cat in keys_of(file, P_MENU_UICATEGORIES) {
            *menu_uicat_hist
                .entry(format!("{:08X}", cat.instance))
                .or_default() += 1;
        }
        if let Some(k) = keys_of(file, P_MENU_CATEGORY_ID).first() {
            *menu_kcategory_hist
                .entry(format!("{:08X}", k.instance))
                .or_default() += 1;
        }
        // 标题既可能是 Scalar(Text) 也可能是 text[] 数组（实测 Menu2 为数组）。
        if let Some(prop) = file.get(P_MENU_TITLE) {
            let mut tables: Vec<u32> = Vec::new();
            match &prop.kind {
                Kind::Scalar(Value::Text(t)) => tables.push(t.table_id),
                Kind::Array(values) => {
                    for value in values {
                        if let Value::Text(t) = value {
                            tables.push(t.table_id);
                        }
                    }
                }
                _ => {}
            }
            for table in tables {
                *menu_title_table_hist
                    .entry(format!("{table:08X}"))
                    .or_default() += 1;
            }
        }
        if file.get(P_MENU_MODULE_MOUNT).is_some() {
            menu_with_module_mount += 1;
        }
        for unit_key in keys_of(file, P_MENU_UNIT) {
            menu_units
                .entry(unit_key.instance)
                .or_default()
                .push(*menu_instance);
        }
    }

    // —— Unit 普查（40E0C000，模板继承展平）——
    struct UnitRec {
        pkg: String,
        instance: u32,
        parent: Option<u32>,
        chain: Vec<u32>,
        bundles: Vec<u32>,
        bundles_excluded: Vec<u32>,
        own_families: BTreeSet<&'static str>,
        families: BTreeSet<&'static str>,
        bin_core: BTreeSet<u32>,
        bin_bundle_only: BTreeSet<u32>,
        utility_amounts: BTreeMap<u32, i64>,
        has_menu: bool,
        /// 叶级（未展平）属性条数：空表=0；变体通常极简。
        leaf_props: usize,
        /// 结构分类：empty / variant / primary（见 classify_unit）。
        class: &'static str,
        lot: Option<LotRec>,
    }
    // 变体判据（用户 2026-11-06 指示）：结构极简且无行为消费的 property 不是主
    // property。这里把"无 bundle、无资源 bin、无菜单入口、叶级属性≤12"判为变体；
    // 0 条属性判为空表；其余为 primary。family_stats 默认只统计 primary。
    const VARIANT_MAX_PROPS: usize = 12;
    let classify_unit = |leaf_props: usize,
                         bundles: &[u32],
                         bins: &BTreeSet<u32>,
                         has_menu: bool|
     -> &'static str {
        if leaf_props == 0 {
            "empty"
        } else if bundles.is_empty()
            && bins.is_empty()
            && !has_menu
            && leaf_props <= VARIANT_MAX_PROPS
        {
            "variant"
        } else {
            "primary"
        }
    };
    let mut units: Vec<UnitRec> = Vec::new();
    let mut unit_freq_own: BTreeMap<u32, usize> = BTreeMap::new();
    let mut unit_freq_flat: BTreeMap<u32, usize> = BTreeMap::new();
    let mut resource_census: BTreeMap<u32, (usize, usize, usize)> = BTreeMap::new(); // res -> (units, core, bundle-only)
    let mut chain_breaks = 0usize;
    let int_array = |file: &PropertyFile, hash: u32| -> Vec<i64> {
        match file.get(hash).map(|p| &p.kind) {
            Some(Kind::Array(values)) => values
                .iter()
                .filter_map(|v| match v {
                    Value::Int32(n) => Some(i64::from(*n)),
                    Value::UInt32(n) => Some(i64::from(*n)),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        }
    };
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_UNIT {
            continue;
        }
        for prop in &file.values {
            *unit_freq_own.entry(prop.hash).or_default() += 1;
        }
        let (flat, chain) = flatten_parent_inheritance_traced(file.clone(), |key: &Key| {
            db.get(&(key.group, key.instance)).map(|(_, f)| f.clone())
        });
        if file.get(PARENT_HASH).is_some() && chain.is_empty() {
            chain_breaks += 1; // 声明了 parent 但第一跳就解析失败
        }
        for prop in &flat.values {
            *unit_freq_flat.entry(prop.hash).or_default() += 1;
        }
        // bundle 启用 = 四级数组并集 − 排除表（PDB FillFromProps.c:261-275）
        let excluded: BTreeSet<u32> = keys_of(&flat, P_BUNDLE_EXCLUDE)
            .iter()
            .map(|k| k.instance)
            .collect();
        let mut enabled: BTreeSet<u32> = BTreeSet::new();
        for level in P_BUNDLE_ENABLE_LEVELS {
            enabled.extend(keys_of(&flat, level).iter().map(|k| k.instance));
        }
        let bundles: Vec<u32> = enabled.difference(&excluded).copied().collect();
        // 资源 bin：core（合并后单位表自身的三列表行）∪ 各启用 bundle 的行
        let mut bin_core: BTreeSet<u32> = keys_of(&flat, P_BIN_RESOURCES)
            .iter()
            .map(|k| k.instance)
            .collect();
        let mut all_bins = bin_core.clone();
        let mut utility_amounts: BTreeMap<u32, i64> = BTreeMap::new();
        let known_resources = [RES_POWER, RES_WATER, RES_GARBAGE];
        let mut absorb_bins = |list: &PropertyFile, all: &mut BTreeSet<u32>| {
            let res_ids = keys_of(list, P_BIN_RESOURCES);
            let amounts = int_array(list, 0x07E0_AD24);
            for (i, key) in res_ids.iter().enumerate() {
                all.insert(key.instance);
                if known_resources.contains(&key.instance) {
                    if let Some(amount) = amounts.get(i) {
                        utility_amounts.insert(key.instance, *amount);
                    }
                }
            }
        };
        absorb_bins(&flat, &mut all_bins);
        for bundle in &bundles {
            if let Some((_, bundle_file)) = db.get(&(GROUP_BUNDLE, *bundle)) {
                absorb_bins(bundle_file, &mut all_bins);
            }
        }
        let bin_bundle_only: BTreeSet<u32> = all_bins.difference(&bin_core).copied().collect();
        for res in &all_bins {
            let entry = resource_census.entry(*res).or_insert((0, 0, 0));
            entry.0 += 1;
            if bin_core.contains(res) {
                entry.1 += 1;
            } else {
                entry.2 += 1;
            }
        }
        let lot_instance = keys_of(&flat, P_LOT_LINK).first().map(|k| k.instance);
        let has_menu = menu_units.contains_key(instance);
        let class = classify_unit(file.values.len(), &bundles, &bin_bundle_only, has_menu);
        units.push(UnitRec {
            pkg: pkg_label(*idx),
            instance: *instance,
            parent: keys_of(file, PARENT_HASH).first().map(|k| k.instance),
            chain: chain.iter().map(|k| k.instance).collect(),
            bundles: bundles.clone(),
            bundles_excluded: excluded.into_iter().collect(),
            own_families: families_of(file),
            families: families_of(&flat),
            bin_core,
            bin_bundle_only,
            utility_amounts,
            has_menu,
            leaf_props: file.values.len(),
            class,
            lot: lot_instance.and_then(|i| lots.get(&i).cloned()),
        });
    }

    // —— 行为族 × 独立性聚合（默认只统计 primary；变体/空表单列）——
    let mut unit_class_hist: BTreeMap<&'static str, usize> = BTreeMap::new();
    for unit in &units {
        *unit_class_hist.entry(unit.class).or_default() += 1;
    }
    let mut family_stats: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    for (name, _) in FAMILIES {
        let matched: Vec<&UnitRec> = units
            .iter()
            .filter(|u| u.class == "primary" && u.families.contains(name))
            .collect();
        let own: usize = units
            .iter()
            .filter(|u| u.class == "primary" && u.own_families.contains(name))
            .count();
        let variant_hits = units
            .iter()
            .filter(|u| u.class != "primary" && u.families.contains(name))
            .count();
        let mut by_pkg: BTreeMap<String, usize> = BTreeMap::new();
        let mut bundle_hist: BTreeMap<usize, usize> = BTreeMap::new();
        let (mut with_menu, mut no_lot, mut lot_global_first, mut lot_local_none) = (0, 0, 0, 0);
        for unit in &matched {
            *by_pkg.entry(unit.pkg.clone()).or_default() += 1;
            *bundle_hist.entry(unit.bundles.len()).or_default() += 1;
            if unit.has_menu {
                with_menu += 1;
            }
            match &unit.lot {
                None => no_lot += 1,
                Some(lot) => {
                    if lot.global_first() {
                        lot_global_first += 1;
                    } else if lot.local_none_only() {
                        lot_local_none += 1;
                    }
                }
            }
        }
        family_stats.insert(
            name,
            json!({
                "total": matched.len(),
                "own_level": own,
                "inherited_only": matched.len() - own,
                "variant_or_empty_hits": variant_hits,
                "by_pkg": by_pkg,
                "bundle_count_hist": bundle_hist,
                "with_menu": with_menu,
                "without_menu": matched.len() - with_menu,
                "lot_global_first": lot_global_first,
                "lot_local_none_only": lot_local_none,
                "no_lot": no_lot,
            }),
        );
    }

    // —— 模板层普查（48E0C000）——
    let mut templates: Vec<serde_json::Value> = Vec::new();
    let mut template_freq: BTreeMap<u32, usize> = BTreeMap::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_TEMPLATE {
            continue;
        }
        for prop in &file.values {
            *template_freq.entry(prop.hash).or_default() += 1;
        }
        templates.push(json!({
            "pkg": pkg_label(*idx),
            "instance": format!("{instance:08X}"),
            "parent": keys_of(file, PARENT_HASH).first().map(|k| format!("{:08X}", k.instance)),
            "families": families_of(file).into_iter().collect::<Vec<_>>(),
            "bundles": P_BUNDLE_ENABLE_LEVELS
                .iter()
                .flat_map(|level| keys_of(file, *level))
                .map(|k| format!("{:08X}", k.instance))
                .collect::<Vec<_>>(),
            "referenced_by_units": units.iter().filter(|u| u.chain.contains(instance)).count(),
        }));
    }

    // —— bundle 普查（40E0C700：规则容器 + 资源 bin + sink 定义）——
    // 实测（小学全家）：0x07E0AD23 = bin 资源 ID 表（Key[]），0x07E0AD24 =
    // bin 数量（int32[]，与 0x07E0AD23 平行），0x08080D58.. = 规则引用，
    // 0x08327D48(+0..20) = 事件规则槽，0x08635E12 = int32[] 标志族，
    // 0x0A2725CE/0x0A2725CF = sink 条目引用表（GB/cEcoSinkEntry::FillFromProps）。
    const P_BUNDLE_BIN_RESOURCES: u32 = 0x07E0_AD23;
    const P_BUNDLE_BIN_AMOUNTS: u32 = 0x07E0_AD24;
    const P_SINK_LIST: u32 = 0x0A27_25CE;
    const P_SINK_LIST2: u32 = 0x0A27_25CF;
    // sink 条目本体（cEcoSinkEntry::FillFromProps 读取列）
    const P_SINK_RESOURCE: u32 = 0x0A81_0F28;
    const P_SINK_CONNECTED: u32 = 0x0DA7_921B;
    const P_SINK_INVERT: u32 = 0x0B55_5629;
    const P_SINK_MODIFIER: u32 = 0x0E7A_94FF;
    let mut bundles: Vec<serde_json::Value> = Vec::new();
    let mut bundle_shape_hist: BTreeMap<String, usize> = BTreeMap::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_BUNDLE {
            continue;
        }
        let mut rule_refs: BTreeSet<u32> = BTreeSet::new();
        for hash in BUNDLE_RULE_REFS {
            rule_refs.extend(keys_of(file, hash).iter().map(|k| k.instance));
        }
        let mut event_slots = 0usize;
        for i in 0..BUNDLE_EVENT_SLOT_COUNT {
            if count_of(file, BUNDLE_EVENT_SLOT_BASE + i) > 0 {
                event_slots += 1;
            }
        }
        let bins: Vec<u32> = keys_of(file, P_BUNDLE_BIN_RESOURCES)
            .iter()
            .map(|k| k.instance)
            .collect();
        let amounts: Vec<i64> = match file.get(P_BUNDLE_BIN_AMOUNTS).map(|p| &p.kind) {
            Some(Kind::Array(values)) => values
                .iter()
                .filter_map(|v| match v {
                    Value::Int32(n) => Some(i64::from(*n)),
                    Value::UInt32(n) => Some(i64::from(*n)),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        let shape = match (bins.is_empty(), rule_refs.is_empty(), event_slots == 0) {
            (false, true, true) => "bins-only",
            (true, false, true) => "rules-only",
            (false, false, _) => "bins+rules",
            (true, true, true) => "empty/marker",
            (true, _, false) => "events-only",
            (false, true, false) => "bins+events",
        };
        *bundle_shape_hist.entry(shape.to_string()).or_default() += 1;
        bundles.push(json!({
            "pkg": pkg_label(*idx),
            "instance": format!("{instance:08X}"),
            "shape": shape,
            "bins": hex_vec(&bins),
            "bin_amounts": amounts,
            "sinks": hex_vec(&keys_of(file, P_SINK_LIST).iter().map(|k| k.instance).collect::<Vec<_>>()),
            "sinks2": hex_vec(&keys_of(file, P_SINK_LIST2).iter().map(|k| k.instance).collect::<Vec<_>>()),
            "rule_refs": hex_vec(&rule_refs.into_iter().collect::<Vec<_>>()),
            "event_rule_slots": event_slots,
            "prop_count": file.values.len(),
            "referenced_by_units": units.iter().filter(|u| u.bundles.contains(instance)).count(),
        }));
    }

    // —— sink 条目本体普查（0x0A810F28 存在的属性表，任意允许组）——
    let mut sink_entries: Vec<serde_json::Value> = Vec::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if file.get(P_SINK_RESOURCE).is_none() {
            continue;
        }
        sink_entries.push(json!({
            "pkg": pkg_label(*idx),
            "group": format!("{group:08X}"),
            "instance": format!("{instance:08X}"),
            "resource": keys_of(file, P_SINK_RESOURCE).first().map(|k| format!("{:08X}", k.instance)),
            "connected": file.get(P_SINK_CONNECTED).map(|_| true).unwrap_or(false),
            "invert": file.get(P_SINK_INVERT).map(|_| true).unwrap_or(false),
            "modifier": u32_of(file, P_SINK_MODIFIER),
            "prop_count": file.values.len(),
        }));
    }

    // —— 主表普查（40800200）——
    let mut master_tables: Vec<serde_json::Value> = Vec::new();
    for ((group, instance), (idx, file)) in db.iter() {
        if *group != GROUP_MASTER {
            continue;
        }
        let mut decls = serde_json::Map::new();
        for (hash, name) in MASTER_DECLS {
            let groups: Vec<String> = keys_of(file, *hash)
                .iter()
                .map(|k| format!("{:08X}", k.instance))
                .collect();
            if !groups.is_empty() {
                decls.insert((*name).to_string(), json!(groups));
            }
        }
        master_tables.push(json!({
            "pkg": pkg_label(*idx),
            "instance": format!("{instance:08X}"),
            "version": u32_of(file, P_MASTER_VERSION),
            "decls": decls,
            "prop_count": file.values.len(),
        }));
    }

    let mut board_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut road_pair_hist: BTreeMap<String, usize> = BTreeMap::new();
    let scope_name = |v: u32| {
        if v == V_SCOPE_GLOBAL {
            "Global"
        } else if v == V_SCOPE_LOCAL {
            "Local"
        } else {
            "?"
        }
    };
    let road_name = |v: u32| match v {
        V_ROAD_FIRST => "First",
        V_ROAD_NONE => "None",
        V_ROAD_LAST => "Last",
        V_ROAD_FIRST_AND_LAST => "FirstAndLast",
        _ => "?",
    };
    for lot in lots.values() {
        if let Some(b) = lot.board {
            *board_hist.entry(format!("{b:08X}")).or_default() += 1;
        }
        for (s, c) in lot.scopes.iter().zip(lot.road_connect.iter()) {
            *road_pair_hist
                .entry(format!("{}+{}", scope_name(*s), road_name(*c)))
                .or_default() += 1;
        }
    }

    // —— 资源 bin 统计：named 属性 vs 三列表行 的功能等价性 ——
    let mut resource_bin_stats = serde_json::Map::new();
    for (name, res, fam) in [
        ("power", RES_POWER, "power_consumer"),
        ("water", RES_WATER, "water_consumer"),
        ("garbage", RES_GARBAGE, "garbage_producer"),
    ] {
        let core = units.iter().filter(|u| u.bin_core.contains(&res)).count();
        let bundle_only = units
            .iter()
            .filter(|u| u.bin_bundle_only.contains(&res))
            .count();
        let prop_only = units
            .iter()
            .filter(|u| {
                u.families.contains(fam)
                    && !u.bin_core.contains(&res)
                    && !u.bin_bundle_only.contains(&res)
            })
            .count();
        let bin_no_prop = units
            .iter()
            .filter(|u| {
                (u.bin_core.contains(&res) || u.bin_bundle_only.contains(&res))
                    && !u.families.contains(fam)
            })
            .count();
        resource_bin_stats.insert(
            name.to_string(),
            json!({
                "bin_core": core,
                "bin_bundle_only": bundle_only,
                "total_with_bin": core + bundle_only,
                "named_prop_only_no_bin": prop_only,
                "bin_without_named_prop": bin_no_prop,
            }),
        );
    }

    // —— 单元明细（普查可追溯性）——
    let unit_details: Vec<serde_json::Value> = units
        .iter()
        .map(|u| {
            json!({
                "pkg": u.pkg,
                "instance": format!("{:08X}", u.instance),
                "parent": u.parent.map(|p| format!("{p:08X}")),
                "chain": hex_vec(&u.chain),
                "bundles": hex_vec(&u.bundles),
                "bundles_excluded": hex_vec(&u.bundles_excluded),
                "own_families": u.own_families.iter().collect::<Vec<_>>(),
                "families": u.families.iter().collect::<Vec<_>>(),
                "bin_core": hex_vec(&u.bin_core.iter().copied().collect::<Vec<_>>()),
                "bin_bundle_only": hex_vec(&u.bin_bundle_only.iter().copied().collect::<Vec<_>>()),
                "utility_amounts": u
                    .utility_amounts
                    .iter()
                    .map(|(r, a)| (format!("{r:08X}"), *a))
                    .collect::<BTreeMap<String, i64>>(),
                "has_menu": u.has_menu,
                "leaf_props": u.leaf_props,
                "class": u.class,
                "lot": u.lot.as_ref().map(|l| l.to_json()),
            })
        })
        .collect();

    let sorted_freq = |freq: &BTreeMap<u32, usize>| -> Vec<serde_json::Value> {
        let mut rows: Vec<_> = freq.iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        rows.iter()
            .map(|(h, n)| json!({"hash": format!("{h:08X}"), "count": n}))
            .collect()
    };

    let census = json!({
        "inputs": pkgs.iter().map(|(l, p)| json!({"label": l, "path": p.display().to_string()})).collect::<Vec<_>>(),
        "load_stats": load_stats,
        "group_census": group_census,
        "unit_total": units.len(),
        "unit_class_hist": unit_class_hist,
        "unit_parent_chain_breaks": chain_breaks,
        "empty_table_census": empty_by_group
            .iter()
            .map(|((pkg, group), (empty, total, tiny))| {
                json!({"pkg": pkg, "group": format!("{group:08X}"), "empty": empty, "tiny_1_3": tiny, "total": total})
            })
            .collect::<Vec<_>>(),
        "family_stats": family_stats,
        "menu": {
            "total": menu_total,
            "with_module_mount": menu_with_module_mount,
            "parent_hist": menu_parent_hist,
            "uicategories_hist": menu_uicat_hist,
            "kcategory_hist": menu_kcategory_hist,
            "title_table_hist": menu_title_table_hist,
            "menu_unit_targets": menu_units.len(),
        },
        "lot_census": {
            "total": lots.len(),
            "global_first": lots.values().filter(|l| l.global_first()).count(),
            "local_none_only": lots.values().filter(|l| l.local_none_only()).count(),
            "board_hist": board_hist,
            "road_scope_pair_hist": road_pair_hist,
        },
        "resource_bin_stats": resource_bin_stats,
        "resource_census": {
            "distinct_resources": resource_census.len(),
            "rows": resource_census
                .iter()
                .map(|(res, (n, core, bundle))| {
                    json!({"resource": format!("{res:08X}"), "units": n, "core": core, "bundle_only": bundle})
                })
                .collect::<Vec<_>>(),
        },
        "master_tables": master_tables,
        "bundle_shape_hist": bundle_shape_hist,
        "sink_entries": sink_entries,
        "bundles": bundles,
        "templates": templates,
        "units": unit_details,
        "unit_prop_freq_own": sorted_freq(&unit_freq_own),
        "unit_prop_freq_flat": sorted_freq(&unit_freq_flat),
        "template_prop_freq": sorted_freq(&template_freq),
    });

    std::fs::create_dir_all(&out_dir).expect("create out dir");
    let out_path = out_dir.join("census.json");
    std::fs::write(&out_path, serde_json::to_vec_pretty(&census).expect("json"))
        .unwrap_or_else(|e| panic!("write {out_path:?}: {e}"));

    // —— stdout 摘要 ——
    println!(
        "== 行为普查：{} 个 Unit（primary {} / variant {} / empty {}；{} 条 lot，{} 条 Menu2，链断 {}）==",
        units.len(),
        unit_class_hist.get("primary").copied().unwrap_or(0),
        unit_class_hist.get("variant").copied().unwrap_or(0),
        unit_class_hist.get("empty").copied().unwrap_or(0),
        lots.len(),
        menu_total,
        chain_breaks
    );
    println!(
        "{:<18} {:>6} {:>6} {:>8} {:>7} {:>7} {:>9} {:>6}",
        "family", "total", "own", "inherit", "bund0", "menu-", "lotGF", "noLot"
    );
    for (name, _) in FAMILIES {
        let s = &family_stats[name];
        let bh0 = s["bundle_count_hist"]
            .get("0")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!(
            "{:<18} {:>6} {:>6} {:>8} {:>7} {:>7} {:>9} {:>6}",
            name,
            s["total"].as_u64().unwrap_or(0),
            s["own_level"].as_u64().unwrap_or(0),
            s["inherited_only"].as_u64().unwrap_or(0),
            bh0,
            s["without_menu"].as_u64().unwrap_or(0),
            s["lot_global_first"].as_u64().unwrap_or(0),
            s["no_lot"].as_u64().unwrap_or(0),
        );
    }
    let empty_total: usize = empty_by_group.values().map(|(e, _, _)| *e).sum();
    let resident_total: usize = empty_by_group.values().map(|(_, t, _)| *t).sum();
    let tiny_total: usize = empty_by_group.values().map(|(_, _, t)| *t).sum();
    println!(
        "empty tables: {empty_total}/{resident_total}（4 字节空表），tiny(1-3 props): {tiny_total}"
    );
    println!(
        "master tables: {}",
        serde_json::to_string(&master_tables).unwrap_or_default()
    );
    println!(
        "resource bins: {}",
        serde_json::to_string(&resource_bin_stats).unwrap_or_default()
    );
    println!("lot (scope,connect): {:?}", road_pair_hist);
    println!("bundle shapes: {:?}", bundle_shape_hist);
    println!(
        "sink entries: {}",
        serde_json::to_string(&sink_entries).unwrap_or_default()
    );
    println!("census -> {}", out_path.display());
}
