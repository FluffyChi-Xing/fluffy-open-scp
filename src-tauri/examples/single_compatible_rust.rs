//! Backend entry for the Rust single-mesh compiler.
//!
//! Input: an existing LOTM9 from the Blender plugin.
//! Output: a one-material LOTM9 with the author's 512-cell surface atlases,
//! 4×width Float32 table and 16-state room atlas. Author semantics
//! (prepare_asset 0.8.6): every source group keeps its own material row —
//! only surface/room atlas cells dedupe. The normal `build_asset` pipeline
//! can then compile RW4 plus property/menu/DBPF.

use fluffy_open_scp_lib::asset_build_m1::{lotm, single_compatible};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .ok_or("usage: single_compatible_rust <in.lotm> <out.lotm>")?;
    let output = env::args()
        .nth(2)
        .ok_or("usage: single_compatible_rust <in.lotm> <out.lotm>")?;
    let source = lotm::parse(&fs::read(input)?)?;
    let plan = single_compatible::plan_surfaces(&source)?;
    let atlases = single_compatible::render_atlases(&source, &plan)?;
    let composed = single_compatible::compose_lotm(&source, &plan, &atlases);
    fs::write(&output, single_compatible::write_lotm(&composed))?;
    println!(
        "WROTE {output} rows={} surfaces={} atlas={} rooms={}",
        plan.rows.len(),
        plan.surface_of_row.iter().max().copied().unwrap_or(0) + 1,
        plan.atlas,
        plan.room_keys.len()
    );
    Ok(())
}
