//! 树 prop 深挖（2026-11-04 任务第二轮）：
//! 1. Graphics 包 R_/C_/I_ property 记录引用的 prop resourceID → 逐条按
//!    prop_models resolve_one 语义跟链（lod1_direct / vehicle_models /
//!    explicit→wrapper / tree 签名 / descriptor），判定可解析性与树类
//! 2. 树签名包装记录（0x0C36D30D + 0x0D8C29CF）全量普查（两套安装对照）
//! 3. 显式 key（0x0D8C29C3）反查：哪些 resourceID 指向每个树签名包装
//! 4. descriptor（G 40002D00 + 0x0E0B99FD）与 Parent 配置表 5F804D7E
//!    （0BD62577 列 198583672-681）全量 dump
//!
//! 运行：cargo run --release -p sc-properties --example tree_hunt

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dbpf::{IndexEntry, Package};
use sc_properties::lot_unit as u;
use sc_properties::{Kind, ParseLimits, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
const KEY_TREE_STATE: u32 = 0x0C36_D30D;
const KEY_LOD_INDEX: u32 = 0x0D8C_29CF;
const KEY_TREE_VARIANTS: u32 = 0x0E0B_99FD;
const KEY_MODEL_EXPLICIT: u32 = 0x0D8C_29C3;
const KEY_VEHICLE_MODELS: u32 = 0x0D89_7169;
const KEY_LOD1: u32 = 0x00F9_EFBB;
const KEY_PARENT: u32 = 0x00B2_CCCB;
const CONFIG_TABLE_PROP: u32 = 0x0BD6_2577;
const CONFIG_COL_BASE: u32 = 0x0BD6_2578;
const CONFIG_COL_COUNT: u32 = 10;
const CONFIG_TABLE_PARENT: u32 = 0x5F80_4D7E;
/// 资源记录典型 group 低 16 位（EcoGame / 安装包互补，prop_models 语义）。
const RESOURCE_GROUP_LOW: [u16; 2] = [0xC100, 0xC600];
const PROP_ID_BASE: u32 = u::PROP_ID_BASE;
const PROP_BINS: usize = u::PROP_BINS;

const LIVE_ROOT: &str = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow";
const OLD_ROOT: &str = r"D:/ea-games/SimCity";

fn walk_packages(root: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_packages(&path.to_string_lossy()));
        } else if path.extension().and_then(|e| e.to_str()) == Some("package") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let skip = name.contains("Audio")
                || name == "Cache.package"
                || name == "GraphicsCache.package"
                || name.contains("Terrain")
                || name == "UpdaterData.package"
                || name == "SimCity_Updater.package"
                || name.ends_with(".package.disabled");
            if !skip {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// EcoGame 家族_版本号 → 最新版（对齐 prop_models）。
fn ecogame_latest(root: &str) -> Vec<PathBuf> {
    let dir = Path::new(root).join("SimCityUserData").join("EcoGame");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut latest: BTreeMap<String, (u64, PathBuf)> = BTreeMap::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("package") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some((family, version)) = stem.rsplit_once('_') else {
            continue;
        };
        let version = version.parse::<u64>().unwrap_or(0);
        let slot = latest
            .entry(family.to_string())
            .or_insert_with(|| (version, path.clone()));
        if version > slot.0 {
            slot.0 = version;
            slot.1 = path.clone();
        }
    }
    latest.into_values().map(|(_, p)| p).collect()
}

fn parse_record(pkg: &Package, entry: &IndexEntry) -> Option<PropertyFile> {
    let data = pkg.read(entry).ok()?;
    PropertyFile::parse_with_limits(&data, ParseLimits::default()).ok()
}

fn values_of<'a>(file: &'a PropertyFile, hash: u32) -> Vec<&'a Value> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(vs) => vs.iter().collect(),
            Kind::Empty => vec![],
        })
        .collect()
}

fn has_prop(file: &PropertyFile, hash: u32) -> bool {
    !values_of(file, hash).is_empty()
}

