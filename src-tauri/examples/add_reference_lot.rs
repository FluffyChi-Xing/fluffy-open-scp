//! 把游戏里的真 lot 条目并入参照包（克隆参照 lot 用）。合并写入，不破坏
//! 参照包既有条目。
//! Usage: cargo run --release -p fluffy-open-scp --example add_reference_lot -- <game.package> <type:group:instance> <reference.package>
use dbpf::{OverlayEntry, Package, ResourceId};

fn main() {
    let mut args = std::env::args().skip(1);
    let game_path = args.next().expect("game package");
    let tgi = args.next().expect("type:group:instance");
    let reference_path = args.next().expect("reference package");
    let parts: Vec<u32> = tgi
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let id = ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    };
    let donor = {
        let game = Package::open(std::path::Path::new(&game_path)).expect("open game");
        let entry = game.entry(id).expect("entry in game package");
        game.read(entry).expect("read entry")
    };
    println!(
        "donor lot {:08X}:{:08X}:{:08X} = {} B",
        id.type_id,
        id.group,
        id.instance,
        donor.len()
    );
    // donor lot 引用的资源一并并入（LotMask raster / 地面与道具模型）。
    // type-0 key = 隐式模型引用，按 2F4E681B → 2F4E681C 顺序解析。
    let dependencies = {
        let game = Package::open(std::path::Path::new(&game_path)).expect("open game");
        let file = sc_properties::PropertyFile::parse(&donor).expect("parse donor lot");
        let mut wanted: Vec<ResourceId> = Vec::new();
        for property in &file.values {
            let mut push_key = |key: sc_properties::Key| {
                for candidate in [
                    ResourceId {
                        type_id: key.type_id,
                        group: key.group,
                        instance: key.instance,
                    },
                    ResourceId {
                        type_id: 0x2F4E_681B,
                        group: 0,
                        instance: key.instance,
                    },
                    ResourceId {
                        type_id: 0x2F4E_681C,
                        group: 0,
                        instance: key.instance,
                    },
                ] {
                    if !wanted.contains(&candidate) {
                        wanted.push(candidate);
                    }
                }
            };
            match &property.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) => push_key(*key),
                sc_properties::Kind::Array(values) => {
                    for value in values {
                        if let sc_properties::Value::Key(key) = value {
                            push_key(*key);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut resolved: Vec<(ResourceId, Vec<u8>)> = Vec::new();
        for candidate in wanted {
            if candidate.type_id != 0x2F4E_681B && candidate.type_id != 0x2F4E_681C {
                continue;
            }
            if let Some(entry) = game.entry(candidate) {
                let bytes = game.read(entry).expect("read dependency");
                println!(
                    "  dep {:08X}:{:08X}:{:08X} = {} B",
                    candidate.type_id,
                    candidate.group,
                    candidate.instance,
                    bytes.len()
                );
                resolved.push((candidate, bytes));
            }
        }
        // 二级遍历：并入的模型（2F4E681B）材质纹理行引用的 raster 一并并入，
        // 否则 material-binding-contract 会在构建期击落（引用闭包不完整）。
        let mut rasters: Vec<(ResourceId, Vec<u8>)> = Vec::new();
        for (candidate, bytes) in &resolved {
            if candidate.type_id != 0x2F4E_681B {
                continue;
            }
            let Ok(file) = rw4::Rw4File::parse(bytes) else {
                continue;
            };
            for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                let Ok(material) = file.decode_material(bytes, section.number) else {
                    continue;
                };
                for row in material.texture_refs() {
                    if row.slot & 0xFF == 0x2D {
                        continue;
                    }
                    let raster_id = ResourceId {
                        type_id: 0x2F4E_681C,
                        group: 0,
                        instance: row.texture_instance,
                    };
                    if resolved
                        .iter()
                        .chain(rasters.iter())
                        .any(|(id, _)| *id == raster_id)
                    {
                        continue;
                    }
                    if let Some(entry) = game.entry(raster_id) {
                        let raster_bytes = game.read(entry).expect("read raster");
                        println!(
                            "  raster {:08X} = {} B (material #{} of {:08X})",
                            row.texture_instance,
                            raster_bytes.len(),
                            section.number,
                            candidate.instance
                        );
                        rasters.push((raster_id, raster_bytes));
                    }
                }
            }
        }
        resolved.extend(rasters);
        resolved
    };
    // 参照包可能被应用进程映射；整包读进内存后立刻释放再写回。
    let mut entries = {
        let reference = Package::open(std::path::Path::new(&reference_path)).expect("open ref");
        let mut entries: Vec<OverlayEntry> = Vec::new();
        for entry in reference.entries() {
            let bytes = reference.read(entry).expect("read ref entry");
            entries.push(OverlayEntry::new(entry.id, bytes));
        }
        println!("existing reference entries: {}", entries.len());
        entries
    };
    entries.retain(|entry| entry.id != id);
    entries.push(OverlayEntry::new(id, donor));
    for (dep_id, bytes) in &dependencies {
        entries.retain(|entry| entry.id != *dep_id);
        entries.push(OverlayEntry::new(*dep_id, bytes.clone()));
    }
    let encoded = dbpf::write_uncompressed_overlay(&entries).expect("encode");
    // 输出路径独立传参：某些安装目录对 exe 写入拒绝（os error 5），由
    // 调用方用 shell 落回原位。
    let output_path = args
        .next()
        .unwrap_or_else(|| format!("{reference_path}.merged"));
    std::fs::write(&output_path, encoded).expect("write output");
    println!(
        "merged reference written: {output_path} ({} entries)",
        entries.len()
    );
}
