//! 真实包（app.package）属性全量转储验证：协议不变量 + 与 registry 的名称解析。
//! 数据文件不入库，缺失时跳过。

use std::collections::HashSet;

use dbpf::Package;
use sc_exporter::prop_json::{PROP_TYPE_ID, dump_resource, fallback_name};
use sc_registry::Registry;

const PACKAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/app.package"
);
const REGISTRY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/database_main.s3db"
);

#[test]
fn real_package_dumps_follow_protocol_invariants() {
    let Ok(package) = Package::open(PACKAGE) else {
        eprintln!("skipping: {PACKAGE} not present");
        return;
    };
    let registry = if std::path::Path::new(REGISTRY).exists() {
        Some(Registry::open(REGISTRY).unwrap())
    } else {
        eprintln!("note: registry missing, name fields stay null");
        None
    };

    let entries: Vec<_> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == PROP_TYPE_ID)
        .collect();
    assert_eq!(entries.len(), 249, "expected property resource count");

    let t0 = std::time::Instant::now();
    let mut total_properties = 0usize;
    let mut named = 0usize;
    let mut type_histogram: std::collections::HashMap<&'static str, usize> = Default::default();

    for (i, entry) in entries.iter().enumerate() {
        let dump = dump_resource(&package, entry, registry.as_ref())
            .unwrap_or_else(|e| panic!("prop #{i} {} failed: {e}", entry.id));

        // 协议不变量
        assert_eq!(dump.name, fallback_name(entry.id));
        assert_eq!(dump.property_count as usize, dump.properties.len());
        let mut hashes = Vec::with_capacity(dump.properties.len());
        for p in &dump.properties {
            assert_eq!(p.hash.len(), 10, "0x%08x lowercase");
            assert!(p.hash.starts_with("0x"));
            assert!(p.hash.bytes().all(|b| b.is_ascii_hexdigit() || b == b'x'));
            assert!(!p.type_name.is_empty());
            hashes.push(p.hash.clone());
            if p.name.is_some() {
                named += 1;
            }
        }
        let unique: HashSet<_> = hashes.iter().cloned().collect();
        assert_eq!(unique.len(), hashes.len(), "hashes unique within a file");
        let mut sorted = hashes.clone();
        sorted.sort();
        assert_eq!(
            sorted, hashes,
            "properties sorted by hash string (= numeric order)"
        );

        total_properties += dump.properties.len();
        for p in &dump.properties {
            *type_histogram.entry(p.type_name).or_default() += 1;
        }
    }
    let secs = t0.elapsed().as_secs_f64();

    eprintln!(
        "dumped {} files / {total_properties} properties in {secs:.3}s; registry-named fields: {named}",
        entries.len()
    );
    let mut hist: Vec<_> = type_histogram.into_iter().collect();
    hist.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (name, n) in &hist {
        eprintln!("  {name:12} {n}");
    }

    // 已知构成（与 sc-properties 真实包测试一致的交叉验证）
    assert!(total_properties > 2000, "expected >2000 properties");
    assert!(
        hist.iter().any(|(n, _)| *n == "Key"),
        "Key entries expected"
    );
    assert!(
        hist.iter().any(|(n, _)| *n == "Float"),
        "Float entries expected"
    );
    if registry.is_some() {
        assert!(
            named > 100,
            "registry should name a meaningful share of hashes, got {named}"
        );
    }
}
