//! 只读工具：按 decal 条目 instance 反查使用它的 lot。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example decal_find_lots -- \
//!       <package> <entry_instance_hex> [more_ids...]

use dbpf::Package;
use sc_properties::{inherit, Key, PropertyFile};

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: decal_find_lots <package> <id...>").clone();
    let targets: Vec<u32> = args[1..]
        .iter()
        .filter_map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .collect();
    let package = Package::open(&path).expect("open package");

    // 字典：条目 instance → 字典 material
    let mut entry_material: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        let Ok(bytes) = package.read(entry) else { continue };
        if let Ok(dict) = sc_properties::DecalDictionary::parse(&bytes) {
            let Some(material) = dict.material else { continue };
            for e in &dict.entries {
                if let Some(id) = e.id {
                    entry_material.entry(id.instance).or_insert(material.instance);
                }
            }
        }
    }

    let resolve_none = |_: &Key| -> Option<PropertyFile> { None };
    let _ = resolve_none;
    // 预读全部 property 资源供父链解析（lot 的 decal 列可能继承自父模板）
    let mut prop_cache: std::collections::HashMap<u32, PropertyFile> =
        std::collections::HashMap::new();
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        if let Ok(raw) = package.read(entry) {
            if let Ok(file) = PropertyFile::parse(&raw) {
                prop_cache.insert(entry.id.instance, file);
            }
        }
    }
    let resolve = |key: &Key| -> Option<PropertyFile> {
        prop_cache.get(&key.instance).cloned()
    };
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        let Ok(raw) = package.read(entry) else { continue };
        let Ok(raw_file) = PropertyFile::parse(&raw) else { continue };
        let flattened = inherit::flatten_parent_inheritance(raw_file, resolve);
        for cat in 0..3u32 {
            let Some(value) = flattened.get(0x0D10_9050 + cat) else { continue };
            let Some(arr) = value.array() else { continue };
            for (index, v) in arr.iter().enumerate() {
                if let sc_properties::Value::Key(k) = v {
                    if targets.contains(&k.instance) {
                        println!(
                            "lot 0x{:08X} cat{}:{} 条目 0x{:08X} (material {})",
                            entry.id.instance,
                            cat,
                            index,
                            k.instance,
                            entry_material
                                .get(&k.instance)
                                .map(|m| format!("0x{m:08X}"))
                                .unwrap_or_else(|| "?".into())
                        );
                    }
                }
            }
        }
    }
}
