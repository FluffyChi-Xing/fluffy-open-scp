//! 只读探针：打印指定 lot 的全部 Decal unit 静态字段
//! （category / scale / depth / material_data / transform 平移 / decal id），
//! 并附字典级 material（0x0CE5EF4E）供族路由判读。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example decal_unit_dump -- \
//!       <package> <lot_instance> [extra_package...]

use dbpf::Package;
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::{assemble_units, inherit, Key, PropertyFile};

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: decal_unit_dump <package> <lot> [extra...]").clone();
    let target = u32::from_str_radix(
        args.get(1).expect("missing lot instance").trim_start_matches("0x"),
        16,
    )
    .expect("bad lot instance");
    let extras: Vec<String> = args.iter().skip(2).cloned().collect();

    let mut packages: Vec<Package> = vec![Package::open(&path).expect("open lot package")];
    for extra in &extras {
        if let Ok(pkg) = Package::open(extra) {
            packages.push(pkg);
        }
    }

    // 字典索引：instance → material
    let mut dict_material: std::collections::HashMap<u32, Option<Key>> =
        std::collections::HashMap::new();
    let mut dicts: Vec<DecalDictionary> = Vec::new();
    for package in &packages {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE
                || !DECAL_ATLAS_INSTANCE_TYPES.contains(&(entry.id.group as u16))
            {
                continue;
            }
            let Ok(bytes) = package.read(entry) else { continue };
            if let Ok(dict) = DecalDictionary::parse(&bytes) {
                dict_material.insert(entry.id.instance, dict.material);
                dicts.push(dict);
            }
        }
    }

    let mut found = false;
    for entry in packages[0].entries() {
        if entry.id.type_id != PROPERTY_TYPE || entry.id.instance != target {
            continue;
        }
        found = true;
        let Ok(raw) = packages[0].read(entry) else { continue };
        let Ok(raw_file) = PropertyFile::parse(&raw) else { continue };
        let resolve = |key: &Key| -> Option<PropertyFile> {
            for source in &packages {
                for candidate in source.entries() {
                    if candidate.id.instance == key.instance
                        && (key.type_id == 0 || candidate.id.type_id == key.type_id)
                    {
                        let Ok(bytes) = source.read(candidate) else { continue };
                        if let Ok(file) = PropertyFile::parse(&bytes) {
                            return Some(file);
                        }
                    }
                }
            }
            None
        };
        let flattened = inherit::flatten_parent_inheritance(raw_file.clone(), resolve);
        let units = assemble_units(&flattened);
        println!("==== lot 0x{target:08X} ====");

        // 逐 category 打印 decal id 列，便于把 unit 下标对到条目
        for cat in 0..3u32 {
            if let Some(value) = flattened.get(0x0D10_9050 + cat) {
                if let Some(arr) = value.array() {
                    let ids: Vec<String> = arr
                        .iter()
                        .map(|v| match v {
                            sc_properties::Value::Key(k) => format!("0x{:08X}", k.instance),
                            _ => "?".into(),
                        })
                        .collect();
                    println!("  cat{cat} ids ({}): {}", ids.len(), ids.join(", "));
                    // 字典 material（若能按 id 找回所在字典）
                    for v in arr {
                        if let sc_properties::Value::Key(k) = v {
                            for dict in &dicts {
                                if dict.entries.iter().any(|e| {
                                    e.id.map(|ek| ek.instance) == Some(k.instance)
                                }) {
                                    println!(
                                        "    0x{:08X} ∈ 字典(material={})",
                                        k.instance,
                                        dict.material
                                            .map(|m| format!("0x{:08X}", m.instance))
                                            .unwrap_or_else(|| "None".into())
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        for unit in &units.units {
            if let sc_properties::LotUnit::Decal {
                index,
                category,
                transform,
                scale,
                depth,
                material_data,
                ..
            } = unit
            {
                let t = transform
                    .as_ref()
                    .map(|tr| {
                        format!(
                            "({:.2},{:.2},{:.2})",
                            tr.matrix[9], tr.matrix[10], tr.matrix[11]
                        )
                    })
                    .unwrap_or_else(|| "None".into());
                println!(
                    "  [cat{category}:{index}] scale={:?} depth={:?} material_data={:?} pos={}",
                    scale, depth, material_data, t
                );
            }
        }
    }
    if !found {
        println!("lot 0x{target:08X} 未在首个包中找到");
    }
}
