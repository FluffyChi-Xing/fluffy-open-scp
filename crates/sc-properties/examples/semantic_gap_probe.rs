//! 探针：语义 tag 漏检面普查。
//!
//! 1. 完整 dump 三个用户报告的 TGI（Action Title / unit 副本 / Alert）。
//! 2. 全量扫描 0x00B1B104：group 低 16 位落在 Other 桶的资源，按
//!    「Parent 指向家族 / 特征键」聚类，找随机 group 复制导致的漏检家族。
//! 3. 统计 E8D0CAFA 型同实例多 group 复制的分布。
//!
//! 用法：cargo run -p sc-properties --release --example semantic_gap_probe -- \
//!   <database_main.s3db> <out_report> <package...>
use dbpf::Package;
use sc_properties::locale::Locale;
use sc_properties::{Kind, PropertyFile, Value};
use sc_registry::Registry;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const TYPE_MODEL: u32 = 0x2F4E_681B;
const TYPE_LOCALE_TABLE: u32 = 0x0A98_EAF0;

/// 三个用户报告案例（T:G:I）。
const FOCUS_TGIS: &[(u32, u32, u32)] = &[
    (0x00B1_B104, 0x40E1_C500, 0x9D57_A3A7), // Action Title text[20]
    (0x00B1_B104, 0x40B7_A83D, 0xE8D0_CAFA), // unit 副本（随机 group）
    (0x00B1_B104, 0x098A_44F2, 0x7FA4_99AA), // AlertPowerCriticalOilCrude
];

/// semantic.rs 低 16 位表已覆盖的子类型（探针独立镜像，用于分桶）。
fn known_low16(low: u16) -> bool {
    matches!(
        low,
        0xC000 | 0xC600 | 0x2D00 | 0xC100 | 0xE800 | 0xC500 | 0xC400 | 0x8B7E | 0xE000
            | 0x44F2 | 0xC300 | 0x8A01 | 0xC900 | 0xBA03 | 0xEB00 | 0xE900 | 0x2043
            | 0xB185 | 0x1651 | 0x1652 | 0x0000
    )
}

/// Parent 键（0x00B2CCCB）指向的家族（取目标 group 低 16 位）。
const KEY_PARENT: u32 = 0x00B2_CCCB;
/// 特征键：Model Bounding Box。
const KEY_BBOX: u32 = 0x00F9_EFBA;
/// 特征键：LOD1。
const KEY_LOD1: u32 = 0x00F9_EFBB;
/// 特征键：画笔 heightmap 名。
const KEY_BRUSH_NAME: u32 = 0x0DBA_3A9C;
/// 特征键：画笔分辨率。
const KEY_BRUSH_RES: u32 = 0x0DC0_97E3;

fn key_hashes(file: &PropertyFile) -> BTreeSet<u32> {
    file.values.iter().map(|p| p.hash).collect()
}

