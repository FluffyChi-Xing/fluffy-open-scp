//! 冒烟工具：从包里解出指定 RW4 模型，打印每个材质的纹理行引用的
//! raster 实例（白模诊断：模型到底绑没绑上纹理）。
//! Usage: cargo run --release -p fluffy-open-scp --example model_texture_refs -- <package> <model_instance_hex>
use dbpf::{Package, ResourceId};
use rw4::SectionType;

fn main() {
    let mut args = std::env::args().skip(1);
    let package_path = args.next().expect("package");
    let instance = u32::from_str_radix(
        args.next()
            .expect("model instance hex")
            .trim_start_matches("0x"),
        16,
    )
    .expect("hex");
    let package = Package::open(std::path::Path::new(&package_path)).expect("open package");
    // ALL = 扫描包内全部模型，统计引用了 32B 空 raster 的材质（白模闭包检查）。
    if args.next().map(|flag| flag == "ALL").unwrap_or(false) {
        let empty_rasters: Vec<u32> = package
            .entries()
            .iter()
            .filter(|entry| entry.id.type_id == 0x2F4E_681C)
            .filter(|entry| {
                package
                    .read(entry)
                    .map(|bytes| bytes.len() <= 64)
                    .unwrap_or(false)
            })
            .map(|entry| entry.id.instance)
            .collect();
        println!("empty rasters (<=64B): {:08X?}", empty_rasters);
        let mut poisoned = 0usize;
        let mut checked = 0usize;
        for entry in package.entries() {
            if entry.id.type_id != 0x2F4E_681B {
                continue;
            }
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(file) = rw4::Rw4File::parse(&bytes) else {
                continue;
            };
            checked += 1;
            for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                if let Ok(material) = file.decode_material(&bytes, section.number) {
                    let hits: Vec<u32> = material
                        .texture_refs()
                        .iter()
                        .filter(|row| empty_rasters.contains(&row.texture_instance))
                        .map(|row| row.texture_instance)
                        .collect();
                    if !hits.is_empty() {
                        poisoned += 1;
                        println!(
                            "  POISONED model {:08X} material #{} -> {:08X?}",
                            entry.id.instance, section.number, hits
                        );
                    }
                }
            }
        }
        println!("checked {checked} models, poisoned materials: {poisoned}");
        return;
    }
    let id = ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance,
    };
    let entry = package
        .entry(id)
        .unwrap_or_else(|| panic!("model {instance:08X} not in package"));
    let bytes = package.read(entry).expect("read model");
    let file = rw4::Rw4File::parse(&bytes).expect("parse rw4");
    println!(
        "model {instance:08X}: {} B, mesh={} material={} mma={} texture={}",
        bytes.len(),
        file.sections_of_type(SectionType::MESH).count(),
        file.sections_of_type(SectionType::MATERIAL).count(),
        file.sections_of_type(SectionType::MESH_MATERIAL_ASSIGNMENT)
            .count(),
        file.sections_of_type(SectionType::TEXTURE).count(),
    );
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let decoded = file.decode_material(&bytes, section.number);
        match decoded {
            Ok(material) => {
                let refs = material.texture_refs();
                let slots: Vec<String> = refs
                    .iter()
                    .map(|row| format!("slot{}->{:08X}", row.slot & 0xFF, row.texture_instance))
                    .collect();
                println!("  material #{:>2}: {}", section.number, slots.join(" "));
            }
            Err(error) => println!("  material #{} decode error: {error}", section.number),
        }
    }
}
