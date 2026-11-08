//! Audit source LOTM interior inputs against mesh area.
//! Usage: cargo run -p fluffy-open-scp --example lotm_night_audit -- <model.lotm> [output_dir]

use fluffy_open_scp_lib::asset_build_m1::{lotm, native_model};
use image::{Rgba, RgbaImage};
use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("LOTM path missing")?;
    let output = args.next().map(std::path::PathBuf::from);
    if let Some(directory) = &output {
        fs::create_dir_all(directory)?;
    }
    let document = lotm::parse(&fs::read(path)?)?;
    println!("diagnostics={}", document.diagnostics);
    println!(
        "version={} meshes={} materials={} bindings={}",
        document.version,
        document.meshes.len(),
        document.materials.len(),
        document.mesh_material.len()
    );
    for (mesh_index, glb) in document.meshes.iter().enumerate() {
        let binding = document
            .mesh_material
            .get(mesh_index)
            .ok_or("mesh material binding missing")?;
        let material_index = binding.material;
        let material = document
            .materials
            .get(material_index as usize)
            .ok_or("mesh material index out of range")?;
        let mut area = 0f64;
        let mut vertices = 0usize;
        let mut columns = std::collections::BTreeMap::new();
        let mut seeds = std::collections::BTreeMap::new();
        for primitive in native_model::parse_glb_primitives(glb)? {
            vertices += primitive.positions.len();
            for column in &primitive.tint_column {
                *columns.entry(*column).or_insert(0usize) += 1;
            }
            for seed in &primitive.seeds {
                *seeds.entry(*seed).or_insert(0usize) += 1;
            }
            for triangle in primitive.indices.chunks_exact(3) {
                let a = primitive.positions[triangle[0] as usize];
                let b = primitive.positions[triangle[1] as usize];
                let c = primitive.positions[triangle[2] as usize];
                let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let cross = [
                    ab[1] * ac[2] - ab[2] * ac[1],
                    ab[2] * ac[0] - ab[0] * ac[2],
                    ab[0] * ac[1] - ab[1] * ac[0],
                ];
                area += 0.5
                    * ((cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2])
                        as f64)
                        .sqrt();
            }
        }
        let params = material
            .params_f32
            .as_ref()
            .map(|bytes| {
                bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let interior_scale = params.get(2).copied().unwrap_or(0.0);
        let interior_offset = params.get(3).copied().unwrap_or(0.0);
        let interior = material
            .pngs
            .get(lotm::LotmMaterial::PNG_INTERIOR)
            .and_then(Option::as_ref)
            .map(|bytes| image::load_from_memory(bytes))
            .transpose()?
            .map(|image| image.to_rgba8());
        let mut alpha_stats = String::new();
        if let Some(image) = &interior {
            let nonzero = image.pixels().filter(|pixel| pixel[3] != 0).count();
            let alpha_sum = image
                .pixels()
                .map(|pixel| u64::from(pixel[3]))
                .sum::<u64>();
            let max_alpha = image.pixels().map(|pixel| pixel[3]).max().unwrap_or(0);
            println!(
                "mesh {mesh_index:02} material={material_index:02} area={area:.1} vertices={vertices} scale={interior_scale} offset={interior_offset} interior={}x{} nonzero_alpha={nonzero} alpha_sum={alpha_sum} max_alpha={max_alpha} columns={columns:?} seeds={seeds:?}",
                image.width(),
                image.height(),
            );
            if image.width() >= 64 {
                for state in 0..image.width() / 64 {
                    let mut lit = 0usize;
                    let mut sum = 0u64;
                    for y in 0..image.height() {
                        for x in state * 64..(state + 1) * 64 {
                            let alpha = image.get_pixel(x, y)[3];
                            if alpha != 0 {
                                lit += 1;
                                sum += u64::from(alpha);
                            }
                        }
                    }
                    alpha_stats.push_str(&format!(" {state}:{lit}/{sum}"));
                }
                println!("  source_states{alpha_stats}");
            }
            if let Some(directory) = &output {
                image.save(directory.join(format!("mesh-{mesh_index:02}-interior.png")))?;
                let mut alpha = RgbaImage::new(image.width(), image.height());
                for (index, pixel) in image.pixels().enumerate() {
                    let value = pixel[3];
                    alpha.put_pixel(
                        index as u32 % image.width(),
                        index as u32 / image.width(),
                        Rgba([value, value, value, 255]),
                    );
                }
                alpha.save(directory.join(format!("mesh-{mesh_index:02}-alpha.png")))?;
            }
        } else {
            println!(
                "mesh {mesh_index:02} material={material_index:02} area={area:.1} vertices={vertices} scale={interior_scale} offset={interior_offset} interior=MISSING columns={columns:?} seeds={seeds:?}"
            );
        }
    }
    Ok(())
}
