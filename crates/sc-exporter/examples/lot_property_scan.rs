//! 取证探针：列出同一 instance 下的全部 lot property 变体（不同 group），
//! 打印各自的 LotSize / LotMask / Lot Textures / LotColor1-4 / LOD 模型。
//!
//! 用途：裁定"用户看到的地面"到底来自哪一条 property（同 instance 多 group
//! 时渲染链路与探针可能各取一条）。
//!
//! 用法：
//! ```text
//! lot_property_scan <package> --instance=<hex> [--model=<hex>] [extra_package...]
//! ```

use dbpf::{IndexEntry, Package};
use sc_properties::{Key, Kind, LotEditorDocument, PropertyFile, Value};
use std::collections::HashSet;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const H_LOT_COLORS: [u32; 4] = [0x0D02_D586, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];
/// `0x00B2CCCB` = Parent：未在本级出现的键从父级递归继承（C# `PropertyFile.
/// GetParentProperties`）。地表授权（LotColors / Lot Textures / LotSize）常挂在
/// 父级，子级只覆盖 LotMask 与 LOD。
const H_PARENT: u32 = 0x00B2_CCCB;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<String> = raw
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .cloned()
        .collect();
    let flag = |name: &str| -> Option<u32> {
        raw.iter()
            .find_map(|arg| arg.strip_prefix(&format!("--{name}=")))
            .map(|value| {
                u32::from_str_radix(value.trim_start_matches("0x"), 16)
                    .unwrap_or_else(|error| panic!("bad --{name}= {value}: {error}"))
            })
    };
    let Some(path) = positional.first() else {
        eprintln!("usage: lot_property_scan <package> --instance=<hex> [--model=<hex>] [extra...]");
        std::process::exit(2);
    };
    let mut paths = vec![path.clone()];
    paths.extend(positional[1..].iter().cloned());
    if raw.iter().any(|arg| arg == "--survey") {
        survey(&paths);
        return;
    }
    // --find=<instance>：列出全部包里该 instance 的所有条目（任意类型）。
    if let Some(target) = flag("find") {
        for (index, path) in paths.iter().enumerate() {
            let Ok(package) = Package::open(path) else {
                continue;
            };
            for entry in package.entries() {
                if entry.id.instance != target {
                    continue;
                }
                let name = path.rsplit('/').next().unwrap_or(path);
                println!(
                    "  pkg#{index} {name:<28} T 0x{:08X} G 0x{:08X} I 0x{:08X}  {}B",
                    entry.id.type_id, entry.id.group, entry.id.instance, entry.decompressed_size
                );
            }
        }
        return;
    }

    let Some(instance) = flag("instance") else {
        eprintln!("usage: lot_property_scan <package> --instance=<hex> [--model=<hex>] [--find=<hex>] [extra...]");
        std::process::exit(2);
    };
    let model = flag("model");

    let packages: Vec<Package> = paths
        .iter()
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();

    let mut hits = 0usize;
    for (owner, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE || entry.id.instance != instance {
                continue;
            }
            let Ok(file) = PropertyFile::parse(&package.read(entry).unwrap()) else {
                continue;
            };
            let doc = LotEditorDocument::from_property_file(file.clone());
            if let Some(model) = model
                && !doc.model_lods.iter().flatten().any(|k| k.instance == model)
            {
                continue;
            }
            hits += 1;
            println!(
                "\n#{}  T=0x{:08X} G=0x{:08X} I=0x{:08X}  pkg={}",
                hits, entry.id.type_id, entry.id.group, entry.id.instance, paths[owner]
            );
            println!("  LotSize      = {:?}", doc.lot_size);
            println!("  LotOffset    = {:?}", doc.lot_offset);
            println!("  LotMask      = {}", fmt_key(doc.lot_mask));
            println!("  Lot Textures = {}", fmt_key(doc.lot_textures));
            for (index, hash) in H_LOT_COLORS.iter().enumerate() {
                println!(
                    "  LotColor{}    = {}",
                    index + 1,
                    fmt_color(color_at(&file, *hash))
                );
            }
            for (index, lod) in doc.model_lods.iter().enumerate() {
                println!("  LOD{}         = {}", index + 1, fmt_key(lod.clone()));
            }
            let unknown: Vec<String> = file
                .values
                .iter()
                .filter(|p| {
                    ![
                        0x0975_695F_u32, // Model Details
                        0x0CCB_7FD5,     // LotMask
                        0x0CCB_7FD4,     // Lot Textures
                        0x0CCB_7FC8,     // LotSize
                        0x0CCB_7FC9,     // LotOffset
                        0x0CCB_7FCA,     // Placement
                        0x0D02_D586,
                        0x0D02_D587,
                        0x0D02_D588,
                        0x0D02_D589,
                    ]
                    .contains(&p.hash)
                })
                .map(|p| format!("0x{:08X}", p.hash))
                .collect();
            println!("  其它属性 ({}): {}", unknown.len(), unknown.join(" "));
        }
    }
    println!("\n共 {hits} 条匹配 property（instance=0x{instance:08X}）");

    if raw.iter().any(|arg| arg == "--chain") {
        let group = flag("group");
        let start = packages
            .iter()
            .find_map(|package| {
                package
                    .entries()
                    .iter()
                    .find(|e| {
                        e.id.type_id == PROPERTY_TYPE
                            && e.id.instance == instance
                            && group.is_none_or(|g| e.id.group == g)
                    })
                    .cloned()
            })
            .unwrap_or_else(|| panic!("property 0x{instance:08X} not found"));
        walk_chain(&packages, start);
    }
}