fn key_ids(file: &PropertyFile, hash: u32) -> Vec<u32> {
    values_of(file, hash)
        .iter()
        .filter_map(|v| match v {
            Value::Key(k) => Some(k.instance),
            Value::UInt32(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn u32_values(file: &PropertyFile, hash: u32) -> Vec<u32> {
    values_of(file, hash)
        .iter()
        .filter_map(|v| match v {
            Value::UInt32(n) => Some(*n),
            _ => None,
        })
        .collect()
}

/// PROP_ID bin 引用（14 bin，Key/UInt32 兼收）。
fn prop_ids(file: &PropertyFile) -> Vec<u32> {
    let mut ids = Vec::new();
    for bin in 0..PROP_BINS as u32 {
        ids.extend(key_ids(file, PROP_ID_BASE + bin));
    }
    ids
}

struct Record {
    label: String,
    group: u32,
    instance: u32,
    file: PropertyFile,
}

/// 跨包找记录（resource-group 形态优先，对齐 prop_models find_records 意图）。
fn find_records<'a>(records: &'a [Record], instance: u32) -> Vec<&'a Record> {
    let mut hits: Vec<&Record> = records
        .iter()
        .filter(|r| r.instance == instance)
        .collect();
    hits.sort_by_key(|r| (
        !RESOURCE_GROUP_LOW.contains(&(r.group as u16)),
        r.group,
    ));
    hits
}

/// prop_models resolve_one 语义的跟链复刻（返回 source 标签 + 模型/树判定）。
fn resolve<'a>(
    records: &'a [Record],
    resource_id: u32,
    depth: usize,
) -> Option<(String, Vec<u32>, bool)> {
    assert!(depth < 4, "resolve chain too deep");
    let recs = find_records(records, resource_id);
    let first = recs.first()?;
    let file = &first.file;
    // 分支 0：LOD1 直引
    let lod1 = key_ids(file, KEY_LOD1);
    if !lod1.is_empty() {
        return Some(("lod1_direct".into(), lod1, false));
    }
    // 分支 2：自带 Vehicle Models
    let vehicle = key_ids(file, KEY_VEHICLE_MODELS);
    if !vehicle.is_empty() {
        return Some(("vehicle_models".into(), vehicle, false));
    }
    // 分支 1：显式包装 key
    if let Some(wrapper) = key_ids(file, KEY_MODEL_EXPLICIT).first().copied() {
        let wrecs = find_records(records, wrapper);
        if let Some(w) = wrecs.first() {
            let wf = &w.file;
            let wlod1 = key_ids(wf, KEY_LOD1);
            if !wlod1.is_empty() {
                return Some(("lod1_wrapper".into(), wlod1, false));
            }
            let wvehicle = key_ids(wf, KEY_VEHICLE_MODELS);
            if !wvehicle.is_empty() {
                return Some(("explicit→vehicle_models".into(), wvehicle, false));
            }
            // 树签名：descriptor + LOD 序号共存
            if has_prop(wf, KEY_TREE_STATE) && has_prop(wf, KEY_LOD_INDEX) {
                return Some((
                    "tree_model".into(),
                    Vec::new(),
                    true,
                ));
            }
            // descriptor 变体
            if let Some(desc) = key_ids(wf, KEY_TREE_STATE).first().copied() {
                for d in find_records(records, desc) {
                    let variants = key_ids(&d.file, KEY_TREE_VARIANTS);
                    if !variants.is_empty() {
                        return Some(("descriptor".into(), variants, true));
                    }
                }
            }
            // 包装记录再嵌套一层（罕见，保底递归）
            if let Some(next) = key_ids(wf, KEY_MODEL_EXPLICIT).first().copied() {
                return resolve(records, next, depth + 1);
            }
        }
        return Some(("explicit(unresolved wrapper)".into(), vec![wrapper], false));
    }
    // self-key 标记：Parent 链 Vehicle Models
    let marker = [KEY_LOD1, KEY_VEHICLE_MODELS, KEY_TREE_STATE]
        .iter()
        .any(|h| has_prop(file, *h));
    if marker {
        if let Some(parent) = key_ids(file, KEY_PARENT).first().copied() {
            for p in find_records(records, parent) {
                let models = key_ids(&p.file, KEY_VEHICLE_MODELS);
                if !models.is_empty() {
                    return Some(("selfkey_parent".into(), models, false));
                }
            }
        }
        return Some(("selfkey(no models)".into(), Vec::new(), false));
    }
    None
}

fn main() {
    let registry = sc_registry::Registry::open("src-tauri/resources/database_main.s3db")
        .expect("open registry; run from repo root");
    let name_of = |instance: u32| -> String {
        registry
            .instances()
            .get(&instance)
            .map(|r| r.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("0x{instance:08X}"))
    };
    let prop_name = |hash: u32| -> String {
        registry
            .properties()
            .get(&hash)
            .map(|r| r.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("0x{hash:08X}"))
    };
    let _ = &prop_name;

    // ---- 解析用包集：主包 + EcoGame 最新版（live 安装） ----
    let mut resolve_pkgs: Vec<Record> = Vec::new();
    for rel in [
        "SimCityData/SimCity_Graphics.package",
        "SimCityData/SimCity_App.package",
        "SimCityData/SimCity_Game.package",
        "SimCityData/SimCityDataEP1.package",
        "SimCityData/SimCity_DLC0.package",
    ] {
        let path = Path::new(LIVE_ROOT).join(rel);
        let Ok(pkg) = Package::open(&path) else {
            eprintln!("SKIP {rel}");
            continue;
        };
        println!("resolve-set: {rel} {} entries", pkg.entries().len());
        for entry in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if let Some(file) = parse_record(&pkg, entry) {
                resolve_pkgs.push(Record {
                    label: rel.to_string(),
                    group: entry.id.group,
                    instance: entry.id.instance,
                    file,
                });
            }
        }
    }
    for path in ecogame_latest(LIVE_ROOT) {
        let rel = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let Ok(pkg) = Package::open(&path) else {
            continue;
        };
        let mut n = 0usize;
        for entry in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if let Some(file) = parse_record(&pkg, entry) {
                resolve_pkgs.push(Record {
                    label: format!("EcoGame/{rel}"),
                    group: entry.id.group,
                    instance: entry.id.instance,
                    file,
                });
                n += 1;
            }
        }
        println!("resolve-set: EcoGame/{rel} {n} 条 B1B104");
    }

    // ---- 1) Graphics R/C/I 记录引用 + 跟链 ----
    println!("\n== 1) Graphics R_/C_/I_ property 引用的 resourceID 跟链 ==");
    let gfx = Package::open(Path::new(LIVE_ROOT).join("SimCityData/SimCity_Graphics.package"))
        .expect("graphics");
    let mut referenced: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
    let prefixes = ["R_", "C_", "I_"];
    let mut rcni_count = [0usize; 3];
    let mut rcni_with_props = 0usize;
    for entry in gfx.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
        let Some(rec) = registry.instances().get(&entry.id.instance) else {
            continue;
        };
        let name = rec.name.trim();
        let Some(p) = prefixes.iter().position(|p| name.starts_with(p)) else {
            continue;
        };
        rcni_count[p] += 1;
        let Some(file) = parse_record(&gfx, entry) else {
            continue;
        };
        let ids = prop_ids(&file);
        if ids.is_empty() {
            continue;
        }
        rcni_with_props += 1;
        for id in ids {
            referenced
                .entry(id)
                .or_default()
                .insert(name[..2].to_string());
        }
    }
    println!(
        "  R/C/I 记录 {}/{}：R={} C={} I={}，带 prop 引用的 {} 条，去重 resourceID {} 个\n",
        rcni_with_props,
        rcni_count.iter().sum::<usize>(),
        rcni_count[0],
        rcni_count[1],
        rcni_count[2],
        rcni_with_props,
        referenced.len()
    );
    let mut tree_resources: Vec<u32> = Vec::new();
    for (id, zones) in &referenced {
        let recs = find_records(&resolve_pkgs, *id);
        let loc = recs
            .first()
            .map(|r| format!("{} G {:08X}", r.label, r.group))
            .unwrap_or_else(|| "NO-RECORD".into());
        match resolve(&resolve_pkgs, *id, 0) {
            Some((source, models, is_tree)) => {
                if is_tree {
                    tree_resources.push(*id);
                }
                let models = if models.is_empty() {
                    String::new()
                } else {
                    format!(
                        " models={}",
                        models
                            .iter()
                            .map(|m| format!("{m:08X}[{}]", name_of(*m)))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                println!(
                    "  {id:08X} [{name}] @{loc} zones={:?} → {source}{models}",
                    zones,
                    name = name_of(*id),
                );
            }
            None => println!(
                "  {id:08X} [{name}] @{loc} zones={:?} → UNRESOLVED",
                zones,
                name = name_of(*id),
            ),
        }
    }

    // ---- 2) 树签名包装记录全量普查（两套安装，流式） ----
    println!("\n== 2) 树签名记录（0x0C36D30D+0x0D8C29CF）全量普查 ==");
    let mut tree_wrappers: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut explicit_refs: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut descriptors: BTreeMap<u32, BTreeMap<String, Vec<u32>>> = BTreeMap::new();
    let mut config_seen = false;
    for (install, root) in [("LIVE", LIVE_ROOT), ("OLD", OLD_ROOT)] {
        for path in walk_packages(root) {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            let Ok(pkg) = Package::open(&path) else {
                continue;
            };
            for entry in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
                let Some(file) = parse_record(&pkg, entry) else {
                    continue;
                };
                for w in key_ids(&file, KEY_MODEL_EXPLICIT) {
                    explicit_refs.entry(w).or_default().push(entry.id.instance);
                }
                let has_state = has_prop(&file, KEY_TREE_STATE);
                let has_lod = has_prop(&file, KEY_LOD_INDEX);
                if has_state && has_lod {
                    let entry_key = format!(
                        "{install} {rel} G {:08X} lod={:?} variants={:?} parent={:?}",
                        entry.id.group,
                        u32_values(&file, KEY_LOD_INDEX),
                        key_ids(&file, KEY_TREE_VARIANTS),
                        key_ids(&file, KEY_PARENT).first(),
                    );
                    tree_wrappers
                        .entry(entry.id.instance)
                        .or_default()
                        .push(entry_key);
                }
                if entry.id.group == 0x4000_2D00 && has_prop(&file, KEY_TREE_VARIANTS) {
                    descriptors
                        .entry(entry.id.instance)
                        .or_default()
                        .entry(format!("{install} {rel}"))
                        .or_insert_with(|| key_ids(&file, KEY_TREE_VARIANTS));
                }
                if entry.id.instance == CONFIG_TABLE_PARENT
                    && has_prop(&file, CONFIG_TABLE_PROP)
                    && !config_seen
                {
                    config_seen = true;
                    println!("  配置表 {install} {rel} G {:08X} [{}]:", entry.id.group, name_of(entry.id.instance));
                    for col in 0..CONFIG_COL_COUNT {
                        let ids = key_ids(&file, CONFIG_COL_BASE + col);
                        if ids.is_empty() {
                            continue;
                        }
                        let parts: Vec<String> = ids
                            .iter()
                            .map(|id| format!("{id:08X}[{}]", name_of(*id)))
                            .collect();
                        println!("    col{col}: {}", parts.join(", "));
                    }
                }
            }
        }
    }
    for (wrapper, locs) in &tree_wrappers {
        let users: Vec<u32> = explicit_refs
            .get(wrapper)
            .cloned()
            .unwrap_or_default();
        let mut uniq_users = users.clone();
        uniq_users.sort_unstable();
        uniq_users.dedup();
        let rci_users: Vec<&u32> = uniq_users.iter().filter(|u| referenced.contains_key(u)).collect();
        println!(
            "  wrapper {wrapper:08X} [{}] — 记录 {} 处；被显式引用 resourceID {} 个（R/C/I 图谱内 {}）",
            name_of(*wrapper),
            locs.len(),
            uniq_users.len(),
            rci_users.len()
        );
        for loc in locs {
            println!("    @{loc}");
        }
        for usr in uniq_users.iter().take(12) {
            println!("      ← {usr:08X} [{}]", name_of(*usr));
        }
    }

    // ---- 3) descriptor 汇总 ----
    println!("\n== 3) descriptor（G 40002D00 + 0x0E0B99FD） ==");
    for (desc, per_pkg) in &descriptors {
        println!("  descriptor {desc:08X} [{}]", name_of(*desc));
        for (loc, variants) in per_pkg {
            let mut uniq = variants.clone();
            uniq.sort_unstable();
            uniq.dedup();
            println!("    @{loc}: {} 项（去重 {}）", variants.len(), uniq.len());
            for v in &uniq {
                println!("      {v:08X} [{}]", name_of(*v));
            }
        }
    }

    // ---- 4) 结论行 ----
    println!("\n== 4) 结论 ==");
    println!(
        "  R/C/I 引用中判定为树 resourceID: {:?}",
        tree_resources
            .iter()
            .map(|id| format!("{id:08X}"))
            .collect::<Vec<_>>()
    );
    println!("  树签名 wrapper: {} 个", tree_wrappers.len());

    // ---- 5) 全量树 resourceID 普查（resolve-set 内全部唯一 instance） ----
    println!("\n== 5) 全量树 prop 普查（resolve-set 全部唯一 resourceID 跟链） ==");
    let all_ids: Vec<u32> = {
        let mut s: BTreeSet<u32> = resolve_pkgs.iter().map(|r| r.instance).collect();
        s.extend(referenced.keys().copied());
        s.into_iter().collect()
    };
    let mut census_trees: Vec<(u32, String, Vec<u32>, Vec<u32>)> = Vec::new();
    for id in &all_ids {
        let Some((source, models, is_tree)) = resolve(&resolve_pkgs, *id, 0) else {
            continue;
        };
        if !is_tree {
            continue;
        }
        // 找出它的显式 wrapper 与 lod 序号（树种 = lod 序号 → 图集象限）
        let wrapper = find_records(&resolve_pkgs, *id)
            .first()
            .and_then(|r| key_ids(&r.file, KEY_MODEL_EXPLICIT).first().copied());
        let lod_vals: Vec<u32> = wrapper
            .map(|w| {
                let recs = find_records(&resolve_pkgs, w);
                recs.first()
                    .map(|r| u32_values(&r.file, KEY_LOD_INDEX))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let group = wrapper
            .map(|w| {
                let recs = find_records(&resolve_pkgs, w);
                recs.first()
                    .map(|r| format!("G {:08X}", r.group))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        census_trees.push((
            *id,
            format!("{source} wrapper={wrapper:?} lod={lod_vals:?} {group}"),
            models.clone(),
            vec![*id],
        ));
    }
    for (id, detail, models, _) in &census_trees {
        println!(
            "  TREE {id} [{}] {} models={}",
            name_of(*id),
            detail,
            models
                .iter()
                .map(|m| format!("{m:08X}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    println!("  → 全量普查树 resourceID 共 {} 个", census_trees.len());

    // ---- 6) 注册表植物命名实例 → 实体定位（棕榈/仙人掌候查） ----
    println!("\n== 6) 注册表植物命名实例实体定位 ==");
    let plant_words = [
        "palm", "cactus", "cacti", "tree", "pine", "conifer", "fir", "spruce", "oak",
        "maple", "birch", "willow", "cypress", "hedge", "shrub", "bush", "bushes",
        "plant", "flower", "vegetation", "foliage", "coconut", "jungle",
    ];
    let named_plants: Vec<(u32, String)> = registry
        .instances()
        .iter()
        .filter(|(_, r)| {
            let n = r.name.to_lowercase();
            plant_words
                .iter()
                .any(|w| {
                    let hb = n.as_bytes();                    if let Some(pos) = n.find(w) {
                        let left_ok = pos == 0 || !hb[pos - 1].is_ascii_alphabetic();
                        let end = pos + w.len();
                        let right_ok =
                            end == hb.len() || !hb[end].is_ascii_alphabetic();
                        left_ok && right_ok
                    } else {
                        false
                    }
                })
        })
        .map(|(id, r)| (*id, r.name.trim().to_string()))
        .collect();
    println!("  注册表植物命名实例 {} 个", named_plants.len());
    let plant_pkgs: Vec<(&str, Package)> = [
        ("Graphics", "SimCityData/SimCity_Graphics.package"),
        ("EP1", "SimCityData/SimCityDataEP1.package"),
        ("Game", "SimCityData/SimCity_Game.package"),
    ]
    .iter()
    .filter_map(|(l, rel)| {
        Package::open(Path::new(LIVE_ROOT).join(rel))
            .ok()
            .map(|p| (*l, p))
    })
    .collect();
    for (id, name) in named_plants.iter().take(60) {
        let mut locs: Vec<String> = Vec::new();
        for (label, pkg) in &plant_pkgs {
            for e in pkg.entries().iter().filter(|e| e.id.instance == *id) {
                locs.push(format!("{label} T{:08X} {}B", e.id.type_id, e.decompressed_size));
            }
        }
        if !locs.is_empty() {
            println!("  {id:08X} [{name}] {}", locs.join("; "));
        }
    }
    println!("  （未列出者 = Graphics/EP1/Game 无实体，仅脚本侧名字）");

    // ---- 7) 公园/树木工具记录的 PROP_ID 引用对拍（R1 公园菜单素材侧） ----
    println!("\n== 7) 公园/树木命名 B1B104 记录的 prop 引用 ==");
    let park_ids: [u32; 9] = [
        0xC0EC_9BBC, // Plant Forest
        0x175A_F694, // short tree row
        0x175A_F695, // Tall tree row
        0x175A_F696, // Wavy tree-lined walkway
        0x175A_F697, // Tree-lined walkway
        0x1230_E4DA, // Tree-linedwalkway
        0x7072_4BF7, // flower plaza
        0x70DD_2F03, // Tree
        0x41C2_D443, // 树 wrapper 的第二引用者（全量普查发现）
    ];
    let game_pkg = plant_pkgs.iter().find(|(l, _)| *l == "Game");
    let Some((_, game)) = game_pkg else {
        return;
    };
    for pid in park_ids {
        let Some(entry) = game
            .entries()
            .iter()
            .find(|e| e.id.type_id == PROPERTY_TYPE && e.id.instance == pid)
        else {
            continue;
        };
        let Some(file) = parse_record(&game, entry) else {
            continue;
        };
        let ids = prop_ids(&file);
        if ids.is_empty() {
            println!("  {pid:08X} [{}]: 无 PROP_ID 引用", name_of(pid));
            continue;
        }
        let parts: Vec<String> = ids
            .iter()
            .map(|id| {
                let tree_tag: String = if census_trees.iter().any(|(t, ..)| t == id) {
                    "TREE".to_string()
                } else {
                    resolve(&resolve_pkgs, *id, 0)
                        .map(|(source, _, _)| source)
                        .unwrap_or_else(|| "UNRESOLVED".to_string())
                };
                format!("{id:08X}[{}]{}", name_of(*id), tree_tag)
            })
            .collect();
        println!("  {pid:08X} [{}]: {}", name_of(pid), parts.join(", "));
    }
}
