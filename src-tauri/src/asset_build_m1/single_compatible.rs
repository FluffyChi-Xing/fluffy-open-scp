//! Pure-Rust input normalization for the author's single-mesh compiler.
//!
//! The author's `compile_tower_asset.py` accepts at most 64 material groups.
//! This module reproduces its first step exactly: hash the six-slot surface
//! payload plus the Float32 parameter block, keep one material per unique
//! combination, and remap every mesh reference. It deliberately does not touch
//! pixels or parameters yet; that is the next stage of the port.

use sha2::{Digest, Sha256};

use super::lotm::{LotmDocument, LotmMaterial, MeshMaterialRef};

pub const MAX_MATERIAL_ROWS: usize = 64;
pub const CELL: u32 = 512;

/// Layout of the author's single-mesh surface/palette/room tables.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePlan {
    pub cell: u32,
    /// Power-of-two cell grid edge, matching `2**ceil(log2(ceil(sqrt(n))))`.
    pub grid: u32,
    pub atlas: u32,
    /// Unique (tint, normal, shader) group selected for each material row.
    pub surface_of_row: Vec<usize>,
    /// Per-row 4x4 Float32 block after palette/region rewriting.
    pub rows: Vec<[f32; 16]>,
    pub room_keys: Vec<String>,
    pub room_atlas: u32,
    pub room_of_row: Vec<usize>,
    /// Source palette columns [Base, Top] per material row.
    pub palette_source: Vec<[u32; 2]>,
    /// Original room projection parameters (x scale, y offset) per room.
    pub room_params: Vec<[f32; 2]>,
}

fn f32_params(material: &LotmMaterial) -> Result<[f32; 16], String> {
    let bytes = material
        .params_f32
        .as_deref()
        .ok_or("single-compatible: material has no Float32 params")?;
    if bytes.len() < 64 {
        return Err(format!(
            "single-compatible: params are {} bytes, expected at least 64",
            bytes.len()
        ));
    }
    let mut values = [0f32; 16];
    for (index, value) in values.iter_mut().enumerate() {
        *value = f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap());
    }
    Ok(values)
}

fn surface_key(material: &LotmMaterial) -> String {
    let png = |index: usize| material.pngs.get(index).and_then(Option::as_ref);
    format!(
        "{}:{}:{}",
        digest(png(LotmMaterial::PNG_TINT)),
        digest(png(LotmMaterial::PNG_NORMAL)),
        digest(png(LotmMaterial::PNG_SHADER)),
    )
}

fn room_key(material: &LotmMaterial, params: &[f32; 16]) -> String {
    format!(
        "{}:{:.8}:{:.8}",
        digest(material.pngs.get(LotmMaterial::PNG_INTERIOR).and_then(Option::as_ref)),
        params[2],
        params[3],
    )
}

fn power_of_two_grid(unique: usize) -> u32 {
    let root = (unique as f64).sqrt().ceil().max(1.0) as u32;
    let mut grid = 1u32;
    while grid < root {
        grid *= 2;
    }
    grid
}

/// Reproduce the author's surface/palette/room parameter layout.
pub fn plan_surfaces(document: &LotmDocument) -> Result<SurfacePlan, String> {
    if document.materials.is_empty() {
        return Err("single-compatible: no materials to plan".into());
    }
    let mut surface_keys: Vec<String> = Vec::new();
    let mut room_keys: Vec<String> = Vec::new();
    let mut surface_of_row = Vec::with_capacity(document.materials.len());
    let mut room_of_row = Vec::with_capacity(document.materials.len());
    let mut rows = Vec::with_capacity(document.materials.len());
    let mut palette_source = Vec::with_capacity(document.materials.len());
    let mut room_params: Vec<[f32; 2]> = Vec::new();
    for (index, material) in document.materials.iter().enumerate() {
        let params = f32_params(material)?;
        let surface = surface_key(material);
        let surface_index = surface_keys
            .iter()
            .position(|key| key == &surface)
            .unwrap_or_else(|| {
                surface_keys.push(surface);
                surface_keys.len() - 1
            });
        let room = room_key(material, &params);
        let room_index = room_keys
            .iter()
            .position(|key| key == &room)
            .unwrap_or_else(|| {
                room_keys.push(room);
                room_keys.len() - 1
            });
        surface_of_row.push(surface_index);
        room_of_row.push(room_index);
        if room_params.len() <= room_index {
            room_params.push([params[2], params[3]]);
        }
        palette_source.push([
            (params[0] * 511.0).round() as u32 % 512,
            (params[1] * 511.0).round() as u32 % 512,
        ]);
        rows.push(params);
    }
    let grid = power_of_two_grid(surface_keys.len());
    let atlas = grid * CELL;
    let room_states = 64u32
        * 2u32.pow((room_keys.len().max(1) as f64).log2().ceil() as u32);
    let room_atlas = room_states.max(1024);
    let grid_f = grid as f32;
    let atlas_f = atlas as f32;
    for (row, params) in rows.iter_mut().enumerate() {
        let x = ((surface_of_row[row] as u32 % grid) * CELL) as f32;
        let y = ((surface_of_row[row] as u32 / grid) * CELL) as f32;
        // palette: row0[0]/row0[1] select Base/Top 2px column blocks.
        for layer in 0..2 {
            params[layer] = ((row * 4 + layer * 2) as f32) / 512.0;
        }
        for line in [1usize, 2] {
            let offset = line * 4;
            params[offset] /= grid_f;
            params[offset + 1] /= grid_f;
            params[offset + 2] = params[offset + 2] / grid_f + x / atlas_f;
            params[offset + 3] = params[offset + 3] / grid_f + y / atlas_f;
        }
        if params[2] != 0.0 {
            params[2] = 64.0 / room_atlas as f32;
            params[3] = room_of_row[row] as f32 * 64.0 / room_atlas as f32;
        }
        params[14] *= grid_f;
        params[15] *= grid_f;
    }
    Ok(SurfacePlan {
        cell: CELL,
        grid,
        atlas,
        surface_of_row,
        rows,
        room_keys,
        room_atlas,
        room_of_row,
        palette_source,
        room_params,
    })
}

