//! Decal 绑定形式普查（2026-11-04 任务，组件库 decal tab + 构建器 decal
//! 处理器前期准备）。已有研究底座 docs/blog/decal-rendering-pipeline.md
//! （投影/通道/引擎机制已定案，1423-lot 普查），本探针补三块：
//! 1. 贴花字典（decal atlas）全量普查：三册 group 族（B185/1651/1652）在
//!    全部包中的字典资源、条目数、材质聚类
//! 2. 条目 raster 的四包联合命中（Graphics/EP1/DLC0/Game；裸 Raster
//!    0x2F4E681C 与 RW4 内嵌 0x2F4E681B 双载体）
//! 3. lot 侧绑定形式：DECAL 四列（ID/Transform/Depth/Material）按类别
//!    0..2 的使用分布 + RenderGroup/MachineSpec 可选列存在性 + ID 使用
//!    频次（tab 默认候选）
//!
//! 运行：cargo run --release -p sc-properties --example decal_census

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dbpf::{IndexEntry, Package};
use sc_properties::decal;
use sc_properties::lot_unit as u;
use sc_properties::{Kind, ParseLimits, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const LIVE_ROOT: &str = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow";
const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;

/// 材质聚类（blog doc §1.4 定谳的三个 instance + 废墟双册）。
const MATERIAL_CLUSTERS: [(&str, u32); 4] = [
    ("sign", 0x7368_4EFC),
    ("graffiti", 0xE539_0A98),
    ("ruins", 0x4491_DE3A),
    ("ruins2", 0x44A3_F5BA),
];

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

fn parse_record(pkg: &Package, entry: &IndexEntry) -> Option<PropertyFile> {
    let data = pkg.read(entry).ok()?;
    PropertyFile::parse_with_limits(&data, ParseLimits::default()).ok()
}

fn values_len(file: &PropertyFile, hash: u32) -> usize {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .map(|p| match &p.kind {
            Kind::Scalar(_) => 1,
            Kind::Array(vs) => vs.len(),
            Kind::Empty => 0,
        })
        .sum()
}

fn scalar_values(file: &PropertyFile, hash: u32) -> Vec<f64> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Scalar(v) => vec![v.clone()],
            Kind::Array(vs) => vs.iter().cloned().collect(),
            Kind::Empty => vec![],
        })
        .filter_map(|v| match v {
            Value::UInt32(n) => Some(n as f64),
            Value::Int32(n) => Some(n as f64),
            Value::Float(f) => Some(f as f64),
            _ => None,
        })
        .collect()
}

