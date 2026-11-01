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