/// 统计地表授权的缺失与继承依赖：有多少 lot property 自身没有 LotColors /
/// Lot Textures，其中多少带 `Parent`（即只有展平继承才能拿到）。
fn survey(paths: &[String]) {
    let h_textures = 0x0CCB_7FD4_u32;
    // 疑似 lot 地面几何/图集参数的未定名键（注册表无名字，见 lot-rendering.md §1）。
    let candidates: [u32; 6] = [
        0x0CCB_7FD0,
        0x0CCB_7FD1,
        0x0CCB_7FD2,
        0x0CCB_7FD3,
        0x0CCB_7FD6,
        0x0CCB_7FD7,
    ];
    let mut total = 0usize;
    let mut with_parent = 0usize;
    let mut missing_textures = 0usize;
    let mut missing_colors = 0usize;
    let mut missing_textures_with_parent = 0usize;
    let mut missing_colors_with_parent = 0usize;
    for path in paths {
        let Ok(package) = Package::open(path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let Ok(file) = PropertyFile::parse(&package.read(entry).unwrap()) else {
                continue;
            };
            total += 1;
            let has_parent = file.get(H_PARENT).is_some();
            if has_parent {
                with_parent += 1;
            }
            let no_textures = file.get(h_textures).is_none();
            let no_colors = H_LOT_COLORS.iter().all(|hash| file.get(*hash).is_none());
            if no_textures {
                missing_textures += 1;
                if has_parent {
                    missing_textures_with_parent += 1;
                }
            }
            if no_colors {
                missing_colors += 1;
                if has_parent {
                    missing_colors_with_parent += 1;
                }
            }
        }
    }
    println!("property(0x00B1B104) 总数           = {total}");
    println!("  带 Parent(0x00B2CCCB) 的           = {with_parent}");
    println!(
        "  自身无 Lot Textures 的             = {missing_textures}（其中 {missing_textures_with_parent} 带 Parent → 靠继承才能拿到）"
    );
    println!(
        "  自身无 LotColor1-4 的              = {missing_colors}（其中 {missing_colors_with_parent} 带 Parent）"
    );

    // 未定名键的取值分布：若 0x0CCB7FD0 是「底图格号」，应逐 lot 不同；
    // 若几乎是常数则它不是（和/或由引擎侧生成）。
    let mut hist: Vec<(u32, std::collections::BTreeMap<String, usize>)> = candidates
        .iter()
        .map(|hash| (*hash, std::collections::BTreeMap::new()))
        .collect();
    let mut seen_keys = [0usize; 6];
    for path in paths {
        let Ok(package) = Package::open(path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let Ok(file) = PropertyFile::parse(&package.read(entry).unwrap()) else {
                continue;
            };
            for (slot, hash) in candidates.iter().enumerate() {
                if let Some(property) = file.get(*hash) {
                    seen_keys[slot] += 1;
                    *hist[slot]
                        .1
                        .entry(first_value(property).map_or("<empty>".to_owned(), |v| {
                            format!("{v}")
                        }))
                        .or_default() += 1;
                }
            }
        }
    }
    println!("\n未定名几何/格号键的取值分布（全库，出现该键的 property 数）：");
    for (slot, hash) in candidates.iter().enumerate() {
        if seen_keys[slot] == 0 {
            println!("  0x{hash:08X}: 未出现");
            continue;
        }
        let mut top: Vec<(&String, &usize)> = hist[slot].1.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        let shown: Vec<String> = top
            .iter()
            .take(6)
            .map(|(value, count)| format!("{value}×{count}"))
            .collect();
        println!(
            "  0x{hash:08X}: {} 个 property 带此键；最常见值 {}",
            seen_keys[slot],
            shown.join("  ")
        );
    }
}

