//! 反查探针：给定模型 RW4 instance，找出引用它作 LOD1~LOD4 的 lot property。
//!
//! 用法：
//! ```text
//! find_lot_by_model <model_instance_hex> <package> [extra_package...]
//! ```

use dbpf::Package;
use sc_properties::{LotEditorDocument, PropertyFile, ParseLimits};

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(model_hex) = args.first() else {
        eprintln!("usage: find_lot_by_model <model_instance_hex> <package> [...]");
        std::process::exit(2);
    };
    let target = u32::from_str_radix(model_hex.trim_start_matches("0x"), 16)
        .unwrap_or_else(|error| panic!("bad instance {model_hex}: {error}"));
    let packages: Vec<Package> = args
        .iter()
        .skip(1)
        .map(|path| Package::open(path).unwrap_or_else(|error| panic!("open {path}: {error}")))
        .collect();

    for (index, package) in packages.iter().enumerate() {
        let path = &args[index + 1];
        for entry in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
                continue;
            };
            let document = LotEditorDocument::from_property_file(file);
            let hit = document
                .model_lods
                .iter()
                .any(|key| key.map(|k| k.instance) == Some(target));
            if hit {
                println!(
                    "{path}: lot T {:08X} G {:08X} I {:08X} (lods: {:?})",
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    document
                        .model_lods
                        .iter()
                        .map(|k| k.map(|v| format!("{:08X}", v.instance)).unwrap_or_else(|| "-".into()))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
