//! Backend entry for the Rust single-mesh compiler.
//!
//! Input: an existing LOTM9 from the Blender plugin.
//! Output: a one-material LOTM9 with the author's 512-cell surface atlases,
//! 4x64 Float32 table and 16-state room atlas. The normal `build_asset`
//! pipeline can then compile RW4 plus property/menu/DBPF.

use fluffy_open_scp_lib::asset_build_m1::{lotm, single_compatible};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or("usage: single_compatible_rust <in.lotm> <out.lotm>")?;
    let output = env::args().nth(2).ok_or("usage: single_compatible_rust <in.lotm> <out.lotm>")?;
    let source = lotm::parse(&fs::read(input)?)?;
    let deduplicated = single_compatible::deduplicate(&source)?;
    let plan = single_compatible::plan_surfaces(&deduplicated)?;
    let atlases = single_compatible::render_atlases(&deduplicated, &plan)?;
    let composed = single_compatible::compose_lotm(&deduplicated, &plan, &atlases);
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