fn key_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    file.values
        .iter()
        .filter(|p| p.hash == hash)
        .flat_map(|p| match &p.kind {
            Kind::Scalar(v) => vec![v.clone()],
            Kind::Array(vs) => vs.iter().cloned().collect(),
            Kind::Empty => vec![],
        })
        .filter_map(|v| match v {
            Value::Key(k) => Some(k.instance),
            _ => None,
        })
        .collect()
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

    // ---- 1) 字典普查（全包） ----
    println!("== 1) 贴花字典普查（group 低 16 位 B185/1651/1652，全包） ==");
    struct DictInfo {
        package: String,
        group: u32,
        instance: u32,
        material: Option<u32>,
        entries: usize,
        uniform: bool,
    }
    let mut dictionaries: Vec<DictInfo> = Vec::new();
    let mut raster_owner: BTreeMap<u32, (String, String, u32)> = BTreeMap::new();
    for path in walk_packages(LIVE_ROOT) {
        let rel = path
            .strip_prefix(LIVE_ROOT)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        let Ok(pkg) = Package::open(&path) else {
            continue;
        };
        for entry in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_TYPE && decal::is_decal_dictionary_group(e.id.group)
        }) {
            let Some(file) = parse_record(&pkg, entry) else {
                continue;
            };
            if !decal::looks_like_dictionary(&file) {
                continue;
            }
            let dict = decal::DecalDictionary::from_file(&file);
            dictionaries.push(DictInfo {
                package: rel.clone(),
                group: entry.id.group,
                instance: entry.id.instance,
                material: dict.material.map(|k| k.instance),
                entries: dict.entries.len(),
                uniform: dict.uniform_arrays,
            });
            for e in &dict.entries {
                if let Some(raster) = &e.raster {
                    raster_owner.entry(raster.instance).or_insert((
                        rel.clone(),
                        name_of(e.id.map(|k| k.instance).unwrap_or(0)),
                        0,
                    ));
                }
            }
        }
    }
    println!(
        "  字典 {} 册；条目总数 {}",
        dictionaries.len(),
        dictionaries.iter().map(|d| d.entries).sum::<usize>()
    );
    for d in &dictionaries {
        println!(
            "    {} G {:08X} I {:08X} [{}] entries={} uniform={} material={}",
            d.package,
            d.group,
            d.instance,
            name_of(d.instance),
            d.entries,
            d.uniform,
            d.material
                .map(|m| format!("{m:08X}"))
                .unwrap_or_else(|| "-".into())
        );
    }
    let mut cluster_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &dictionaries {
        let cluster = MATERIAL_CLUSTERS
            .iter()
            .find(|(_, m)| Some(*m) == d.material)
            .map(|(name, _)| *name)
            .unwrap_or("unknown");
        *cluster_counts.entry(cluster).or_default() += d.entries;
    }
    println!("  材质聚类条目数: {cluster_counts:?}");

    // ---- 2) raster 四包联合命中 ----
    println!("\n== 2) 条目 raster 四包联合命中（Graphics/EP1/DLC0/Game） ==");
    let mut joint: BTreeMap<String, Package> = BTreeMap::new();
    for rel in [
        "SimCityData/SimCity_Graphics.package",
        "SimCityData/SimCityDataEP1.package",
        "SimCityData/SimCity_DLC0.package",
        "SimCityData/SimCity_Game.package",
    ] {
        if let Ok(pkg) = Package::open(Path::new(LIVE_ROOT).join(rel)) {
            joint.insert(rel.to_string(), pkg);
        }
    }
    let mut hit = 0usize;
    let mut miss = 0usize;
    let mut per_package: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_type: BTreeMap<u32, usize> = BTreeMap::new();
    for raster_instance in raster_owner.keys() {
        let mut found = false;
        for (rel, pkg) in &joint {
            for e in pkg.entries().iter().filter(|e| {
                e.id.instance == *raster_instance
                    && (e.id.type_id == RASTER_TYPE || e.id.type_id == RW4_MODEL_TYPE)
            }) {
                found = true;
                hit += 1;
                let label = rel
                    .split('/')
                    .next_back()
                    .unwrap_or(rel)
                    .trim_end_matches(".package")
                    .to_string();
                *per_package.entry(label).or_default() += 1;
                *per_type.entry(e.id.type_id).or_default() += 1;
                break;
            }
            if found {
                break;
            }
        }
        if !found {
            miss += 1;
        }
    }
    println!(
        "  去重 raster {} 个：四包命中 {hit}（裸 Raster {} / RW4 {}），未命中 {miss}",
        raster_owner.len(),
        per_type.get(&RASTER_TYPE).copied().unwrap_or(0),
        per_type.get(&RW4_MODEL_TYPE).copied().unwrap_or(0)
    );
    println!("  所在包分布: {per_package:?}");

    // ---- 3) lot 侧绑定形式（Graphics/EP1/DLC0/Game 全 B1B104 扫描） ----
    println!("\n== 3) lot 侧 decal 绑定形式 ==");
    let id_bases = [
        u::DECAL_ID_BASE,
        u::DECAL_TRANSFORM_BASE,
        u::DECAL_DEPTH_BASE,
        u::DECAL_MATERIAL_BASE,
    ];
    let render_group_hash: u32 = 0x0D10_9090;
    let machine_spec_hash: u32 = 0x0D10_90A0;
    let mut lots_with_decal = 0usize;
    let mut category_usage: BTreeMap<u32, usize> = BTreeMap::new();
    let mut column_presence: BTreeMap<u32, usize> = BTreeMap::new();
    let mut render_group_lots = 0usize;
    let mut machine_spec_lots = 0usize;
    let mut used_ids: BTreeMap<u32, usize> = BTreeMap::new();
    let mut depth_min = f64::INFINITY;
    let mut depth_max = f64::NEG_INFINITY;
    let mut hole_light_params: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    let mut material_samples: Vec<String> = Vec::new();
    for (rel, pkg) in &joint {
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            let Some(file) = parse_record(&pkg, entry) else {
                continue;
            };
            let mut has_decal = false;
            for cat in 0..u::DECAL_CATEGORIES as u32 {
                let ids = key_instances(&file, u::DECAL_ID_BASE + cat);
                if ids.is_empty() {
                    continue;
                }
                has_decal = true;
                *category_usage.entry(cat).or_default() += ids.len();
                for id in ids {
                    *used_ids.entry(id).or_default() += 1;
                }
            }
            if has_decal {
                lots_with_decal += 1;
                for base in id_bases {
                    if values_len(&file, base) > 0 {
                        *column_presence.entry(base).or_default() += 1;
                    }
                }
                if values_len(&file, render_group_hash) > 0 {
                    render_group_lots += 1;
                }
                if values_len(&file, machine_spec_hash) > 0 {
                    machine_spec_lots += 1;
                }
                for depth in scalar_values(&file, u::DECAL_DEPTH_BASE) {
                    depth_min = depth_min.min(depth);
                    depth_max = depth_max.max(depth);
                }
                // MaterialData Vector3 样本（处理器默认值依据）
                for p in file.values.iter().filter(|p| {
                    (u::DECAL_MATERIAL_BASE..u::DECAL_MATERIAL_BASE + 3).contains(&p.hash)
                }) {
                    if material_samples.len() < 12 {
                        if let Kind::Array(vs) = &p.kind {
                            let xyz: Vec<String> = vs.iter().map(|v| v.to_string()).collect();
                            material_samples.push(format!("{:08X}:[{}]", p.hash, xyz.join(",")));
                        }
                    }
                }
                // 破洞假内景光参数（blog doc §1.2：0x0DA76A05/06 标量 Float）
                let lights = scalar_values(&file, 0x0DA7_6A05);
                let radii = scalar_values(&file, 0x0DA7_6A06);
                for (l, r) in lights.iter().zip(radii.iter()) {
                    *hole_light_params
                        .entry(((l * 1000.0) as u64, (r * 1000.0) as u64))
                        .or_default() += 1;
                }
            }
        }
        let _ = rel;
    }
    println!("  含 decal 的 lot {lots_with_decal} 个（四包合计）");
    println!("  列存在 lot 数: {column_presence:?}");
    println!("  类别使用（ID 条目数）: {category_usage:?}");
    println!("  RenderGroup lot {render_group_lots} / MachineSpec lot {machine_spec_lots}");
    if depth_min.is_finite() {
        println!("  Depth 范围 {depth_min:.3}..{depth_max:.3} m");
    }
    println!("  破洞光参数 (lightScale*1000, invRadius*1000): {hole_light_params:?}");
    println!("  MaterialData Vector3 样本: {:?}", material_samples);

    // ---- 4) ID 使用频次（tab 默认候选）与字典覆盖 ----
    println!("\n== 4) 贴花 ID 使用频次（top 20，tab 默认候选） ==");
    let mut by_use: Vec<(u32, usize)> = used_ids.iter().map(|(k, v)| (*k, *v)).collect();
    by_use.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (id, n) in by_use.iter().take(20) {
        println!("  {id:08X} [{}] ×{n}", name_of(*id));
    }
    println!(
        "  使用去重 ID {} 个；字典去重 raster {} 个（ID→字典条目映射在 PE resolve_decal_textures 已实现）",
        used_ids.len(),
        raster_owner.len()
    );
}