fn digest(bytes: Option<&Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    if let Some(bytes) = bytes {
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn material_key(material: &LotmMaterial) -> String {
    let png = |index: usize| material.pngs.get(index).and_then(Option::as_ref);
    format!(
        "{}:{}:{}:{}:{}",
        digest(png(LotmMaterial::PNG_TINT)),
        digest(png(LotmMaterial::PNG_NORMAL)),
        digest(png(LotmMaterial::PNG_SHADER)),
        digest(png(LotmMaterial::PNG_PALETTE)),
        digest(material.params_f32.as_ref()),
    )
}

/// Collapse identical material rows and remap every mesh to its unique row.
pub fn deduplicate(document: &LotmDocument) -> Result<LotmDocument, String> {
    if document.materials.is_empty() {
        return Err("single-compatible: input has no materials".into());
    }
    let mut rows: Vec<LotmMaterial> = Vec::new();
    let mut row_by_key: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut material_row = Vec::with_capacity(document.materials.len());
    for material in &document.materials {
        let key = material_key(material);
        let row = *row_by_key.entry(key).or_insert_with(|| {
            rows.push(material.clone());
            rows.len() - 1
        });
        material_row.push(row);
    }
    if rows.len() > MAX_MATERIAL_ROWS {
        return Err(format!(
            "single-compatible: {} unique material rows exceed the engine limit {}",
            rows.len(),
            MAX_MATERIAL_ROWS
        ));
    }
    let mesh_material = document
        .mesh_material
        .iter()
        .map(|reference| {
            let row = material_row
                .get(reference.material as usize)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "single-compatible: mesh references material {} out of range",
                        reference.material
                    )
                })?;
            Ok(MeshMaterialRef {
                material: row as u32,
                uv_kind: reference.uv_kind,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(LotmDocument {
        version: document.version,
        meshes: document.meshes.clone(),
        materials: rows,
        mesh_material,
        diagnostics: document.diagnostics.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(params: u8) -> LotmMaterial {
        LotmMaterial {
            pngs: vec![None; 8],
            params_f32: Some(vec![params; 64]),
            param_cols: 1,
        }
    }

    #[test]
    fn collapses_identical_rows_and_remaps_meshes() {
        let document = LotmDocument {
            version: 9,
            meshes: vec![vec![], vec![], vec![]],
            materials: vec![material(0), material(1), material(0)],
            mesh_material: vec![
                MeshMaterialRef {
                    material: 0,
                    uv_kind: 0,
                },
                MeshMaterialRef {
                    material: 1,
                    uv_kind: 0,
                },
                MeshMaterialRef {
                    material: 2,
                    uv_kind: 0,
                },
            ],
            diagnostics: String::new(),
        };
        let output = deduplicate(&document).unwrap();
        assert_eq!(output.materials.len(), 2);
        assert_eq!(
            output
                .mesh_material
                .iter()
                .map(|reference| reference.material)
                .collect::<Vec<_>>(),
            vec![0, 1, 0]
        );
    }

    #[test]
    fn rejects_more_than_64_unique_rows() {
        let document = LotmDocument {
            version: 9,
            meshes: vec![vec![]],
            materials: (0..65).map(material).collect(),
            mesh_material: vec![MeshMaterialRef {
                material: 64,
                uv_kind: 0,
            }],
            diagnostics: String::new(),
        };
        assert!(deduplicate(&document).is_err());
    }
}