fn parent_group(file: &PropertyFile) -> Option<u32> {
    file.values.iter().find_map(|p| {
        if p.hash != KEY_PARENT {
            return None;
        }
        let mut found = None;
        let mut scan = |v: &Value| {
            if let Value::Key(k) = v {
                if found.is_none() {
                    found = Some(k.group);
                }
            }
        };
        match &p.kind {
            Kind::Scalar(v) => scan(v),
            Kind::Array(vals) => {
                for val in vals {
                    scan(val);
                }
            }
            Kind::Empty => {}
        }
        found
    })
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
            let head: String = s.chars().take(40).collect();
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

#[derive(Default)]
struct OtherRow {
    count: u64,
    /// Parent 指向的家族低 16 位 -> 计数
    parent_family: BTreeMap<u16, u64>,
    no_parent: u64,
    has_bbox: u64,
    has_lod1: u64,
    has_brush: u64,
    ref_model: u64,
    /// 同实例出现过的 group 数分布的累计观察（instance -> groups）
    instance_groups: BTreeMap<u32, BTreeSet<u32>>,
    /// (instance, 命名) 样例，按 parent 家族留 6 个
    samples: BTreeMap<u16, Vec<(u32, String, u32)>>,
    /// 无 Parent 样例
    orphan_samples: Vec<(u32, String, u32, Vec<String>)>,
}

/// 额外辨识样品：按 (group 精确匹配 或 实例) 抓第一个命中做完整 dump。
const EXTRA_SAMPLES: &[(u32, u32, &str)] = &[
    (0x61F0_EC00, 0, "0xEC00 簇(kCategoryID*)"),
    (0x4020_0100, 0, "0x0100 簇(Parent→0x0000)"),
    (0x4047_0102, 0, "0x0102 簇(Settings UI)"),
    (0, 0x494F_8678, "environment ×181"),
    (0x97B9_9ADA, 0, "decal atlas 副本(随机 group)"),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: semantic_gap_probe <s3db> <out_report> <package...> [--locale <package>]");
        std::process::exit(2);
    }
    let registry = Registry::open(&args[1]).expect("open s3db");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[2]).expect("create report"));

    // 拆分 --locale <package> 与常规包列表。
    let mut pkg_paths: Vec<&String> = Vec::new();
    let mut locale_path: Option<&String> = None;
    let mut rest = &args[3..];
    while let Some(p) = rest.first() {
        if p == "--locale" {
            locale_path = Some(rest.get(1).expect("--locale 需要包路径"));
            rest = &rest[2..];
            continue;
        }
        pkg_paths.push(p);
        rest = &rest[1..];
    }

    // 加载 en-us 字符串表用于解析 text 引用。
    let locale = locale_path
        .map(|path| {
            let package = Package::open(path).expect("open locale package");
            let resources = package.entries().iter().filter_map(|entry| {
                if entry.id.type_id != TYPE_LOCALE_TABLE {
                    return None;
                }
                let data = package.read(entry).ok()?;
                Some((entry.id.instance, data))
            });
            Locale::from_resources(resources).expect("parse locale")
        })
        .unwrap_or_default();
    if locale.string_count() > 0 {
        eprintln!(
            "locale 表 {} 张 / 字符串 {} 条",
            locale.table_count(),
            locale.string_count()
        );
    }

    // text 值带本地化解析的摘要。
    let text_summary = |t: &sc_properties::Text| -> String {
        let base = format!("text(inst={:08X})", t.instance_id);
        match locale.get(t.table_id, t.instance_id) {
            Some(s) => format!("{base}「{}」", s.chars().take(48).collect::<String>()),
            None => base,
        }
    };

    let mut other = OtherRow::default();
    let mut total_property: u64 = 0;
    let mut tagged_known: u64 = 0;

    for pkg_path in &pkg_paths {
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
            total_property += 1;
            let low = (entry.id.group & 0xFFFF) as u16;
            let brush_group = known_low16(low);

            // ---- 焦点 / 辨识样品 dump（不论桶） ----
            let focus = FOCUS_TGIS.iter().find(|(_, g, i)| {
                *g == entry.id.group && *i == entry.id.instance
            });
            let extra = EXTRA_SAMPLES.iter().find(|(g, i, _)| {
                (*g == 0 && *i == entry.id.instance) || (*g != 0 && *g == entry.id.group)
            });
            if focus.is_some() || extra.is_some() {
                let label = match (focus, extra) {
                    (Some(_), _) => "焦点".to_string(),
                    (_, Some((_, _, name))) => format!("样品[{name}]"),
                    _ => unreachable!(),
                };
                let data = match package.read(entry) {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = writeln!(out, "{label} {:08X} 读取失败: {e:?}", entry.id.instance);
                        continue;
                    }
                };
                let _ = writeln!(
                    out,
                    "\n--- {label} T={:08X} G={:08X} I={:08X} ({} props) [{}] 实名: {}",
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    PropertyFile::parse(&data).map(|f| f.values.len()).unwrap_or(0),
                    pkg_name,
                    registry.instance_name(entry.id.instance),
                );
                if let Ok(file) = PropertyFile::parse(&data) {
                    let tag = sc_properties::property_semantic(&file, entry.id.group);
                    let _ = writeln!(
                        out,
                        "    >>> property_semantic = {:?} / {} / {}",
                        tag,
                        tag.label_zh(),
                        tag.label_en()
                    );
                    let mut v: Vec<_> = file.values.iter().collect();
                    v.sort_by_key(|p| p.hash);
                    for p in v {
                        let pname = registry.property_name(p.hash);
                        let summary = match &p.kind {
                            Kind::Scalar(Value::Text(t)) => text_summary(t),
                            Kind::Scalar(val) => value_summary(val),
                            Kind::Array(vals) => {
                                let head: Vec<String> = vals
                                    .iter()
                                    .take(24)
                                    .map(|val| match val {
                                        Value::Text(t) => text_summary(t),
                                        other => value_summary(other),
                                    })
                                    .collect();
                                format!("array[{}] {}", vals.len(), head.join("; "))
                            }
                            Kind::Empty => "empty".into(),
                        };
                        let _ = writeln!(
                            out,
                            "    0x{:08X} {:<10} {}: {summary}",
                            p.hash,
                            p.prop_type.name(),
                            pname
                        );
                    }
                }
            }

            if brush_group {
                tagged_known += 1;
                continue;
            }

            // ---- Other 桶画像 ----
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            let hashes = key_hashes(&file);
            other.count += 1;
            other.instance_groups
                .entry(entry.id.instance)
                .or_default()
                .insert(entry.id.group);
            if hashes.contains(&KEY_BBOX) {
                other.has_bbox += 1;
            }
            if hashes.contains(&KEY_LOD1) {
                other.has_lod1 += 1;
            }
            if hashes.contains(&KEY_BRUSH_NAME) && hashes.contains(&KEY_BRUSH_RES) {
                other.has_brush += 1;
            }
            let has_model_ref = file.values.iter().any(|p| {
                let mut hit = false;
                let mut scan = |v: &Value| {
                    if let Value::Key(k) = v {
                        if k.type_id == TYPE_MODEL {
                            hit = true;
                        }
                    }
                };
                match &p.kind {
                    Kind::Scalar(v) => scan(v),
                    Kind::Array(vals) => {
                        for val in vals {
                            scan(val);
                        }
                    }
                    Kind::Empty => {}
                }
                hit
            });
            if has_model_ref {
                other.ref_model += 1;
            }
            let name = registry.instance_name(entry.id.instance);
            let named = !name.starts_with("0x");
            match parent_group(&file) {
                Some(g) => {
                    let fam = (g & 0xFFFF) as u16;
                    *other.parent_family.entry(fam).or_default() += 1;
                    if named {
                        let s = other.samples.entry(fam).or_default();
                        if s.len() < 6 && !s.iter().any(|(i, _, _)| *i == entry.id.instance) {
                            s.push((entry.id.instance, name, entry.id.group));
                        }
                    }
                }
                None => {
                    other.no_parent += 1;
                    if named && other.orphan_samples.len() < 25 {
                        let keylist: Vec<String> = hashes
                            .iter()
                            .take(12)
                            .map(|h| format!("{h:08X}"))
                            .collect();
                        other
                            .orphan_samples
                            .push((entry.id.instance, name, entry.id.group, keylist));
                    }
                }
            }
        }
    }

    // ------------------------------------------------ 汇总
    let _ = writeln!(out, "\n\n########## 漏检面汇总 ##########");
    let _ = writeln!(
        out,
        "property 总数 {total_property}，低16表已覆盖 {tagged_known}，Other 桶 {}",
        other.count
    );
    let _ = writeln!(out, "\n## Other 桶 Parent 家族分布（top 20）：");
    let mut fams: Vec<_> = other.parent_family.iter().collect();
    fams.sort_by(|a, b| b.1.cmp(a.1));
    for (fam, n) in fams.iter().take(20) {
        let _ = writeln!(out, "  Parent→0x{fam:04X}: {n}");
        if let Some(samples) = other.samples.get(fam) {
            for (inst, name, group) in samples {
                let _ = writeln!(out, "      0x{inst:08X} G={group:08X}  {name}");
            }
        }
    }
    let _ = writeln!(
        out,
        "\n## Other 桶特征：has_bbox {} has_lod1 {} has_brush {} ref_model {} no_parent {}",
        other.has_bbox, other.has_lod1, other.has_brush, other.ref_model, other.no_parent
    );
    let _ = writeln!(out, "\n## 同实例多 group 复制（Other 桶，group 数 ≥3 的实例，top 30）：");
    let mut multi: Vec<_> = other
        .instance_groups
        .iter()
        .filter(|(_, gs)| gs.len() >= 3)
        .collect();
    multi.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (inst, gs) in multi.iter().take(30) {
        let name = registry.instance_name(**inst);
        let head: Vec<String> = gs.iter().take(6).map(|g| format!("{g:08X}")).collect();
        let _ = writeln!(
            out,
            "  0x{inst:08X} ×{}  {name}  groups: {}…",
            gs.len(),
            head.join(",")
        );
    }
    let _ = writeln!(out, "\n## Other 桶无 Parent 命名样例（≤25）：");
    for (inst, name, group, keys) in &other.orphan_samples {
        let _ = writeln!(
            out,
            "  0x{inst:08X} G={group:08X}  {name}  keys: {}",
            keys.join(",")
        );
    }

    let _ = writeln!(out, "\n（完）");
    println!("报告已写出: {}", args[2]);
}
