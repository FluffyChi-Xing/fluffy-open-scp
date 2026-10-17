//! 手动取证（effect Swarm 预览前期调查，2026-10-08）：扫描包内 Lot 属性的
//! Effect 单元，统计 effect_id 引用的资源 TGI 分布——回答「effect 资源是
//! 什么类型、在哪个包、有多少」。只读，不改任何数据。
//! cargo run -p sc-properties --release --example effect_units -- <pkg> [...]
use dbpf::Package;
use sc_properties::LotUnit;
use std::collections::BTreeMap;

fn main() {
    let mut ref_types: BTreeMap<u32, usize> = BTreeMap::new();
    let mut samples: Vec<String> = Vec::new();
    let mut lot_count = 0usize;
    let mut effect_count = 0usize;

    for path in std::env::args().skip(1) {
        let Ok(package) = Package::open(&path) else {
            println!("skip {path}");
            continue;
        };
        let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let Ok(file) = sc_properties::PropertyFile::parse_with_limits(
                &data,
                sc_properties::ParseLimits::default(),
            ) else {
                continue;
            };
            let units = sc_properties::LotEditorDocument::from_property_file(file)
                .assemble_units();
            for unit in &units.units {
                let LotUnit::Effect {
                    effect_id,
                    transform,
                    enabled,
                    ..
                } = unit
                else {
                    continue;
                };
                lot_count += 1;
                if let Some(key) = effect_id {
                    effect_count += 1;
                    *ref_types.entry(key.type_id).or_default() += 1;
                    if samples.len() < 24 {
                        samples.push(format!(
                            "{name} lot {:08X} eff#{:03} T {:08X} G {:08X} I {:08X} enabled={:?} pos=({:.1},{:.1},{:.1})",
                            entry.id.instance,
                            units.units.iter().position(|u| std::ptr::eq(u, unit)).unwrap_or(0),
                            key.type_id,
                            key.group,
                            key.instance,
                            enabled,
                            transform.as_ref().map(|t| t.matrix[9]).unwrap_or(0.0),
                            transform.as_ref().map(|t| t.matrix[10]).unwrap_or(0.0),
                            transform.as_ref().map(|t| t.matrix[11]).unwrap_or(0.0),
                        ));
                    }
                }
            }
        }
    }

    println!("lot effects total: {lot_count}; with effect_id: {effect_count}");
    println!("-- effect_id 引用的资源类型分布 --");
    for (type_id, count) in &ref_types {
        println!("  T {type_id:08X}: {count}");
    }
    println!("-- 样本（前 24 条）--");
    for line in &samples {
        println!("  {line}");
    }
}
