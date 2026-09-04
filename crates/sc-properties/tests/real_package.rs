//! 真实包属性表全量验证：解析 app.package 中全部 0x00B1B104 资源。
//! 数据文件不入库，缺失时跳过。

use std::collections::HashMap;
use std::time::Instant;

use dbpf::Package;
use sc_properties::{Kind, PropertyFile};

const REAL_PACKAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/app.package"
);

#[test]
fn real_package_parses_all_property_lists() {
    let Ok(package) = Package::open(REAL_PACKAGE) else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let prop_entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
        .collect();
    assert_eq!(prop_entries.len(), 249, "expected property list count");

    let t0 = Instant::now();
    let mut total_props = 0usize;
    let mut type_histogram: HashMap<&'static str, usize> = HashMap::new();

    for (i, e) in prop_entries.iter().enumerate() {
        let data = package.read(e).unwrap_or_else(|err| {
            panic!("prop #{i} {} failed to read: {err}", e.id);
        });
        let pf = PropertyFile::parse(&data).unwrap_or_else(|err| {
            panic!(
                "prop #{i} {} failed to parse ({} bytes): {err}",
                e.id,
                data.len()
            );
        });
        total_props += pf.values.len();
        for p in &pf.values {
            *type_histogram.entry(p.prop_type.name()).or_default() += 1;
        }
        // 每个属性文件应能打印（Display 不 panic）
        let _dump_len = format!("{pf}").len();
    }
    let secs = t0.elapsed().as_secs_f64();

    eprintln!(
        "parsed {} property files, {total_props} properties in {secs:.3}s",
        prop_entries.len()
    );
    let mut hist: Vec<_> = type_histogram.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (name, n) in &hist {
        eprintln!("  {name:10} {n}");
    }

    // 常见类型必然出现（SimCity 属性表的构成，HANDOFF §4 有记录）
    assert!(
        hist.iter().any(|(n, _)| *n == "Key"),
        "Key properties expected"
    );
    assert!(
        hist.iter().any(|(n, _)| *n == "float"),
        "float properties expected"
    );

    // 数组与非空标量至少各出现一次
    let has_array = prop_entries.iter().any(|e| {
        let data = package.read(e).unwrap();
        PropertyFile::parse(&data)
            .unwrap()
            .values
            .iter()
            .any(|p| matches!(p.kind, Kind::Array(_)))
    });
    assert!(has_array, "array properties expected");
}

/// Key 值的字段顺序验证：Key 引用的 type_id 应与包索引中真实存在的资源类型
/// 高度重合（若 instance/type/group 顺序错乱，type_id 会变成随机实例哈希，
/// 与索引类型几乎不可能重合）。
#[test]
fn real_package_keys_reference_real_types() {
    let Ok(package) = Package::open(REAL_PACKAGE) else {
        eprintln!("skipping: {REAL_PACKAGE} not present");
        return;
    };

    let index_types: std::collections::HashSet<u32> =
        package.entries().iter().map(|e| e.id.type_id).collect();

    let mut type_histogram: HashMap<u32, usize> = HashMap::new();
    let mut checked = 0usize;
    for e in package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
    {
        let data = package.read(e).unwrap();
        let pf = PropertyFile::parse(&data).unwrap();
        for p in &pf.values {
            for key in p.keys() {
                if key.type_id == 0 {
                    continue; // null/placeholder references are common and carry no type
                }
                *type_histogram.entry(key.type_id).or_default() += 1;
                checked += 1;
            }
        }
    }
    assert!(checked > 20, "expected a meaningful sample, got {checked}");

    let mut hist: Vec<_> = type_histogram.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (t, n) in hist.iter().take(6) {
        eprintln!("  key type {t:08X}: {n}");
    }

    // 绝大多数非空引用的 type_id 必须存在于包索引（其余为跨包引用）
    let total_known: usize = hist
        .iter()
        .map(|(t, n)| if index_types.contains(t) { *n } else { 0 })
        .sum();
    eprintln!(
        "{total_known}/{checked} non-null key references target types present in the package index"
    );
    assert!(
        total_known * 10 >= checked * 8,
        "expected >=80% of non-null key type ids to exist in the index, got {total_known}/{checked}"
    );
}
