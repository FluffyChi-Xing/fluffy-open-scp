//! 一次性诊断：dump 任意 .package 的类型普查、模型材质槽、贴图编码与
//! 游戏 TGI 重叠情况。用法：
//! `cargo run -p rw4 --example nighthouse_dump -- <pkg> [gameDataDir]`

use std::collections::{BTreeMap, HashSet};

use dbpf::{IndexEntry, Package};
use rw4::{MaterialSection, RasterImage, Rw4File, SectionType};

fn tgi(e: &IndexEntry) -> String {
    format!(
        "{:08X}:{:08X}:{:08X}",
        e.id.type_id, e.id.group, e.id.instance
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: nighthouse_dump <pkg> [gameDataDir]");
    let game_dir = args.get(2);

    let pkg = Package::open(path)?;
    println!("=== {path} ===");
    println!("entries: {}", pkg.entries().len());

    let mut by_type: BTreeMap<u32, Vec<&IndexEntry>> = BTreeMap::new();
    for e in pkg.entries() {
        by_type.entry(e.id.type_id).or_default().push(e);
    }
    println!("\n-- type census --");
    for (t, es) in &by_type {
        let total: u64 = es.iter().map(|e| e.decompressed_size as u64).sum();
        println!(
            "{t:08X} ({}): {} entries, {total} bytes",
            SectionType::name(*t).unwrap_or("?"),
            es.len()
        );
    }

    let mut game_tgis: HashSet<(u32, u32, u32)> = HashSet::new();
    if let Some(dir) = game_dir {
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.extension().is_some_and(|e| e == "package")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("SimCity_"))
            {
                match Package::open(&p) {
                    Ok(gp) => {
                        println!("indexing {} ({} entries)", p.display(), gp.entries().len());
                        for e in gp.entries() {
                            game_tgis.insert((e.id.type_id, e.id.group, e.id.instance));
                        }
                    }
                    Err(e) => println!("skip {}: {e}", p.display()),
                }
            }
        }
    }

    println!("\n-- models (0x2F4E681B) --");
    for e in by_type.get(&0x2F4E681B).map(Vec::as_slice).unwrap_or(&[]) {
        let data = pkg.read(e)?;
        println!(
            "\nMODEL {} ({} bytes, compressed={})",
            tgi(e),
            data.len(),
            e.compressed
        );
        if game_tgis.contains(&(e.id.type_id, e.id.group, e.id.instance)) {
            println!("  [TGI exists in game packages -> override]");
        }
        let file = match Rw4File::parse(&data) {
            Ok(f) => f,
            Err(err) => {
                println!("  parse FAILED: {err}");
                continue;
            }
        };
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        for s in file.sections() {
            *counts.entry(s.type_code).or_default() += 1;
        }
        let mut line = String::new();
        for (tc, n) in &counts {
            line.push_str(&format!(
                "{}({tc:05X})x{n} ",
                SectionType::name(*tc).unwrap_or("?")
            ));
        }
        println!("  sections: {line}");

        for (i, m) in file
            .sections_of_type(SectionType::MATERIAL)
            .collect::<Vec<_>>()
            .iter()
            .enumerate()
        {
            match file.decode_material(&data, m.number) {
                Ok(MaterialSection::Decoded(dm)) => {
                    let slots: Vec<String> = dm
                        .texture_slots()
                        .map(|r| format!("slot{}={:08X}", r.slot_byte(), r.texture_instance))
                        .collect();
                    println!("  material#{i}: {}", slots.join(" "));
                }
                Ok(MaterialSection::Raw(_)) => {
                    println!("  material#{i}: RAW (layout not recognized)");
                }
                Err(err) => println!("  material#{i}: decode error {err}"),
            }
        }
        let bindings = file.decode_mesh_material_bindings(&data);
        println!(
            "  mesh-material bindings: {} (mesh sections {})",
            bindings.len(),
            counts.get(&SectionType::MESH).unwrap_or(&0)
        );
        for m in file.sections_of_type(SectionType::MESH).collect::<Vec<_>>() {
            match file.decode_mesh(&data, m.number) {
                Ok(dm) => {
                    let mut uv_min = [f32::MAX, f32::MAX];
                    let mut uv_max = [f32::MIN, f32::MIN];
                    let mut uv4_max = 0.0f32;
                    let mut n_color = 0usize;
                    for v in &dm.vertices {
                        if let Some(uv) = v.uv() {
                            uv_min[0] = uv_min[0].min(uv[0]);
                            uv_min[1] = uv_min[1].min(uv[1]);
                            uv_max[0] = uv_max[0].max(uv[0]);
                            uv_max[1] = uv_max[1].max(uv[1]);
                        }
                        for (e, val) in &v.components {
                            if e.usage == rw4::DeclarationUsage::TexCoord {
                                if let rw4::ComponentValue::Float4(f) = val {
                                    uv4_max = uv4_max.max(
                                        f[0].abs().max(f[1].abs()).max(f[2].abs()).max(f[3].abs()),
                                    );
                                }
                            }
                            if e.usage == rw4::DeclarationUsage::Color {
                                n_color += 1;
                            }
                        }
                    }
                    let has_uv = uv_max[0] >= f32::MIN;
                    println!(
                        "  mesh #{}: {} verts {} tris; uv {}..{:?}; FLOAT4|max|={uv4_max:.2}; color verts={n_color}; tangent={}>",
                        m.number,
                        dm.vertices.len(),
                        dm.triangles.len(),
                        if has_uv {
                            format!("{uv_min:?}")
                        } else {
                            "none".into()
                        },
                        if has_uv {
                            format!("{uv_max:?}")
                        } else {
                            "none".into()
                        },
                        dm.vertices.first().and_then(|v| v.tangent()).is_some(),
                    );
                    if let Some(v0) = dm.vertices.first() {
                        let comps: Vec<String> = v0
                            .components
                            .iter()
                            .map(|(e, val)| {
                                format!("{:?}/{:?}[{}]={val:?}", e.usage, e.decl_type, e.index)
                            })
                            .collect();
                        println!("    first vertex: {}", comps.join(" | "));
                    }
                }
                Err(err) => println!("  mesh #{}: decode error {err}", m.number),
            }
        }
        for t in file
            .sections_of_type(SectionType::TEXTURE)
            .collect::<Vec<_>>()
        {
            match file.decode_texture(&data, t.number) {
                Ok(dt) => println!(
                    "  embedded texture #{}: type={} {}x{} mips={} blob={}",
                    t.number,
                    dt.texture_type,
                    dt.width,
                    dt.height,
                    dt.mip_count(),
                    dt.blob.len()
                ),
                Err(err) => println!("  embedded texture #{}: decode error {err}", t.number),
            }
        }
    }

    println!("\n-- rasters (0x2F4E681C) --");
    for e in by_type.get(&0x2F4E681C).map(Vec::as_slice).unwrap_or(&[]) {
        let data = pkg.read(e)?;
        match RasterImage::parse(&data) {
            Ok(r) => println!(
                "RASTER {} {}x{} pixFmt={} mips={} rawlen={} decodable={}",
                tgi(e),
                r.width,
                r.height,
                r.pixel_format,
                r.mip_count,
                data.len(),
                r.decode_top_mip_rgba().is_ok()
            ),
            Err(err) => println!("RASTER {} parse error {err}", tgi(e)),
        }
    }

    println!("\n-- properties (0x00B1B104) --");
    for e in by_type.get(&0x00B1B104).map(Vec::as_slice).unwrap_or(&[]) {
        let data = pkg.read(e)?;
        let ov = if game_tgis.contains(&(e.id.type_id, e.id.group, e.id.instance)) {
            " [override]"
        } else {
            ""
        };
        println!("PROP {} size={} bytes{ov}", tgi(e), data.len());
    }

    if !game_tgis.is_empty() {
        let mut ov = 0usize;
        for e in pkg.entries() {
            if game_tgis.contains(&(e.id.type_id, e.id.group, e.id.instance)) {
                ov += 1;
            }
        }
        println!(
            "\n-- TGI overlap with game packages: {ov} override / {} new --",
            pkg.entries().len() - ov
        );
    }
    Ok(())
}
