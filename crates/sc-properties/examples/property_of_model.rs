//! 冒烟工具：反查引用指定 RW4 模型（LOD1-4 键）的 property 实例。
//!
//! 用法：cargo run -p sc-properties --release --example property_of_model -- \
//!   <package> 0xC2D6D136 [0x...]

use sc_properties::{LotEditorDocument, PropertyFile};

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, targets) = args.split_first().expect("usage: property_of_model <package> 0xMODEL...");
    let package = dbpf::Package::open(path).expect("open package");
    let targets: Vec<u32> = targets
        .iter()
        .filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .collect();
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(properties) =
            PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default())
        else {
            continue;
        };
        let document = LotEditorDocument::from_property_file(properties);
        for lod in document.model_lods.iter().flatten() {
            if targets.contains(&lod.instance) {
                println!(
                    "model 0x{:08X} <- property 0x{:08X} (LOD level {})",
                    lod.instance,
                    entry.id.instance,
                    document
                        .model_lods
                        .iter()
                        .position(|l| l.as_ref().is_some_and(|k| k.instance == lod.instance))
                        .unwrap_or(0)
                        + 1
                );
            }
        }
    }
}
