//! 探针：property 语义子类型普查（排除 Unit 0xC000 后的特殊语义家族扫描）。
//!
//! 按 InstanceType（group & 0xFFFF）聚类全部 0x00B1B104 资源，输出：
//!   A. 子类型分布总表（数量 / 注册表命名率 / 跨包组号 / 引用画像 / Top 特征键）
//!   B. 非 Unit 子类型的结构特征（Top 特征键 + 注册表属性名 + 值类型 + 覆盖率）
//!   C. 名称词根家族（road/brush/map/truck/…）跨子类型分布
//!   D. 关注子类型的完整样品 dump（Map/Agent/Path/Network/Descriptor…）
//!
//! 用法：cargo run -p sc-properties --release --example property_semantics_survey -- \
//!   <database_main.s3db> <out_report> <package...>
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use sc_registry::Registry;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const TYPE_MODEL: u32 = 0x2F4E_681B;
const TYPE_RASTER: u32 = 0x2F4E_681C;

/// SCP InstanceTypeIconConverter 已知子类型的可读名。
fn subtype_label(it: u16) -> &'static str {
    match it {
        0xC000 => "Unit",
        0xC600 => "Agent",
        0x8B7E => "Path",
        0xC400 => "Network",
        0xC900 => "Menu2",
        0x8A01 => "Menu",
        0xE000 => "Map",
        0x2043 => "Descriptor",
        0xB185 => "DecalAtlas1",
        0x1651 => "DecalAtlas2",
        0x1652 => "DecalAtlas3",
        _ => "?",
    }
}

/// 名称词根扫描表（小写子串匹配）。
const NAME_TOKENS: &[&str] = &[
    "road", "brush", "map", "truck", "agent", "vehicle", "train", "plane", "ship", "boat",
    "sim", "pedestrian", "path", "network", "zone", "terrain", "water", "tree", "sand",
    "mud", "cliff", "rock", "grass", "building", "house", "park", "landmark", "bus",
    "fire", "police", "health", "power", "sewage", "garbage", "miner", "oil", "river",
    "bridge", "tunnel", "rail", "street", "avenue", "dirt", "curve",
];

#[derive(Default)]
struct Cluster {
    count: u64,
    parse_fail: u64,
    groups: BTreeSet<u32>,
    named: u64,
    names: Vec<(u32, String)>,
    /// hash -> 出现次数
    key_cov: BTreeMap<u32, u64>,
    /// hash -> (主要值类型名, 计数)
    key_type: BTreeMap<u32, (String, u64)>,
    /// hash -> 样例值摘要
    key_sample: BTreeMap<u32, String>,
    ref_model: u64,
    ref_raster: u64,
    ref_property: u64,
    ref_key_zero_type: u64,
    has_text: u64,
    has_transform: u64,
    has_color: u64,
    dumped: u32,
}

