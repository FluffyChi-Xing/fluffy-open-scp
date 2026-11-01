//! Inspect LOTM materials and the six-slot resolution used by the compiler.
//!
//! cargo run -p fluffy-open-scp --example lotm_materials -- <model.lotm>

use fluffy_open_scp_lib::asset_build_m1::{lotm, materials, native_model};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: lotm_materials <model.lotm>")?;
    let bytes = std::fs::read(path)?;
    let doc = lotm::parse(&bytes)?;
    let plan = materials::resolve_materials(&doc)?;
    println!(
        "version={} meshes={} materials={} unique={}",
        doc.version,
        doc.meshes.len(),
        doc.materials.len(),
        plan.unique.len()
    );
    let mut area = vec![0f64; plan.unique.len()];
    for (mesh_index, glb) in doc.meshes.iter().enumerate() {
        let material = plan.mesh_material.get(mesh_index).copied().unwrap_or(0) as usize;
        if material >= area.len() {
            continue;
        }
        for primitive in native_model::parse_glb_primitives(glb)? {
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
                area[material] += 0.5
                    * ((cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]) as f64)
                        .sqrt();
            }
        }
    }
    let mut order: Vec<usize> = (0..area.len()).collect();
    order.sort_by(|a, b| area[*b].partial_cmp(&area[*a]).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, material) in order.iter().take(10).enumerate() {
        println!(
            "rank {rank}: material {material} area {:.1} params={:?}",
            area[*material],
            doc.materials[*material].params_f32.as_ref().map(|bytes| {
                bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect::<Vec<_>>()
            })
        );
    }
    Ok(())
}
