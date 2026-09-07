//! 冒烟探针：收集材质 slot 0x2D（shader-def）引用并跨包定位 + 字节取证（阶段 4）。
//!
//! 用法：cargo run -p rw4 --release --example shader_def_scan -- \
//!   <material_package> <candidate_package>...

use std::collections::BTreeSet;

const RW4_IMAGE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (material_pkg, candidates) = args.split_first().expect("usage: shader_def_scan <pkg> [pkg...]");
    let package = dbpf::Package::open(material_pkg).expect("open material package");

    let mut shader_defs: BTreeSet<u32> = BTreeSet::new();
    for entry in package.entries() {
        if entry.id.type_id != RW4_IMAGE {
            continue;
        }
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
        for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
            let Ok(rw4::MaterialSection::Decoded(mat)) =
                file.decode_material(&data, section.number)
            else {
                continue;
            };
            for r in &mat.texture_refs {
                if r.slot == rw4::SHADER_DEF_MARKER && r.texture_instance != 0 {
                    shader_defs.insert(r.texture_instance);
                }
            }
        }
    }
    println!("unique shader-def instances referenced: {}", shader_defs.len());
    for id in &shader_defs {
        println!("    0x{id:08X}");
    }

    for candidate in candidates {
        let Ok(pkg) = dbpf::Package::open(candidate) else {
            println!("[{candidate}] open failed");
            continue;
        };
        let name = std::path::Path::new(candidate)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut hits = 0usize;
        let mut sample: Option<(u32, Vec<u8>)> = None;
        for id in &shader_defs {
            if let Some(entry) = package_entry(&pkg, *id) {
                hits += 1;
                if sample.is_none() {
                    if let Ok(data) = pkg.read(&entry) {
                        sample = Some((*id, data));
                    }
                }
            }
        }
        println!("[{name}] shader-def hits: {hits}/{}", shader_defs.len());
        if let Some((id, data)) = sample {
            println!(
                "  sample 0x{id:08X}: type_id=0x{:08X} size={} bytes",
                package_entry(&pkg, id).map(|e| e.id.type_id).unwrap_or(0),
                data.len()
            );
            let head = &data[..data.len().min(160)];
            for (row, chunk) in head.as_chunks::<16>().0.iter().enumerate() {
                let hex: String = chunk
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let floats: String = chunk
                    .chunks_exact(4)
                    .map(|c| format!("{:.3}", f32::from_le_bytes(c.try_into().unwrap())))
                    .collect::<Vec<_>>()
                    .join(",");
                println!("  {:02x}0  {hex:<47}  | {floats}", row);
            }
        }
    }
}

fn package_entry(pkg: &dbpf::Package, instance: u32) -> Option<dbpf::IndexEntry> {
    pkg.entries()
        .iter()
        .find(|e| e.id.instance == instance)
        .cloned()
}