/// 沿 `Parent`(0x00B2CCCB) 链合并属性（子级优先），打印每级来源与合并后的
/// 地表授权字段。
fn walk_chain(packages: &[Package], start: IndexEntry) {
    println!("\n===== Parent 链合并（0x00B2CCCB，子级优先）=====");
    let mut merged = PropertyFile::default();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut current = Some((start.id.type_id, start.id.group, start.id.instance));
    let mut depth = 0usize;
    while let Some((type_id, group, instance)) = current {
        if !seen.insert(instance) {
            println!("  [{depth}] 0x{instance:08X} 循环引用，停止");
            break;
        }
        // 精确 TGI 优先（同 instance 常有多 group 变体），再退化为同类型同 instance。
        let locate = |exact_group: bool| {
            packages.iter().find_map(|package| {
                package
                    .entries()
                    .iter()
                    .find(|e| {
                        e.id.type_id == type_id
                            && e.id.instance == instance
                            && (!exact_group || e.id.group == group)
                    })
                    .map(|e| (package, e.clone()))
            })
        };
        let Some((package, entry)) = locate(true).or_else(|| locate(false)) else {
            println!("  [{depth}] 0x{instance:08X} 资源缺失，停止");
            break;
        };
        let file = PropertyFile::parse(&package.read(&entry).unwrap()).unwrap();
        let mut added = 0usize;
        for property in file.values.iter().filter(|p| p.hash != H_PARENT) {
            if merged.values.iter().all(|m| m.hash != property.hash) {
                merged.values.push(property.clone());
                added += 1;
            }
        }
        println!(
            "  [{depth}] G=0x{:08X} I=0x{instance:08X}  {} 键（新增 {added}）",
            entry.id.group,
            file.values.len()
        );
        let _ = group;
        current = file
            .get(H_PARENT)
            .and_then(first_value)
            .and_then(|value| match value {
                Value::Key(key) => Some((key.type_id, key.group, key.instance)),
                _ => None,
            });
        depth += 1;
    }
    merged.claimed_count = merged.values.len() as u32;
    let doc = LotEditorDocument::from_property_file(merged);
    println!("\n  合并后LotSize      = {:?}", doc.lot_size);
    println!("  合并后LotMask      = {}", fmt_key(doc.lot_mask));
    println!("  合并后Lot Textures = {}", fmt_key(doc.lot_textures));
    for (index, hash) in H_LOT_COLORS.iter().enumerate() {
        println!("  合并后LotColor{}    = {}", index + 1, fmt_color(color_at(&doc.properties, *hash)));
    }
}

/// 取属性的值本体（标量本体或数组首元素）。
fn first_value(property: &sc_properties::Property) -> Option<Value> {
    match &property.kind {
        Kind::Scalar(value) => Some(value.clone()),
        Kind::Array(values) => values.first().cloned(),
        Kind::Empty => None,
    }
}

fn fmt_key(key: Option<Key>) -> String {
    key.map_or_else(
        || "<missing>".to_owned(),
        |key| {
            format!(
                "T 0x{:08X} G 0x{:08X} I 0x{:08X}",
                key.type_id, key.group, key.instance
            )
        },
    )
}

fn fmt_color(color: Option<[f32; 4]>) -> String {
    color.map_or_else(
        || "<missing>".to_owned(),
        |c| format!("({:.4}, {:.4}, {:.4}) A={:.4}", c[0], c[1], c[2], c[3]),
    )
}

fn scalar(file: &PropertyFile, hash: u32) -> Option<Value> {
    match &file.get(hash)?.kind {
        Kind::Scalar(value) => Some(value.clone()),
        Kind::Array(values) => values.first().cloned(),
        Kind::Empty => None,
    }
}

fn color_at(file: &PropertyFile, hash: u32) -> Option<[f32; 4]> {
    match scalar(file, hash)? {
        Value::ColorRgba { r, g, b, a } => Some([r, g, b, a]),
        Value::Vector4(values) => Some(values),
        _ => None,
    }
}
