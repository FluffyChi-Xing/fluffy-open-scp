//! 普通建筑车辆 prop bin 普查（2026-11-04 任务）：
//! 1. 锚点对拍：热气球公园 8D7A596C@G 43F0C000（3 车 id / 17 prop / slot 9）、
//!    R_ 建筑 B8415837@G 41E1C000（CarW2 / 10 prop / slot 3）——逐 bin 全列 dump
//! 2. 全量普查：Game/EP1 全部 B1B104 lot 记录的 14 个 prop bin → 哪些
//!    resourceID 被普通建筑绑定（权威车辆清单）、slot/randomize/percent 分布
//! 3. 车辆 id 的解析链分类（复用 tree_hunt 的 resolve 语义：vehicle_models 等）
//!
//! 运行：cargo run --release -p sc-properties --example car_census

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dbpf::{IndexEntry, Package};
use sc_properties::lot_unit as u;
use sc_properties::{Kind, ParseLimits, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const LIVE_ROOT: &str = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow";
const PROP_ID_BASE: u32 = u::PROP_ID_BASE;
const PROP_TRANSFORM_BASE: u32 = u::PROP_TRANSFORM_BASE;
const PROP_SLOT_BASE: u32 = u::PROP_SLOT_BASE;
const PROP_RANDOMIZE_SLOT_BASE: u32 = u::PROP_RANDOMIZE_SLOT_BASE;
const PROP_PERCENT_FILL_BASE: u32 = u::PROP_PERCENT_FILL_BASE;
const PROP_BINS: usize = u::PROP_BINS;

/// 用户锚点：(instance, group)。
const ANCHORS: [(u32, u32); 2] = [(0x8D7A_596C, 0x43F0_C000), (0xB841_5837, 0x41E1_C000)];

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

fn int_values(file: &PropertyFile, hash: u32) -> Vec<i32> {
    values_of(file, hash)
        .iter()
        .filter_map(|v| match v {
            Value::UInt32(n) => Some(*n as i32),
            Value::Int32(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn bool_values(file: &PropertyFile, hash: u32) -> Vec<bool> {
    values_of(file, hash)
        .iter()
        .filter_map(|v| match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        })
        .collect()
}

/// 一个 bin 的全列快照。
struct BinDump {
    bin: usize,
    ids: Vec<u32>,
    transforms: usize,
    slots: Vec<i32>,
    randomize: Vec<bool>,
    percent: Vec<bool>,
}

fn dump_bins(file: &PropertyFile) -> Vec<BinDump> {
    let mut out = Vec::new();
    for bin in 0..PROP_BINS {
        let ids = key_ids(file, PROP_ID_BASE + bin as u32);
        if ids.is_empty() {
            continue;
        }
        out.push(BinDump {
            bin,
            transforms: values_of(file, PROP_TRANSFORM_BASE + bin as u32).len(),
            slots: int_values(file, PROP_SLOT_BASE + bin as u32),
            randomize: bool_values(file, PROP_RANDOMIZE_SLOT_BASE + bin as u32),
            percent: bool_values(file, PROP_PERCENT_FILL_BASE + bin as u32),
            ids,
        });
    }
    out
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

    // ---- Game + EP1 的全部 lot 记录（建筑本体在安装包；EcoGame 是脚本资源） ----
    let mut lots: Vec<(String, u32, u32, PropertyFile)> = Vec::new();
    for rel in [
        "SimCityData/SimCity_Game.package",
        "SimCityData/SimCityDataEP1.package",
    ] {
        let path = Path::new(LIVE_ROOT).join(rel);
        let Ok(pkg) = Package::open(&path) else {
            eprintln!("SKIP {rel}");
            continue;
        };
        let mut n = 0usize;
        for entry in pkg
            .entries()
            .iter()
            .filter(|e| e.id.type_id == PROPERTY_TYPE)
        {
            if let Some(file) = parse_record(&pkg, entry) {
                lots.push((rel.to_string(), entry.id.group, entry.id.instance, file));
                n += 1;
            }
        }
        println!("scanned {rel}: {n} 条 B1B104");
    }

    // ---- 1) 锚点对拍（全包搜索：Game/EP1 找不到时走全盘 walk） ----
    println!("\n== 1) 锚点对拍 ==");
    let mut anchor_hits: Vec<(String, u32, u32, PropertyFile)> = Vec::new();
    for (instance, group) in ANCHORS {
        if let Some((rel, g, i, file)) = lots
            .iter()
            .find(|(_, g, i, _)| *g == group && *i == instance)
        {
            anchor_hits.push((rel.clone(), *g, *i, file.clone()));
        } else {
            // 全盘搜索（任意包、任意 type）
            let mut found = false;
            for path in walk_packages(LIVE_ROOT) {
                let Ok(pkg) = Package::open(&path) else {
                    continue;
                };
                let Some(entry) = pkg.entries().iter().find(|e| e.id.instance == instance) else {
                    continue;
                };
                let rel = path
                    .strip_prefix(LIVE_ROOT)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                println!(
                    "  {instance:08X}@G {group:08X}: Game/EP1 无 → 全盘命中 {rel} T {:08X} G {:08X}",
                    entry.id.type_id, entry.id.group
                );
                if let Some(file) = parse_record(&pkg, entry) {
                    anchor_hits.push((rel, entry.id.group, instance, file));
                }
                found = true;
                break;
            }
            if !found {
                println!("  {instance:08X}@G {group:08X}: 全盘未找到！");
            }
        }
    }
    for (rel, group, instance, file) in &anchor_hits {
        println!(
            "  {instance:08X}@G {group:08X} [{name}] @{rel}",
            name = name_of(*instance)
        );
        for bin in dump_bins(file) {
            let mut ids: BTreeMap<u32, usize> = BTreeMap::new();
            for id in &bin.ids {
                *ids.entry(*id).or_default() += 1;
            }
            let id_summary: Vec<String> = ids
                .iter()
                .map(|(id, n)| format!("{id:08X}[{}]×{n}", name_of(*id)))
                .collect();
            println!(
                "    bin{}: props={} transforms={} slots={:?} randomize={:?} percent={:?}\n      ids: {}",
                bin.bin,
                bin.ids.len(),
                bin.transforms,
                bin.slots,
                bin.randomize,
                bin.percent,
                id_summary.join(", ")
            );
        }
    }

    // ---- 2) 全量普查：bin 绑定的 resourceID → 统计 ----
    println!("\n== 2) 全部 lot prop bin 绑定统计（Game+EP1） ==");
    struct IdStat {
        lots: usize,
        props: usize,
        slots: Vec<i32>,
        percent_used: usize,
        randomize_used: usize,
        groups: BTreeSet<u32>,
    }
    let mut stats: BTreeMap<u32, IdStat> = BTreeMap::new();
    let mut lots_with_bins = 0usize;
    for (rel, group, instance, file) in &lots {
        let bins = dump_bins(file);
        if bins.is_empty() {
            continue;
        }
        lots_with_bins += 1;
        for bin in bins {
            for id in &bin.ids {
                let stat = stats.entry(*id).or_insert(IdStat {
                    lots: 0,
                    props: 0,
                    slots: Vec::new(),
                    percent_used: 0,
                    randomize_used: 0,
                    groups: BTreeSet::new(),
                });
                stat.lots += 1;
                stat.props += 1;
                stat.slots.extend(bin.slots.iter().copied());
                stat.percent_used += bin.percent.iter().filter(|b| **b).count();
                stat.randomize_used += bin.randomize.iter().filter(|b| **b).count();
                stat.groups.insert(*group);
            }
        }
        let _ = (rel, instance);
    }
    println!(
        "  有 prop bin 的 lot {lots_with_bins} 个；被绑定 resourceID {} 个\n",
        stats.len()
    );
    let mut rows: Vec<(&u32, &IdStat)> = stats.iter().collect();
    rows.sort_by_key(|(id, s)| (std::cmp::Reverse(s.lots), **id));
    for (id, s) in rows.iter().take(40) {
        let slots_min = s.slots.iter().min().copied();
        let slots_max = s.slots.iter().max().copied();
        let groups = s
            .groups
            .iter()
            .map(|g| format!("{g:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  {id:08X} [{}] lots={} props={} slot={slots_min:?}..{slots_max:?} percent_on={} rand_on={} groups={groups}",
            name_of(**id),
            s.lots,
            s.props,
            s.percent_used,
            s.randomize_used,
        );
    }

    // ---- 3) 车辆 id 清单（R/C/I 普查已证 0AB4DFE1/2/3 = CarW1/W2/W3） ----
    println!("\n== 3) 普通建筑车辆候选（绑定 lots ≥ 20 且名字含 car 的 id） ==");
    for (id, s) in rows.iter() {
        let name = name_of(**id).to_lowercase();
        let car_like = ["car", "vehicle", "sedan", "moped", "taxi"]
            .iter()
            .any(|w| name.contains(w));
        if car_like && s.lots >= 20 {
            println!(
                "  {id:08X} [{}] lots={} props={} slot={:?}..{:?}",
                name_of(**id),
                s.lots,
                s.props,
                s.slots.iter().min(),
                s.slots.iter().max()
            );
        }
    }

    // ---- 4) slot 与 props 的关系（按 id 聚合离散分布） ----
    println!("\n== 4) slot 值分布（CarW1/W2/W3） ==");
    for car in [0x0AB4_DFE1u32, 0x0AB4_DFE2, 0x0AB4_DFE3] {
        let Some(s) = stats.get(&car) else {
            continue;
        };
        let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
        for v in &s.slots {
            *hist.entry(*v).or_default() += 1;
        }
        println!(
            "  {car:08X} [{}]: {} 个 slot 值直方 {:?}",
            name_of(car),
            s.slots.len(),
            hist
        );
    }

    // ---- 5) 财富等级假设：lot 名 W1/W2/W3 标记 × 绑定 CarW1/W2/W3 列联 ----
    println!("\n== 5) 财富等级（lot 名 _W1_/_W2_/_W3_）× 车辆等级列联 ==");
    let mut grid: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut unnamed = 0usize;
    for (_, _, instance, file) in &lots {
        let bins = dump_bins(file);
        let mut car_set: BTreeSet<u32> = BTreeSet::new();
        for bin in &bins {
            for id in &bin.ids {
                if matches!(*id, 0x0AB4_DFE1 | 0x0AB4_DFE2 | 0x0AB4_DFE3) {
                    car_set.insert(*id);
                }
            }
        }
        if car_set.is_empty() {
            continue;
        }
        let cars: Vec<&str> = ["W1", "W2", "W3"]
            .iter()
            .filter(|w| {
                let id = match **w {
                    "W1" => 0x0AB4_DFE1,
                    "W2" => 0x0AB4_DFE2,
                    _ => 0x0AB4_DFE3,
                };
                car_set.contains(&id)
            })
            .map(|w| if car_set.len() > 1 { "mix" } else { *w })
            .collect();
        let car_key = if cars.len() > 1 {
            "mix".to_string()
        } else {
            cars.first().copied().unwrap_or("?").to_string()
        };
        let name = name_of(*instance).to_lowercase();
        let wealth = ["_w1_", "_w2_", "_w3_"]
            .iter()
            .find(|w| name.contains(*w))
            .map(|w| w.trim_matches('_').to_uppercase())
            .unwrap_or_else(|| {
                unnamed += 1;
                "无名".to_string()
            });
        *grid.entry((wealth, car_key)).or_default() += 1;
    }
    for ((wealth, car), n) in &grid {
        println!("  lot 名财富={wealth:3} → 车={car:3} : {n}");
    }
    println!("  （无名 lot = 未计入列联样本 {} 个）", unnamed);

    // ---- 6) Graphics 建筑记录（R/C/I 命名）：名字财富 × 绑定车辆列联 ----
    println!("\n== 6) Graphics R/C/I 建筑记录财富×车辆列联（权威验证） ==");
    let gfx = Package::open(Path::new(LIVE_ROOT).join("SimCityData/SimCity_Graphics.package"))
        .expect("graphics");
    let mut grid2: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut named_with_cars = 0usize;
    let mut mix_samples: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let car_for = |w: &str| match w {
        "W1" => 0x0AB4_DFE1,
        "W2" => 0x0AB4_DFE2,
        _ => 0x0AB4_DFE3,
    };
    for entry in gfx
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROPERTY_TYPE)
    {
        let Some(rec) = registry.instances().get(&entry.id.instance) else {
            continue;
        };
        let name = rec.name.trim();
        if !["R_", "C_", "I_"].iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let Some(file) = parse_record(&gfx, entry) else {
            continue;
        };
        let mut car_set: BTreeSet<u32> = BTreeSet::new();
        for bin in dump_bins(&file) {
            for id in &bin.ids {
                if matches!(*id, 0x0AB4_DFE1 | 0x0AB4_DFE2 | 0x0AB4_DFE3) {
                    car_set.insert(*id);
                }
            }
        }
        if car_set.is_empty() {
            continue;
        }
        named_with_cars += 1;
        let car_key = {
            let mut parts: Vec<&str> = ["W1", "W2", "W3"]
                .iter()
                .copied()
                .filter(|w| car_set.contains(&car_for(w)))
                .collect();
            if parts.len() > 1 {
                parts.join("+")
            } else {
                parts.pop().unwrap_or("?").to_string()
            }
        };
        if car_set.len() > 1 {
            let lower0 = name.to_lowercase();
            mix_samples.entry(car_key.clone()).or_default().push(lower0);
        }
        let lower = name.to_lowercase();
        let wealth = ["w1d", "w2d", "w3d", "wl0", "wl1", "wl2", "wl3"]
            .iter()
            .find_map(|w| {
                if lower.contains(w) {
                    Some(w[..2].to_uppercase())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "?".to_string());
        *grid2.entry((wealth, car_key)).or_default() += 1;
    }
    for ((wealth, car), n) in &grid2 {
        println!("  建筑名财富={wealth:3} → 车={car:6} : {n}");
    }
    println!("  --- mix 构成样本（每组成分前 3 个名字）---");
    for (composition, names) in &mix_samples {
        println!("  [{composition}] {} 条，样本:", names.len());
        for name in names.iter().take(3) {
            println!("    {name}");
        }
    }
    println!("  （绑定车辆的建筑记录 {named_with_cars} 条）");

    // ---- 7) 车皮颜色取证：三档车的 LOD 模型 → 材质贴图槽 → 调色板色数 ----
    // 每档 4 个 RW4 LOD（tree_census 解析输出）；取最大 LOD 解材质 0（车壳）。
    println!("\n== 7) 车皮颜色取证（材质贴图槽 + 调色板色数） ==");
    let car_lods: [(&str, [u32; 4]); 3] = [
        ("W1", [0x315F_77E8, 0xDFE2_FF49, 0xDFE2_FF4A, 0xDFE2_FF4B]),
        ("W2", [0xCA26_5D8B, 0x0534_ACA8, 0x0534_ACAB, 0x0534_ACAA]),
        ("W3", [0x821A_DA65, 0xEBEE_675E, 0xEBEE_675D, 0xEBEE_675C]),
    ];
    let gfx = Package::open(Path::new(LIVE_ROOT).join("SimCityData/SimCity_Graphics.package"))
        .expect("graphics");
    let read_rw4 = |instance: u32| -> Option<(Vec<u8>, u32)> {
        let entry = gfx
            .entries()
            .iter()
            .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == instance)?;
        let size = entry.decompressed_size;
        gfx.read(entry).ok().map(|data| (data, size))
    };
    for (label, lods) in &car_lods {
        let Some((best_instance, (best, size))) = lods
            .iter()
            .copied()
            .filter_map(|i| read_rw4(i).map(|(d, s)| (i, (d, s))))
            .max_by_key(|(_, (_, s))| *s)
        else {
            println!("  Car {label}: 模型未找到");
            continue;
        };
        let Ok(file) = rw4::Rw4File::parse(&best) else {
            println!("  Car {label}: RW4 解析失败");
            continue;
        };
        let mat_count = file.sections_of_type(rw4::SectionType::MATERIAL).count();
        let assignment_count = file
            .sections_of_type(rw4::SectionType::MESH_MATERIAL_ASSIGNMENT)
            .count();
        println!(
            "  Car {label} LOD {best_instance:08X} ({size}B) materials={mat_count} assignments={assignment_count}"
        );
        let mut seen_instances: BTreeSet<u32> = BTreeSet::new();
        let _ = std::fs::create_dir_all("tmp");
        let mat_numbers: Vec<u32> = file
            .sections_of_type(rw4::SectionType::MATERIAL)
            .map(|s| s.number)
            .collect();
        for mat_no in mat_numbers {
            let material = match file.decode_material(&best, mat_no) {
                Ok(m) => m,
                Err(e) => {
                    println!("    mat{mat_no} 解码失败: {e}");
                    break;
                }
            };
            let refs = material.texture_refs().len();
            if refs == 0 {
                println!("    mat{mat_no}: 无贴图引用");
                continue;
            }
            println!("    mat{mat_no}: {refs} 条贴图引用");
            for slot_ref in material.texture_refs() {
                let slot = slot_ref.slot_byte();
                let instance = slot_ref.texture_instance;
                if slot > 5 || instance == 0 || !seen_instances.insert(instance) {
                    continue;
                }
                let Some((tex_bytes, _)) = read_rw4(instance) else {
                    continue;
                };
                let Ok(tex_file) = rw4::Rw4File::parse(&tex_bytes) else {
                    continue;
                };
                let Some(sec) = tex_file
                    .sections_of_type(rw4::SectionType::TEXTURE)
                    .next()
                    .map(|s| s.number)
                else {
                    continue;
                };
                let Ok(tex) = tex_file.decode_texture(&tex_bytes, sec) else {
                    continue;
                };
                let Ok(rgba) = tex.decode_top_mip_rgba() else {
                    continue;
                };
                let mut colors: BTreeSet<[u8; 3]> = BTreeSet::new();
                for px in rgba.chunks_exact(4) {
                    if px[3] > 128 {
                        colors.insert([px[0] / 32, px[1] / 32, px[2] / 32]);
                    }
                }
                let path = format!("tmp/car_{label}_s{slot}_{}x{}.png", tex.width, tex.height);
                let img = image::RgbaImage::from_fn(tex.width as u32, tex.height as u32, |x, y| {
                    let at = (y as usize * tex.width as usize + x as usize) * 4;
                    image::Rgba([rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]])
                });
                let _ = image::DynamicImage::ImageRgba8(img)
                    .save_with_format(&path, image::ImageFormat::Png);
                println!(
                    "    slot{slot} → tex {instance:08X} {}x{} 去重色档 {} → {path}",
                    tex.width,
                    tex.height,
                    colors.len()
                );
            }
            break; // 只看第一个材质（车壳）
        }
    }
}
