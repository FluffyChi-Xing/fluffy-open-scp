//! 树 prop 普查（2026-11-04 任务）：R/C/I 分区 property 引用的树 prop，
//! 全游戏树 descriptor / 树签名记录权威清单，OpenSCP 大学/商业包对拍。
//!
//! 运行（需本机游戏包 + EcoGame 目录 + 注册表资源）：
//!   cargo run --release -p sc-properties --example tree_census
//!
//! 输出五段：
//! 1. Graphics 包（+EP1/App/Game 对照）B1B104 记录中注册表名 R_/C_/I_ 前缀计数
//! 2. 这些 R/C/I 记录经 PROP_ID bin（0x0C12EF20+bin）引用的全部 resourceID
//! 3. 树签名记录（0x0C36D30D descriptor + 0x0D8C29CF LOD 序号，引擎
//!    cGraphicsInstancedImpostor 触发签名）全量普查
//! 4. 树种 descriptor（G 40002D00 + 0x0E0B99FD 变体数组）全量普查 → 有效树清单
//! 5. OpenSCP 大学/商业包 prop 引用逐条对拍（可解析性）

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dbpf::Package;
use sc_properties::lot_unit as u;
use sc_properties::{Kind, ParseLimits, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
/// 树种 descriptor key（cGraphicsResource 树分支）。
const KEY_TREE_STATE: u32 = 0x0C36_D30D;
/// 树 LOD 序号（0x0D8C29CF）——与 KEY_TREE_STATE 共存 = impostor 树签名。
const KEY_LOD_INDEX: u32 = 0x0D8C_29CF;
/// 树种 descriptor 的变体模型 key 数组。
const KEY_TREE_VARIANTS: u32 = 0x0E0B_99FD;
/// 显式模型包装 key。
const KEY_MODEL_EXPLICIT: u32 = 0x0D8C_29C3;
/// Vehicle Models（载具/杂件直出）。
const KEY_VEHICLE_MODELS: u32 = 0x0D89_7169;
/// LOD1 直引模型。
const KEY_LOD1: u32 = 0x00F9_EFBB;
/// 引擎共享的树 impostor 源 3D 模型（Graphics 包，全树种共用）。
const TREE_PART_MODELS: [u32; 4] = [0x4DF4_3690, 0xC2FB_D178, 0x1113_B131, 0x89D6_58DF];
/// 树公告板图集。
const TREE_ATLAS: u32 = 0x835D_64F3;
/// prop resourceID 引用（14 bin）。
const PROP_ID_BASE: u32 = u::PROP_ID_BASE;
const PROP_BINS: usize = u::PROP_BINS;

const GAME_DATA: &str = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData";
const ECOGAME: &str = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityUserData/EcoGame";
const REGISTRY: &str = "src-tauri/resources/database_main.s3db";

type Packages = Vec<(String, Package)>;

fn open(label: &str, path: impl AsRef<Path>, out: &mut Packages) -> bool {
    let path = path.as_ref();
    match Package::open(path) {
        Ok(pkg) => {
            println!(
                "opened {label}: {} entries ({})",
                pkg.entries().len(),
                path.display()
            );
            out.push((label.to_string(), pkg));
            true
        }
        Err(e) => {
            eprintln!("SKIP {label}: {e}");
            false
        }
    }
}

/// EcoGame 目录按「家族_版本号」取最新版（对齐 src-tauri prop_models 语义）。
fn ecogame_latest(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
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
    latest.into_values().map(|(_, path)| path).collect()
}

fn parse_record(pkg: &Package, entry: &dbpf::IndexEntry) -> Option<PropertyFile> {
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

/// PROP_ID bin 引用的 resourceID（Key/UInt32 兼收）。
fn prop_ids(file: &PropertyFile) -> Vec<u32> {
    let mut ids = Vec::new();
    for bin in 0..PROP_BINS as u32 {
        for v in values_of(file, PROP_ID_BASE + bin) {
            match v {
                Value::Key(k) => ids.push(k.instance),
                Value::UInt32(n) => ids.push(*n),
                _ => {}
            }
        }
    }
    ids
}

struct TreeRecord {
    instance: u32,
    group: u32,
    package: String,
    descriptor: Option<u32>,
    lod_index: Option<u32>,
    has_model_explicit: bool,
    has_vehicle_models: bool,
}

fn main() {
    let registry = sc_registry::Registry::open(REGISTRY)
        .expect("open registry; run from repo root");
    let name_of = |instance: u32| -> String {
        registry
            .instances()
            .get(&instance)
            .map(|r| r.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("0x{instance:08X}"))
    };

    let mut packages: Packages = Vec::new();
    let data = Path::new(GAME_DATA);
    open("Graphics", data.join("SimCity_Graphics.package"), &mut packages);
    open("App", data.join("SimCity_App.package"), &mut packages);
    open("Game", data.join("SimCity_Game.package"), &mut packages);
    open("EP1", data.join("SimCityDataEP1.package"), &mut packages);
    open(
        "university_s9",
        data.join("000_OpenSCP_University_s9.package"),
        &mut packages,
    );
    open(
        "commercial_v7",
        data.join("000_OpenSCP_Commercial_v7.package"),
        &mut packages,
    );
    for path in ecogame_latest(Path::new(ECOGAME)) {
        let label = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ecogame")
            .to_string();
        open(&label, &path, &mut packages);
    }

    // ---- 1) R/C/I 前缀命名统计（按包） ----
    println!("\n== 1) R_/C_/I_ 前缀注册表名的 B1B104 记录（按包） ==");
    let prefixes = ["R_", "C_", "I_"];
    let mut rcni_records: BTreeMap<(String, char), Vec<u32>> = BTreeMap::new();
    for (label, pkg) in &packages {
        let mut counts: BTreeMap<char, usize> = BTreeMap::new();
        let mut named = 0usize;
        let mut total = 0usize;
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            total += 1;
            let Some(name) = registry.instances().get(&entry.id.instance) else {
                continue;
            };
            let name = name.name.trim();
            if name.is_empty() {
                continue;
            }
            named += 1;
            for p in prefixes {
                if name.starts_with(p) {
                    let c = p.as_bytes()[0] as char;
                    *counts.entry(c).or_default() += 1;
                    rcni_records
                        .entry((label.clone(), c))
                        .or_default()
                        .push(entry.id.instance);
                }
            }
        }
        if !counts.is_empty() {
            println!(
                "  {label}: B1B104 total={total} named={named} {}",
                counts
                    .iter()
                    .map(|(c, n)| format!("{c_}:{n}", c_ = c))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }

    // ---- 2) R/C/I 记录引用的 prop resourceID ----
    println!("\n== 2) R/C/I 记录 PROP_ID bin 引用的 resourceID ==");
    let mut referenced: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
    for ((label, c), instances) in &rcni_records {
        let Some((_, pkg)) = packages.iter().find(|(l, _)| l == label) else {
            continue;
        };
        for instance in instances {
            let Some(entry) = pkg
                .entries()
                .iter()
                .find(|e| e.id.type_id == PROPERTY_TYPE && e.id.instance == *instance)
            else {
                continue;
            };
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            for id in prop_ids(&file) {
                referenced.entry(id).or_default().insert(format!("{label}/{c}"));
            }
        }
    }
    println!(
        "  去重后 {} 个被引用 resourceID（引用来源数 {}）",
        referenced.len(),
        referenced.values().map(|s| s.len()).sum::<usize>()
    );

    // ---- 3) 树签名记录普查（全部包） ----
    println!("\n== 3) 树签名记录（0x0C36D30D + 0x0D8C29CF，impostor 触发签名） ==");
    let mut tree_records: BTreeMap<u32, Vec<TreeRecord>> = BTreeMap::new();
    for (label, pkg) in &packages {
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            let has_state = has_prop(&file, KEY_TREE_STATE);
            let has_lod = has_prop(&file, KEY_LOD_INDEX);
            if !has_state {
                continue;
            }
            let descriptor = values_of(&file, KEY_TREE_STATE).iter().find_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                Value::UInt32(n) => Some(*n),
                _ => None,
            });
            tree_records.entry(entry.id.instance).or_default().push(TreeRecord {
                instance: entry.id.instance,
                group: entry.id.group,
                package: label.clone(),
                descriptor,
                lod_index: has_lod.then_some(1u32),
                has_model_explicit: has_prop(&file, KEY_MODEL_EXPLICIT),
                has_vehicle_models: has_prop(&file, KEY_VEHICLE_MODELS),
            });
        }
    }
    let signature_trees: BTreeSet<u32> = tree_records
        .iter()
        .filter(|(_, v)| v.iter().any(|r| r.lod_index.is_some()))
        .map(|(k, _)| *k)
        .collect();
    println!(
        "  含 0x0C36D30D 的 resourceID {} 个；其中带 0x0D8C29CF（完整签名）{} 个",
        tree_records.len(),
        signature_trees.len()
    );

    // ---- 4) 树种 descriptor 普查（G 40002D00 + 0x0E0B99FD） ----
    println!("\n== 4) 树种 descriptor（G 40002D00 + 0x0E0B99FD 变体数组） ==");
    let mut descriptors: BTreeMap<u32, Vec<(String, Vec<u32>)>> = BTreeMap::new();
    for (label, pkg) in &packages {
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_TYPE && e.id.group == 0x4000_2D00
        }) {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            let variants: Vec<u32> = values_of(&file, KEY_TREE_VARIANTS)
                .iter()
                .filter_map(|v| match v {
                    Value::Key(k) => Some(k.instance),
                    Value::UInt32(n) => Some(*n),
                    _ => None,
                })
                .collect();
            if variants.is_empty() {
                continue;
            }
            descriptors
                .entry(entry.id.instance)
                .or_default()
                .push((label.clone(), variants));
        }
    }
    println!("  descriptor {} 个", descriptors.len());

    // 变体模型实体检查（RW4 存在性 + 大小 + 所在包）
    let model_loc = |instance: u32| -> Vec<(String, u32)> {
        let mut hits = Vec::new();
        for (label, pkg) in &packages {
            for e in pkg.entries().iter().filter(|e| {
                e.id.type_id == RW4_MODEL_TYPE && e.id.instance == instance
            }) {
                hits.push((label.clone(), e.decompressed_size));
            }
        }
        hits
    };

    println!("\n== 5) 有效树清单（descriptor → 变体模型 RW4 实体核实） ==");
    let mut effective: Vec<(u32, String, Vec<(String, Vec<u32>)>)> = Vec::new();
    for (desc, hits) in &descriptors {
        effective.push((*desc, name_of(*desc), hits.clone()));
    }
    effective.sort_by_key(|(_, name, _)| name.clone());
    for (desc, name, hits) in &effective {
        let mut line = format!("  descriptor {desc:08X} [{name}]");
        for (label, variants) in hits {
            let mut vs = String::new();
            for v in variants {
                let loc = model_loc(*v);
                let tag = if loc.is_empty() {
                    "MISSING".to_string()
                } else {
                    loc.iter()
                        .map(|(l, sz)| format!("{l}:{sz}B"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                vs.push_str(&format!("\n      variant {v:08X} [{}] {tag}", name_of(*v)));
            }
            line.push_str(&format!("\n    @{label}{vs}"));
        }
        println!("{line}");
    }

    println!("\n== 6) 共享 impostor 源模型 + 图集（Graphics 包实体核实） ==");
    for inst in TREE_PART_MODELS {
        let loc = model_loc(inst);
        println!(
            "  model {inst:08X} [{}] {}",
            name_of(inst),
            loc.iter()
                .map(|(l, sz)| format!("{l}:{sz}B"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    println!(
        "  atlas  {TREE_ATLAS:08X} [{}] {}",
        name_of(TREE_ATLAS),
        model_loc(TREE_ATLAS)
            .iter()
            .map(|(l, sz)| format!("{l}:{sz}B"))
            .collect::<Vec<_>>()
            .join(",")
    );

    // ---- 7) R/C/I 引用 ∩ 树 ----
    println!("\n== 7) R/C/I property 引用中的树 prop ==");
    let mut referenced_trees = 0usize;
    for (id, sources) in &referenced {
        let is_tree = signature_trees.contains(id)
            || descriptors.keys().any(|d| {
                tree_records
                    .get(id)
                    .map(|rs| rs.iter().any(|r| r.descriptor == Some(*d)))
                    .unwrap_or(false)
            });
        if is_tree {
            referenced_trees += 1;
            let recs = tree_records.get(id);
            let desc = recs
                .and_then(|rs| rs.first())
                .and_then(|r| r.descriptor)
                .map(|d| format!("{d:08X} [{}]", name_of(d)))
                .unwrap_or_else(|| "-".into());
            println!("  {id:08X} [{}] desc={desc} src={:?}", name_of(*id), sources);
        }
    }
    println!("  → R/C/I 引用的树 resourceID 共 {referenced_trees} 个");

    // ---- 8) OpenSCP 包对拍 ----
    println!("\n== 8) OpenSCP 大学/商业包 prop 引用对拍 ==");
    for label in ["university_s9", "commercial_v7"] {
        let Some((_, pkg)) = packages.iter().find(|(l, _)| l == label) else {
            continue;
        };
        let mut referenced_here: BTreeSet<u32> = BTreeSet::new();
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            let Some(file) = parse_record(pkg, entry) else {
                continue;
            };
            referenced_here.extend(prop_ids(&file));
        }
        println!(
            "  {label}: prop 引用 {} 个",
            referenced_here.len()
        );
        for id in &referenced_here {
            let recs = tree_records.get(id);
            let status = if recs.is_some() {
                let is_sig = recs
                    .map(|rs| rs.iter().any(|r| r.lod_index.is_some()))
                    .unwrap_or(false);
                if is_sig { "tree-signature" } else { "tree-state(no-lod)" }
            } else if referenced.contains_key(id) {
                "non-tree (R/C/I 图谱内)"
            } else {
                "UNRESOLVED (普查包全查无记录)"
            };
            println!("    {id:08X} [{}] {status}", name_of(*id));
        }
    }
}