fn value_summary(v: &Value) -> String {
    match v {
        Value::Key(k) => format!("key(T={:08X} G={:08X} I={:08X})", k.type_id, k.group, k.instance),
        Value::Bool(b) => format!("bool({b})"),
        Value::Int32(n) => format!("i32({n})"),
        Value::UInt32(n) => format!("u32({n})"),
        Value::Float(f) => format!("f32({f:.3})"),
        Value::Text(t) => format!("text(inst={:08X})", t.instance_id),
        Value::String8(s) | Value::String16(s) => {
            let head: String = s.chars().take(24).collect();
            format!("str({head})")
        }
        Value::ColorRgb { .. } => "colorRgb".into(),
        Value::ColorRgba { .. } => "colorRgba".into(),
        Value::Vector2(v) => format!("vec2({:.2},{:.2})", v[0], v[1]),
        Value::Vector3(v) => format!("vec3({:.2},{:.2},{:.2})", v[0], v[1], v[2]),
        Value::Vector4(v) => format!("vec4({:.2},{:.2},{:.2},{:.2})", v[0], v[1], v[2], v[3]),
        Value::BoundingBox { .. } => "bbox".into(),
        Value::Transform(_) => "transform".into(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: property_semantics_survey <s3db> <out_report> <package...>");
        std::process::exit(2);
    }
    let registry = Registry::open(&args[1]).expect("open s3db");
    let mut out = std::io::BufWriter::new(
        std::fs::File::create(&args[2]).expect("create report"),
    );

    let mut clusters: BTreeMap<u16, Cluster> = BTreeMap::new();
    // 词根家族：token -> (覆盖数, 样例 [(instancetype, name)])
    let mut token_families: BTreeMap<String, (u64, Vec<(u16, String)>)> = BTreeMap::new();

    for pkg_path in &args[3..] {
        let package = Package::open(pkg_path).unwrap_or_else(|e| panic!("open {pkg_path}: {e:?}"));
        let pkg_name = std::path::Path::new(pkg_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(pkg_path)
            .to_string();
        let _ = writeln!(out, "\n===== package: {pkg_name} =====");
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let it = (entry.id.group & 0xFFFF) as u16;
            let c = clusters.entry(it).or_default();
            c.count += 1;
            c.groups.insert(entry.id.group & 0xFFFF_0000);

            let name = registry.instance_name(entry.id.instance);
            let named = !name.starts_with("0x");
            if named {
                c.named += 1;
                if c.names.len() < 40 {
                    c.names.push((entry.id.instance, name.clone()));
                }
                let lower = name.to_lowercase();
                for tok in NAME_TOKENS {
                    if lower.contains(tok) {
                        let fam = token_families.entry(tok.to_string()).or_default();
                        fam.0 += 1;
                        if fam.1.len() < 12 && !fam.1.iter().any(|(_, n)| n == &name) {
                            fam.1.push((it, name.clone()));
                        }
                    }
                }
            }

            let Ok(data) = package.read(entry) else {
                c.parse_fail += 1;
                continue;
            };
            let Ok(file) = PropertyFile::parse(&data) else {
                c.parse_fail += 1;
                continue;
            };

            let mut seen_this_file: BTreeSet<u32> = BTreeSet::default();
            let mut has_text = false;
            let mut has_transform = false;
            let mut has_color = false;
            for p in &file.values {
                if seen_this_file.insert(p.hash) {
                    *c.key_cov.entry(p.hash).or_default() += 1;
                }
                let tn = p.prop_type.name().to_string();
                let entry_type = c.key_type.entry(p.hash).or_insert((tn.clone(), 0));
                entry_type.1 += 1;
                c.key_sample.entry(p.hash).or_insert_with(|| match &p.kind {
                    Kind::Scalar(v) => value_summary(v),
                    Kind::Array(vals) => {
                        let head: Vec<String> = vals.iter().take(2).map(value_summary).collect();
                        format!("array[{}] {}", vals.len(), head.join("; "))
                    }
                    Kind::Empty => "empty".into(),
                });
                // 引用画像（数组内的 key 也计）
                let mut scan = |v: &Value| match v {
                    Value::Key(k) => match k.type_id {
                        TYPE_MODEL => c.ref_model += 1,
                        TYPE_RASTER => c.ref_raster += 1,
                        PROPERTY_TYPE => c.ref_property += 1,
                        0 => c.ref_key_zero_type += 1,
                        _ => {}
                    },
                    Value::Text(_) => has_text = true,
                    Value::Transform(_) => has_transform = true,
                    Value::ColorRgb { .. } | Value::ColorRgba { .. } => has_color = true,
                    _ => {}
                };
                match &p.kind {
                    Kind::Scalar(v) => scan(v),
                    Kind::Array(vals) => vals.iter().for_each(scan),
                    Kind::Empty => {}
                }
            }
            if has_text {
                c.has_text += 1;
            }
            if has_transform {
                c.has_transform += 1;
            }
            if has_color {
                c.has_color += 1;
            }

            // 关注子类型的完整样品 dump（每簇 2 个）
            if matches!(it, 0xE000 | 0xC600 | 0x8B7E | 0xC400 | 0x2043)
                && c.dumped < 2
                && named
            {
                c.dumped += 1;
                let _ = writeln!(
                    out,
                    "\n--- sample 0x{:04X} inst 0x{:08X} group 0x{:08X} ({} props) [{}]",
                    it,
                    entry.id.instance,
                    entry.id.group,
                    file.values.len(),
                    pkg_name
                );
                let mut v: Vec<_> = file.values.iter().collect();
                v.sort_by_key(|p| p.hash);
                for p in v {
                    let pname = registry.property_name(p.hash);
                    let summary = match &p.kind {
                        Kind::Scalar(val) => value_summary(val),
                        Kind::Array(vals) => {
                            let head: Vec<String> = vals.iter().take(4).map(value_summary).collect();
                            format!("array[{}] {}", vals.len(), head.join("; "))
                        }
                        Kind::Empty => "empty".into(),
                    };
                    let _ = writeln!(out, "    0x{:08X} {:<10} {}: {summary}", p.hash, p.prop_type.name(), pname);
                }
            }
        }
    }

    // ------------------------------------------------ A. 分布总表
    let _ = writeln!(out, "\n\n########## A. InstanceType 分布总表 ##########");
    let _ = writeln!(out, "{:<8} {:<12} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}  {}",
        "IType", "label", "count", "named%", "grpH", "mdl", "raster", "prop", "text", "xform", "top keys");
    for (it, c) in &clusters {
        let named_pct = if c.count > 0 { c.named * 100 / c.count } else { 0 };
        let mut top: Vec<_> = c.key_cov.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top_keys: Vec<String> = top
            .iter()
            .take(3)
            .map(|(h, n)| format!("{h:08X}({n})"))
            .collect();
        let _ = writeln!(out,
            "0x{it:04X}   {:<12} {:>7} {:>6}% {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}  {}",
            subtype_label(*it), c.count, named_pct, c.groups.len(),
            c.ref_model, c.ref_raster, c.ref_property, c.has_text, c.has_transform,
            top_keys.join(" "));
    }

    // ------------------------------------------------ B. 非 Unit 子类型结构特征
    let _ = writeln!(out, "\n########## B. 非 Unit 子类型结构特征 ##########");
    for (it, c) in &clusters {
        if *it == 0xC000 || c.count == 0 {
            continue;
        }
        let _ = writeln!(out,
            "\n=== 0x{it:04X} {} — {} 个（命名率 {:.0}%，parse_fail {}，组高 16 位变体 {} 种）",
            subtype_label(*it), c.count,
            if c.count > 0 { c.named * 100 / c.count } else { 0 },
            c.parse_fail, c.groups.len());
        let _ = writeln!(out, "  名称样例:");
        for (inst, name) in c.names.iter().take(14) {
            let _ = writeln!(out, "    0x{inst:08X}  {name}");
        }
        let mut top: Vec<_> = c.key_cov.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let _ = writeln!(out, "  特征键（覆盖率 = 出现文件数 / 簇内总数）：");
        for (h, n) in top.iter().take(14) {
            let cov = (**n) as f64 / c.count as f64 * 100.0;
            let (tn, _) = c.key_type.get(h).cloned().unwrap_or(("?".into(), 0));
            let sample = c.key_sample.get(h).cloned().unwrap_or_default();
            let sample = if sample.len() > 60 { sample[..60].to_string() } else { sample };
            let _ = writeln!(out,
                "    0x{h:08X} {:<9} {:>5.1}%  {:<28} {}",
                tn, cov, registry.property_name(**h), sample);
        }
    }

    // ------------------------------------------------ C. 名称词根家族
    let _ = writeln!(out, "\n########## C. 名称词根家族（非全部 property，含 Unit 命名） ##########");
    let mut fams: Vec<_> = token_families.iter().collect();
    fams.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    for (tok, (n, samples)) in fams {
        let n = *n;
        let by_type: BTreeMap<u16, usize> = samples.iter().fold(BTreeMap::new(), |mut m, (it, _)| {
            *m.entry(*it).or_default() += 1;
            m
        });
        let types: Vec<String> = by_type
            .iter()
            .map(|(it, _)| format!("0x{it:04X}({})", subtype_label(*it)))
            .collect();
        let _ = writeln!(out, "\n  *\"{tok}\"* — {n} 处命名命中，样例子类型: {}", types.join(", "));
        for (it, name) in samples.iter().take(8) {
            let _ = writeln!(out, "      [0x{it:04X}] {name}");
        }
    }

    let _ = writeln!(out, "\n（完）");
    println!("报告已写出: {}", args[2]);
}
