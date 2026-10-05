//! P1 取证探针：EcoGame 脚本资源表解析（dev-dump 文档 §3 配方的离线验证）。
//!
//! 链路：扫包找 group 0x40800200 的属性列表（sim 主表）→ 0x0C947ABC(LocalKey)
//! → nonSim 主表 → 九张 KeyArray 子表 group 列表 → 枚举各 group 全部属性列表
//! （instance = resourceID）→ cGraphicsResource 两分支取模型 key。
//!
//! 验证案例：消防局 0x4DE9912B（prop=树/车、spawner=小人位置、path=进楼路线）。
//!
//! 用法：script_resources_probe <package> [extra...] --lot <lot_instance_hex> <lot_group_hex>

use dbpf::Package;
use sc_properties::{Kind, PropertyFile, ParseLimits, Value};
use sc_properties::PROPERTY_RESOURCE_TYPE;

const SIM_MASTER_GROUP: u32 = 0x4080_0200;
const ADDON_MASTER_GROUP: u32 = 0x4080_0201;
const NON_SIM_KEY: u32 = 0x0C94_7ABC;
const GROUP_UNITS: u32 = 0x07E0_AC00;
const GROUP_RESOURCES: u32 = 0x07E0_AC01;
const GROUP_PATHS: u32 = 0x0913_8114;
const GROUP_POINTS: u32 = 0x0913_8115;

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

