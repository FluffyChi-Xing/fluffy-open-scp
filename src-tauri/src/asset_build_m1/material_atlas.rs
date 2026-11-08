//! Bake the per-material tint masks into one atlas.
//!
//! The game's custom-building loader accepts only a one-mesh static model here,
//! while the LOTM can carry ~100 facade materials.  The compromise is a single
//! material whose slot1 is an atlas of every material's tint mask.  Each source
//! mesh keeps its own atlas cell in its vertex UVs; the material's regionXform
//! becomes identity.  This module is pure: plan + image bytes in, atlas bytes
//! and normalized cell rectangles out.

use super::materials::MaterialPlan;
use image::{ImageFormat, RgbaImage};
use std::io::Cursor;

pub struct TintAtlas {
    pub png: Vec<u8>,
    /// `[scale_x, scale_y, offset_x, offset_y]` per unique material.
    pub cells: Vec<[f32; 4]>,
    pub size: u32,
}

/// 16脳16 cells of 128 px; enough for the commercial sample's 98 materials
/// while keeping the atlas at 2048虏.
const GRID: u32 = 16;
const CELL: u32 = 128;

pub fn build_tint_atlas(plan: &MaterialPlan) -> Result<TintAtlas, String> {
    let size = GRID * CELL;
    let mut canvas = RgbaImage::new(size, size);
    let mut cells = Vec::with_capacity(plan.unique.len());
    for (index, material) in plan.unique.iter().enumerate() {
        let cell_x = index as u32 % GRID;
        let cell_y = index as u32 / GRID;
        if cell_y >= GRID {
            return Err(format!(
                "tint atlas has {} materials, max {}",
                plan.unique.len(),
                GRID * GRID
            ));
        }
        let source = material
            .slots
            .get(1)
            .and_then(|slot| slot.as_ref())
            .filter(|(_, is_png)| *is_png)
            .and_then(|(bytes, _)| image::load_from_memory(bytes).ok());
        let cell_image = match source {
            Some(image) => image::imageops::resize(
                &image.to_rgba8(),
                CELL,
                CELL,
                image::imageops::FilterType::Triangle,
            ),
            None => RgbaImage::from_pixel(CELL, CELL, image::Rgba([255, 255, 255, 255])),
        };
        image::imageops::replace(
            &mut canvas,
            &cell_image,
            i64::from(cell_x * CELL),
            i64::from(cell_y * CELL),
        );
        cells.push([
            CELL as f32 / size as f32,
            CELL as f32 / size as f32,
            (cell_x * CELL) as f32 / size as f32,
            (cell_y * CELL) as f32 / size as f32,
        ]);
    }
    let mut png = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|error| format!("tint atlas PNG encode failed: {error}"))?;
    Ok(TintAtlas { png, cells, size })
}

/// Keep the source parameter table but force both region transforms to
/// identity; the UVs now carry their atlas cell directly.
pub fn identity_params(source: Option<&[u8]>) -> Vec<u8> {
    let mut params = source.map(<[u8]>::to_vec).unwrap_or_default();
    if params.len() < 64 {
        params.resize(64, 0);
    }
    let identity = [1.0f32, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0];
    for (index, value) in identity.iter().enumerate() {
        params[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::materials::ResolvedMaterial;

    #[test]
    fn atlas_cells_are_inside_the_image_and_identity_params_are_canonical() {
        let plan = MaterialPlan {
            unique: vec![
                ResolvedMaterial {
                    slots: Default::default(),
                },
                ResolvedMaterial {
                    slots: Default::default(),
                },
            ],
            mesh_material: vec![0, 1],
        };
        let atlas = build_tint_atlas(&plan).unwrap();
        assert_eq!(atlas.size, 2048);
        assert_eq!(atlas.cells.len(), 2);
        for cell in &atlas.cells {
            assert!(cell[0] > 0.0 && cell[2] + cell[0] <= 1.0);
            assert!(cell[1] > 0.0 && cell[3] + cell[1] <= 1.0);
        }
        assert!(atlas.png.starts_with(b"\x89PNG"));
        let params = identity_params(None);
        assert_eq!(params.len(), 64);
        assert_eq!(f32::from_le_bytes(params[0..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(params[16..20].try_into().unwrap()), 1.0);
    }
}