fn key_of(v: &Value) -> Option<(u32, u32, u32)> {
    match v {
        Value::Key(k) => Some((k.instance, k.type_id, k.group)),
        _ => None,
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut lot: Option<(u32, u32)> = None;
    if let Some(pos) = args.iter().position(|a| a == "--lot") {
        let tail = args.split_off(pos);
        let inst = u32::from_str_radix(tail[1].trim_start_matches("0x"), 16).expect("lot instance");
        let grp = u32::from_str_radix(tail[2].trim_start_matches("0x"), 16).expect("lot group");
        lot = Some((inst, grp));
    }
    if args.is_empty() {
        eprintln!("usage: script_resources_probe <package> [extra...] [--lot <inst> <group>]");
        std::process::exit(2);
    }
    // --dump <inst>：跨包转储一条属性记录全文（先摘除标志再开包）
    let mut dump_inst: Option<u32> = None;
    if let Some(pos) = args.iter().position(|a| a == "--dump") {
        dump_inst = Some(
            u32::from_str_radix(args[pos + 1].trim_start_matches("0x"), 16).expect("hex"),
        );
        args.remove(pos);
        args.remove(pos);
    }
    let packages: Vec<Package> = args
        .iter()
        .map(|p| Package::open(p).expect("open package"))
        .collect();

    if let Some(inst) = dump_inst {
        for (pi, pkg) in packages.iter().enumerate() {
            for e in pkg.entries().iter().filter(|e| {
                e.id.instance == inst && e.id.type_id == PROPERTY_RESOURCE_TYPE
            }) {
                let data = pkg.read(e).unwrap();
                let f = PropertyFile::parse_with_limits(&data, ParseLimits::default())
                    .unwrap();
                println!(
                    "pkg{pi} I {inst:08X} T {:08X} G {:08X} ({} props)",
                    e.id.type_id,
                    e.id.group,
                    f.values.len()
                );
                for p in &f.values {
                    match &p.kind {
                        Kind::Scalar(v) => {
                            println!("  {:08X} {} {:?}", p.hash, p.prop_type.name(), v)
                        }
                        Kind::Array(vs) => {
                            println!("  {:08X} {} n={}", p.hash, p.prop_type.name(), vs.len());
                            for v in vs.iter().take(40) {
                                println!("    {v:?}");
                            }
                        }
                        Kind::Empty => println!("  {:08X} empty", p.hash),
                    }
                }
            }
        }
        return;
    }

    // 1. sim 主表：group 0x40800200 的属性列表
    let mut masters: Vec<(usize, dbpf::ResourceId, PropertyFile)> = Vec::new();
    for (pi, pkg) in packages.iter().enumerate() {
        for e in pkg.entries().iter().filter(|e| {
            e.id.type_id == PROPERTY_RESOURCE_TYPE
        }) {
            let Ok(data) = pkg.read(e) else { continue };
            let Ok(file) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
                continue;
            };
            let has_group_array = file.values.iter().any(|p| p.hash == GROUP_RESOURCES);
            let has_ns_key = file.values.iter().any(|p| p.hash == NON_SIM_KEY);
            if !has_group_array && !has_ns_key {
                continue;
            }
            println!(
                "主表候选: pkg{pi} T {:08X} G {:08X} I {:08X} ({} 属性, group数组={has_group_array}, nsKey={has_ns_key})",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                file.values.len()
            );
            masters.push((pi, e.id, file));
        }
    }
    if masters.is_empty() {
        eprintln!("未找到 group 0x40800200/0x40800201 的主表！");
        std::process::exit(1);
    }

    // 1.4 主表属性全览（找 nonSim LocalKey 线索）
    for (_, id, file) in &masters {
            }
    for (_, id, file) in masters.iter() {
        if id.instance != 0x622B_9CD7 {
            continue;
        }
        println!("== 主表 622B9CD7 Key/LocalKey 型属性:");
        for p in &file.values {
            let tn = p.prop_type.name().to_string();
            if tn.to_lowercase().contains("key") {
                println!("  {:08X} {} {:?}", p.hash, tn, p.kind);
            }
        }
        println!("== 属性类型分布:");
        use std::collections::BTreeMap;
        let mut dist: BTreeMap<String, usize> = BTreeMap::new();
        for p in &file.values {
            *dist.entry(p.prop_type.name().to_string()).or_default() += 1;
        }
        for (t, n) in dist {
            println!("  {t}: {n}");
        }
    }

    // 1.5 主表的九张 group 数组全览
    for (_, id, file) in &masters {
        for (hash, name) in [
            (0x07E0_AC00u32, "units"),
            (0x07E0_AC01, "resources"),
            (0x07E0_AC03, "maps"),
            (0x0913_8113, "zones"),
            (0x0913_8114, "paths"),
            (0x0913_8115, "points"),
            (0x0A30_A557, "sinks"),
            (0x0A30_A558, "transports"),
            (0x0A30_A559, "bundles"),
        ] {
            let ks: Vec<String> = values_of(file, hash)
                .iter()
                .filter_map(|v| key_of(v))
                .map(|(i, t, g)| format!("I{i:08X} G{g:08X}"))
                .collect();
            if !ks.is_empty() {
                println!("主表 {:08X} [{name}] = {}", id.instance, ks.join(", "));
            }
        }
    }

    // 2. nonSim 主表（0x0C947ABC LocalKey）
    let mut ns_keys: Vec<(u32, u32)> = Vec::new();
    for (_, id, file) in &masters {
        for v in values_of(file, NON_SIM_KEY) {
            if let Some((inst, _ty, grp)) = key_of(v) {
                println!("主表 {:08X} → nonSim 主表 key I {:08X} G {:08X}", id.instance, inst, grp);
                ns_keys.push((inst, grp));
            }
        }
    }
    let mut nonsim: Vec<(usize, dbpf::ResourceId, PropertyFile)> = Vec::new();
    for (inst, grp) in &ns_keys {
        for (pi, pkg) in packages.iter().enumerate() {
            for e in pkg.entries().iter().filter(|e| {
                e.id.type_id == PROPERTY_RESOURCE_TYPE
                    && e.id.instance == *inst
                    && e.id.group == *grp
            }) {
                let Ok(data) = pkg.read(e) else { continue };
                let Ok(file) =
                    PropertyFile::parse_with_limits(&data, ParseLimits::default())
                else {
                    continue;
                };
                println!("nonSim 主表: pkg{pi} I {inst:08X} G {grp:08X} ({} 属性)", file.values.len());
                nonsim.push((pi, e.id, file));
            }
        }
    }

    // 3. 九张子表之一先验 resources/units/paths 三张（本探针目标）
    let mut groups_wanted: Vec<(u32, u32, &str)> = Vec::new(); // (group, 来源, 表名)
    for (label, files) in [("sim", &masters), ("nonSim", &nonsim)] {
        for (_, id, file) in files {
            for (hash, name) in [
                (GROUP_RESOURCES, "resources"),
                (GROUP_UNITS, "units"),
                (GROUP_PATHS, "paths"),
                (GROUP_POINTS, "points"),
            ] {
                for v in values_of(file, hash) {
                    if let Some((inst, _ty, grp)) = key_of(v) {
                        groups_wanted.push((grp, inst, name));
                        if label == "sim" {
                                            println!("sim 主表 {:08X} 子表[{name}] group key I {inst:08X} G {grp:08X}", id.instance);
                        }
                    }
                }
            }
        }
    }

    // 4. 枚举各 group 的全部属性列表（表项；instance=resourceID）
    // group 语义二选一：Key.group 或 Key.instance——两个都试，命中多者胜
    for want_name in ["resources", "units", "paths"] {
        let wants: Vec<(u32, u32)> = groups_wanted
            .iter()
            .filter(|(_, _, n)| *n == want_name)
            .map(|(g, i, _)| (*g, *i))
            .collect();
        if wants.is_empty() {
            continue;
        }
        let mut by_group = 0usize;
        let mut by_instance = 0usize;
        for pkg in &packages {
            for e in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE) {
                if wants.iter().any(|(g, _)| *g == e.id.group) {
                    by_group += 1;
                }
                if wants.iter().any(|(_, i)| *i == e.id.instance) {
                    by_instance += 1;
                }
            }
        }
        println!(
            "[{want_name}] group 字段命中 {by_group} 条 / instance 字段命中 {by_instance} 条"
        );
    }

    // 5. 消防局 lot：prop/spawner/path id 反查
    if let Some((lot_inst, lot_grp)) = lot {
        let mut lot_file: Option<PropertyFile> = None;
        'outer: for pkg in &packages {
            for e in pkg.entries().iter().filter(|e| {
                e.id.type_id == PROPERTY_RESOURCE_TYPE
                    && e.id.instance == lot_inst
                    && e.id.group == lot_grp
            }) {
                let Ok(data) = pkg.read(e) else { continue };
                if let Ok(f) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) {
                    lot_file = Some(f);
                    break 'outer;
                }
            }
        }
        let Some(file) = lot_file else {
            eprintln!("lot 未找到 {lot_inst:08X}/{lot_grp:08X}");
            std::process::exit(1);
        };
        println!("== 消防局 lot {lot_inst:08X}/{lot_grp:08X}");

        // 4.5 资源表构建：group 40E1C100 的全部属性列表 → resourceID → 模型 key
        let res_groups: Vec<u32> = groups_wanted
            .iter()
            .filter(|(_, _, n)| *n == "resources")
            .map(|(g, _, _)| *g)
            .collect();
        let mut aliases: std::collections::HashMap<u32, (u32, String)> =
            std::collections::HashMap::new();
        let mut res_table: std::collections::HashMap<u32, (String, String)> =
            std::collections::HashMap::new();
        for pkg in &packages {
            for e in pkg.entries().iter().filter(|e| {
                e.id.type_id == PROPERTY_RESOURCE_TYPE
                    && res_groups.iter().any(|g| *g == e.id.group)
            }) {
                let Ok(data) = pkg.read(e) else { continue };
                let Ok(f) = PropertyFile::parse_with_limits(&data, ParseLimits::default())
                else {
                    println!("  资源表解析失败: I {:08X} G {:08X} ({} 字节)", e.id.instance, e.id.group, data.len());
                    continue;
                };
                let model = values_of(&f, 0x0D8C_29C3)
                    .first()
                    .and_then(|v| key_of(v))
                    .map(|(i, t, g)| format!("explicit I{i:08X} T{t:08X} G{g:08X}"));
                let markers = [0x00F9_EFBBu32, 0x0D89_7169, 0x0C36_D30D]
                    .iter()
                    .filter(|h| f.values.iter().any(|p| p.hash == **h))
                    .count();
                let model = model.unwrap_or_else(|| {
                    if markers > 0 {
                        format!("selfkey I{:08X} T{:08X} G{:08X} (markers={markers})", e.id.instance, e.id.type_id, e.id.group)
                    } else {
                        "无模型 key".to_string()
                    }
                });
                // 别名表：0x0E61772E = alias Key 数组（loader 把别名 id 也建进索引）
                for v in values_of(&f, 0x0E61_772E) {
                    if let Some((ai, at, ag)) = key_of(v) {
                        aliases.insert(ai, (e.id.instance, format!("T{at:08X} G{ag:08X}")));
                    }
                }
                // 名字候选：第一条 String8 属性
                let name = f
                    .values
                    .iter()
                    .filter_map(|p| match &p.kind {
                        Kind::Scalar(Value::String8(s)) => {
                            Some(format!("0x{:08X}={s}", p.hash))
                        }
                        Kind::Array(vs) => vs.first().and_then(|v| match v {
                            Value::String8(s) => Some(format!("0x{:08X}={s}", p.hash)),
                            _ => None,
                        }),
                        _ => None,
                    })
                    .next()
                    .unwrap_or_default();
                res_table.insert(e.id.instance, (model, name));
            }
        }
        println!("资源表条目: {} 条, 别名 {} 条", res_table.len(), aliases.len());
        // 4.7 EcoGame 脚本包直查（SimCityUserData/EcoGame）
        {
            let mut eco_ids: Vec<u32> = Vec::new();
            for bin in 0..14usize {
                for v in values_of(&file, 0x0C12_EF20 + bin as u32) {
                    if let Some((inst, _, _)) = key_of(v) {
                        eco_ids.push(inst);
                    }
                }
            }
            let eco_dirs = [r"D:\ea-games\SimCity\SimCityUserData\EcoGame"];
            for dir in eco_dirs {
                let Ok(rd) = std::fs::read_dir(dir) else { continue };
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("package") {
                        continue;
                    }
                    let Some(pkg) = Package::open(&path).ok() else { continue };
                    let mut hits = 0usize;
                    for e in pkg.entries().iter().filter(|e| {
                        e.id.type_id == PROPERTY_RESOURCE_TYPE
                            && eco_ids.contains(&e.id.instance)
                    }) {
                        hits += 1;
                        if hits > 40 {
                            continue;
                        }
                        let Ok(data) = pkg.read(e) else { continue };
                        let Ok(f) = PropertyFile::parse_with_limits(
                            &data,
                            ParseLimits::default(),
                        ) else {
                            continue;
                        };
                        let model = values_of(&f, 0x0D8C_29C3)
                            .first()
                            .and_then(|v| key_of(v))
                            .map(|(i, t, g)| {
                                format!("explicit I{i:08X} T{t:08X} G{g:08X}")
                            });
                        let markers = [0x00F9_EFBBu32, 0x0D89_7169, 0x0C36_D30D]
                            .iter()
                            .filter(|h| f.values.iter().any(|p| p.hash == **h))
                            .count();
                        let hashes: Vec<String> = f
                            .values
                            .iter()
                            .take(10)
                            .map(|p| format!("{:08X}", p.hash))
                            .collect();
                        println!(
                            "EcoGame I {:08X} -> G {:08X} ({} props) model={} markers={markers} hashes=[{}]",
                            e.id.instance,
                            e.id.group,
                            f.values.len(),
                            model.unwrap_or_else(|| "selfkey?".into()),
                            hashes.join(",")
                        );
                    }
                }
            }
        }

        // 4.8 最后一跳：模型 key（G 40E02D00 包装）→ LOD1(0x00F9EFBB) → RW4
        {
            let eco_ids: Vec<u32> = {
                let mut v = Vec::new();
                for bin in 0..14usize {
                    for x in values_of(&file, 0x0C12_EF20 + bin as u32) {
                        if let Some((inst, _, _)) = key_of(x) {
                            v.push(inst);
                        }
                    }
                }
                v
            };
            // 从 EcoGame 命中里收集显式模型 key
            let mut model_keys: Vec<(u32, u32)> = Vec::new();
            for dir in [r"D:\ea-games\SimCity\SimCityUserData\EcoGame"] {
                let Ok(rd) = std::fs::read_dir(dir) else { continue };
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("package") {
                        continue;
                    }
                    let Some(pkg) = Package::open(&path).ok() else { continue };
                    for e in pkg.entries().iter().filter(|e| {
                        e.id.type_id == PROPERTY_RESOURCE_TYPE
                            && e.id.group == 0x40E0_C100
                            && eco_ids.contains(&e.id.instance)
                    }) {
                        let Ok(data) = pkg.read(e) else { continue };
                        let Ok(f) = PropertyFile::parse_with_limits(
                            &data,
                            ParseLimits::default(),
                        ) else {
                            continue;
                        };
                        if let Some(v) = values_of(&f, 0x0D8C_29C3).first() {
                            if let Some((inst, _t, _g)) = key_of(v) {
                                model_keys.push((inst, 0x40E0_2D00));
                            }
                        }
                    }
                }
            }
            model_keys.sort();
            model_keys.dedup();
            // 在主包装包装记录 → LOD1 → RW4 实例存在性
            let main_pkg = &packages[0];
            for (inst, grp) in model_keys.iter().take(8) {
                let mut lod1: Option<u32> = None;
                for e in main_pkg.entries().iter().filter(|e| {
                    e.id.type_id == PROPERTY_RESOURCE_TYPE
                        && e.id.instance == *inst
                        && e.id.group == *grp
                }) {
                    let Ok(data) = main_pkg.read(e) else { continue };
                    let Ok(f) = PropertyFile::parse_with_limits(
                        &data,
                        ParseLimits::default(),
                    ) else {
                        continue;
                    };
                    lod1 = values_of(&f, 0x00F9_EFBB)
                        .first()
                        .and_then(|v| key_of(v))
                        .map(|(i, _, _)| i);
                }
                match lod1 {
                    Some(rw4_inst) => {
                        let found = packages.iter().any(|p| {
                            p.entries().iter().any(|e| {
                                e.id.instance == rw4_inst && e.id.type_id == 0x2F4E_681B
                            })
                        });
                        println!(
                            "wrapper I {inst:08X} -> LOD1 I {rw4_inst:08X} RW4({found})"
                        );
                    }
                    None => {
                        println!("wrapper I {inst:08X} -> LOD1 缺失，记录全文:");
                        for e in main_pkg.entries().iter().filter(|e| {
                            e.id.type_id == PROPERTY_RESOURCE_TYPE
                                && e.id.instance == *inst
                                && e.id.group == *grp
                        }) {
                            let Ok(data) = main_pkg.read(e) else { continue };
                            let Ok(f) = PropertyFile::parse_with_limits(
                                &data,
                                ParseLimits::default(),
                            ) else {
                                continue;
                            };
                            for p in &f.values {
                                println!("    {:08X} {} {:?}", p.hash, p.prop_type.name(), p.kind);
                            }
                        }
                    }
                }
            }
        }

        // sweep：收集全部 lot 的 bin id，与资源表实例求交集
        {
            use std::collections::HashSet;
            let res_ids: HashSet<u32> = res_table.keys().copied().collect();
            let mut all_bin_ids: HashSet<u32> = HashSet::default();
            let mut lots_scanned = 0usize;
            for pkg in &packages {
                for e in pkg.entries().iter().filter(|e| {
                    e.id.type_id == PROPERTY_RESOURCE_TYPE
                        && (e.id.group & 0xFFFF) == 0xC000
                }) {
                    let Ok(data) = pkg.read(e) else { continue };
                    let Ok(f) =
                        PropertyFile::parse_with_limits(&data, ParseLimits::default())
                    else {
                        continue;
                    };
                    lots_scanned += 1;
                    for bin in 0..14usize {
                        for v in values_of(&f, 0x0C12_EF20 + bin as u32) {
                            if let Some((inst, _, _)) = key_of(v) {
                                all_bin_ids.insert(inst);
                            }
                        }
                    }
                }
            }
            let hits = all_bin_ids.iter().filter(|i| res_ids.contains(i)).count();
            println!(
                "sweep: lots={lots_scanned} 不同 bin id={} 与资源表交集={} ({:.1}%)",
                all_bin_ids.len(),
                hits,
                if all_bin_ids.is_empty() {
                    0.0
                } else {
                    hits as f32 / all_bin_ids.len() as f32 * 100.0
                }
            );
            // 分类普查：96 个 bin id → 记录位置 + 模型可解性 + 包装形状
            {
                let mut b1_all: std::collections::HashMap<u32, Vec<(u32, u64)>> = Default::default();
                for (pi, pkg) in packages.iter().enumerate() {
                    for e in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE) {
                        b1_all.entry(e.id.instance).or_default().push((e.id.group, pi as u64));
                    }
                }
                let mut stats: std::collections::BTreeMap<String, usize> = Default::default();
                for id in &all_bin_ids {
                    let recs = b1_all.get(id);
                    let Some(recs) = recs else {
                        *stats.entry("phantom(无记录)".into()).or_default() += 1;
                        continue;
                    };
                    // 解析第一个记录看形状
                    let mut shape = "unknown".to_string();
                    'shape: for (grp, _) in recs {
                        for pkg in packages.iter() {
                            for e in pkg.entries().iter().filter(|e| {
                                e.id.type_id == PROPERTY_RESOURCE_TYPE
                                    && e.id.instance == *id
                                    && e.id.group == *grp
                            }) {
                                let Ok(data) = pkg.read(e) else { continue };
                                let Ok(f) = PropertyFile::parse_with_limits(
                                    &data,
                                    ParseLimits::default(),
                                ) else {
                                    continue;
                                };
                                let has = |h: u32| !values_of(&f, h).is_empty();
                                let explicit = values_of(&f, 0x0D8C_29C3).first().and_then(|v| key_of(v));
                                let vehicle = has(0x0D89_7169);
                                let marker = has(0x00F9_EFBB) || has(0x0C36_D30D);
                                let parent = values_of(&f, 0x00B2_CCCB).first().and_then(|v| key_of(v));
                                shape = if vehicle {
                                    "vehicle_models直出".into()
                                } else if explicit.is_some() {
                                    format!("显式key→{:08X}", explicit.unwrap().0)
                                } else if marker {
                                    "selfkey标记".into()
                                } else if parent.is_some() {
                                    format!("parent→{:08x}", parent.unwrap().0)
                                } else {
                                    format!("g{:08X}无形状", grp)
                                };
                                break 'shape;
                            }
                        }
                    }
                    // 组族
                    let g16 = recs.first().map(|(g, _)| g & 0xFFFF).unwrap_or(0);
                    let key = format!("{shape} @组低16={g16:04X}");
                    *stats.entry(key).or_default() += 1;
                }
                println!("== prop id 分类普查：");
                for (k, n) in &stats {
                    println!("  {n:>3} × {k}");
                }
                // 每类抽 2 个样本 id 供定性
                let mut samples: std::collections::BTreeMap<String, Vec<u32>> = Default::default();
                for id in &all_bin_ids {
                    let recs = b1_all.get(id);
                    let Some(recs) = recs else { continue };
                    let g16 = recs.first().map(|(g, _)| g & 0xFFFF).unwrap_or(0);
                    let (shape, has_mod) = {
                        let mut shape = "unknown".to_string();
                        let mut has_mod = false;
                        'shape2: for (grp, _) in recs {
                            for pkg in packages.iter() {
                                for e in pkg.entries().iter().filter(|e| {
                                    e.id.type_id == PROPERTY_RESOURCE_TYPE
                                        && e.id.instance == *id
                                        && e.id.group == *grp
                                }) {
                                    let Ok(data) = pkg.read(e) else { continue };
                                    let Ok(f) = PropertyFile::parse_with_limits(
                                        &data,
                                        ParseLimits::default(),
                                    ) else {
                                        continue;
                                    };
                                    let has = |h: u32| !values_of(&f, h).is_empty();
                                    has_mod = has(0x0D8C_29C3) || has(0x0D89_7169);
                                    shape = if has(0x0D89_7169) {
                                        "vehicle_models直出".into()
                                    } else if has(0x0D8C_29C3) {
                                        "显式key".into()
                                    } else if has(0x00F9_EFBB) || has(0x0C36_D30D) {
                                        "selfkey标记".into()
                                    } else {
                                        "无形状".into()
                                    };
                                    break 'shape2;
                                }
                            }
                        }
                        (shape, has_mod)
                    };
                    let key = format!("{shape} @组低16={g16:04X}");
                    samples.entry(key).or_default().push(*id);
                }
                for (k, ids) in samples {
                    let hexes: Vec<String> = ids.iter().take(3).map(|i| format!("{i:08X}")).collect();
                    println!("  样本[{k}]: {}", hexes.join(" "));
                }
            }

            // 96 个 bin id 的落点分布：全量 B1B104 索引 (instance → group 低16位)
            let mut where_lived: std::collections::HashMap<u32, usize> =
                std::collections::HashMap::new();
            let mut b1_index: std::collections::HashMap<u32, u32> =
                std::collections::HashMap::new();
            for pkg in &packages {
                for e in pkg.entries().iter().filter(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE) {
                    b1_index.entry(e.id.instance).or_insert(e.id.group);
                }
            }
            for id in &all_bin_ids {
                if let Some(g) = b1_index.get(id) {
                    *where_lived.entry(g & 0xFFFF).or_default() += 1;
                }
            }
            let mut dist: Vec<_> = where_lived.iter().collect();
            dist.sort_by(|a, b| b.1.cmp(a.1));
            for (g16, n) in dist {
                println!("  bin id 落点 group低16={g16:04X}: {n} 个");
            }
            let total_in_index = where_lived.values().sum::<usize>();
            println!(
                "  落点合计 {total_in_index}/{}（其余 = 非 property 记录或不存在）",
                all_bin_ids.len()
            );
            // 抽样 10 个未命中 id
            let missing: Vec<u32> = all_bin_ids
                .iter()
                .filter(|i| !res_ids.contains(i))
                .copied()
                .take(10)
                .collect();
            println!(
                "未命中抽样: {:?}",
                missing.iter().map(|i| format!("{i:08X}")).collect::<Vec<_>>()
            );
        }

        // 全部条目：instance → group/model/name
        {
            let mut items: Vec<_> = res_table.iter().collect();
            items.sort_by_key(|(k, _)| **k);
            for (inst, (model, name)) in items.iter() {
                println!("  res I {inst:08X} {model} {name}");
            }
        }
        // 92BEE95F 的 self-key RW4 是否存在
        {
            let want = 0x92BE_E95Fu32;
            for (pi, pkg) in packages.iter().enumerate() {
                for e in pkg.entries().iter().filter(|e| e.id.instance == want && e.id.type_id != PROPERTY_RESOURCE_TYPE) {
                    println!(
                        "selfkey 模型验证: pkg{pi} I {want:08X} T {:08X} G {:08X} ({} 字节)",
                        e.id.type_id,
                        e.id.group,
                        e.decompressed_size
                    );
                }
            }
        }
        {
            let probe_ids = [
                0x0AB4DFE1u32, 0x0AB4DFE2, 0x0AB4DFE3, 0xBF2C1022, 0x54CA89F0,
                0x92BEE95F, 0x14984C6B, 0x14984C6A, 0x14984C69, 0x14984C68,
            ];

            // addon 资源组直查（各 master 的 resources group）
            let mut addon_groups: Vec<u32> = groups_wanted
                .iter()
                .filter(|(_, _, n)| *n == "resources")
                .map(|(g, _, _)| *g)
                .collect();
            addon_groups.sort();
            addon_groups.dedup();
            println!("全部资源组: {:?}", addon_groups.iter().map(|g| format!("{g:08X}")).collect::<Vec<_>>());
            for id in &probe_ids {
                for pkg in &packages {
                    for e in pkg.entries().iter().filter(|e| {
                        e.id.instance == *id
                            && addon_groups.iter().any(|g| *g == e.id.group)
                    }) {
                        println!(
                            "  addon 资源组命中 I {id:08X} → T {:08X} G {:08X}",
                            e.id.type_id, e.id.group
                        );
                    }
                }
            }
            for id in &probe_ids {
                if let Some((target, ctx)) = aliases.get(id) {
                    println!("  别名命中 I {id:08X} → 资源 I {target:08X} ({ctx})");
                }
            }
        }

        // 4.6 prop id 按 instance 全包直查（绕过 group 簿记）
        let mut prop_ids: Vec<u32> = Vec::new();
        for bin in 0..14usize {
            for v in values_of(&file, 0x0C12_EF20 + bin as u32) {
                if let Some((inst, _, _)) = key_of(v) {
                    prop_ids.push(inst);
                }
            }
        }
        println!("-- prop id 全包直查:");
        for id in &prop_ids {
            let mut hits = 0usize;
            for pkg in &packages {
                for e in pkg.entries().iter().filter(|e| e.id.instance == *id) {
                    hits += 1;
                    if hits > 8 {
                        continue;
                    }
                    let Ok(data) = pkg.read(e) else { continue };
                    let Ok(f) =
                        PropertyFile::parse_with_limits(&data, ParseLimits::default())
                    else {
                        continue;
                    };
                    let model = values_of(&f, 0x0D8C_29C3)
                        .first()
                        .and_then(|v| key_of(v))
                        .map(|(i, t, g)| format!("explicit I{i:08X} T{t:08X} G{g:08X}"));
                    let markers = [0x00F9_EFBBu32, 0x0D89_7169, 0x0C36_D30D]
                        .iter()
                        .filter(|h| f.values.iter().any(|p| p.hash == **h))
                        .count();
                    let hashes: Vec<String> =
                        f.values.iter().take(8).map(|p| format!("{:08X}", p.hash)).collect();
                    println!(
                        "  I {id:08X} -> T {:08X} G {:08X} ({} props) model={} markers={markers} hashes=[{}]",
                        e.id.type_id,
                        e.id.group,
                        f.values.len(),
                        model.unwrap_or_else(|| "none".into()),
                        hashes.join(",")
                    );
                }
            }
            if hits == 0 {
                // 邻域排查：±0x20 内有无任何记录（字节序/偏移问题可视化）
                let mut near = 0usize;
                for pkg in &packages {
                    for e in pkg.entries().iter() {
                        let d = (e.id.instance as i64 - *id as i64).abs();
                        if d <= 0x20 {
                            near += 1;
                            if near <= 4 {
                                println!(
                                    "    邻域: I {:08X} T {:08X} G {:08X} (距 {d})",
                                    e.id.instance, e.id.type_id, e.id.group
                                );
                            }
                        }
                    }
                }
                println!("  I {id:08X} -> MISS 全包无记录 (邻域命中 {near})");
            }
        }
        for bin in 0..14usize {
            let ids = values_of(&file, 0x0C12_EF20 + bin as u32);
            if ids.is_empty() {
                continue;
            }
            for v in &ids {
                if let Some((inst, ty, grp)) = key_of(v) {
                    let hit = res_table.get(&inst);
                    println!(
                        "  prop bin{bin}: id I {inst:08X} T {ty:08X} G {grp:08X} → {}",
                        hit.map(|(m, n)| format!("{m} {n}")).unwrap_or_else(|| "✗ 不在资源表".into())
                    );
                }
            }
        }
        for v in values_of(&file, 0x0E1B_AC61) {
            if let Some((inst, ty, grp)) = key_of(v) {
                println!("  spawner: id I {inst:08X} T {ty:08X} G {grp:08X}");
            }
        }
        for v in values_of(&file, 0x0CC8_FE7A) {
            if let Some((inst, ty, grp)) = key_of(v) {
                println!("  path entry: id I {inst:08X} T {ty:08X} G {grp:08X}");
            }
        }
    }
}
