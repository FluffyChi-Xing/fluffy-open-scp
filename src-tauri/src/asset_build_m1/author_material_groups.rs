//! Port of SC5 Asset Packager (2) engine/material_groups.py:3–27.

use std::collections::HashMap;

use super::lotm::LotmDocument;
use super::single_compatible::MAX_MATERIAL_ROWS;

pub(super) struct MaterialLayout {
    pub source_by_row: Vec<usize>,
    pub row_by_group: Vec<usize>,
    pub write_order: Vec<usize>,
    pub compacted: bool,
}

pub(super) fn material_layout(doc: &LotmDocument) -> Result<MaterialLayout, String> {
    let count = doc.materials.len();
    if count == 0 || doc.meshes.len() != count {
        return Err(format!(
            "author materials: expected one material per geometry group ({} meshes, {count} materials)",
            doc.meshes.len()
        ));
    }
    for (group, material) in doc.materials.iter().enumerate() {
        if material.param_cols != 1 || material.params_f32.as_ref().map(Vec::len) != Some(64) {
            return Err(format!(
                "author materials: group {} requires a 4×1×4 Float32 parameter table",
                group + 1
            ));
        }
    }
    if count <= MAX_MATERIAL_ROWS {
        return Ok(MaterialLayout {
            source_by_row: (0..count).collect(),
            row_by_group: (0..count).collect(),
            write_order: (0..count).collect(),
            compacted: false,
        });
    }
    let mut rows = HashMap::new();
    let mut source_by_row = Vec::new();
    let mut row_by_group = Vec::with_capacity(count);
    let mut last_group_by_row = Vec::new();
    for (group, material) in doc.materials.iter().enumerate() {
        let key = (&material.pngs, &material.params_f32);
        let row = *rows.entry(key).or_insert_with(|| {
            source_by_row.push(group);
            last_group_by_row.push(group);
            source_by_row.len() - 1
        });
        row_by_group.push(row);
        last_group_by_row[row] = group;
    }
    if source_by_row.len() > MAX_MATERIAL_ROWS {
        return Err(format!(
            "author Building40: {count} geometry groups, {} unique material rows after exact deduplication; maximum is {MAX_MATERIAL_ROWS}. Merge materials in Blender and re-export LOTM.",
            source_by_row.len()
        ));
    }
    let mut write_order: Vec<_> = (0..source_by_row.len()).collect();
    write_order.sort_unstable_by_key(|&row| last_group_by_row[row]);
    Ok(MaterialLayout {
        source_by_row,
        row_by_group,
        write_order,
        compacted: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::lotm::LotmMaterial;

    fn fixture(count: usize) -> LotmDocument {
        let materials = (0..count)
            .map(|group| LotmMaterial {
                pngs: vec![None; 8],
                params_f32: Some(vec![group as u8; 64]),
                param_cols: 1,
            })
            .collect();
        LotmDocument {
            version: 9,
            meshes: vec![Vec::new(); count],
            materials,
            mesh_material: Vec::new(),
            diagnostics: "{}".into(),
        }
    }

    #[test]
    fn merges_only_byte_identical_rows_above_the_limit() {
        let mut doc = fixture(130);
        doc.materials[128] = doc.materials[0].clone();
        doc.materials[129] = doc.materials[1].clone();
        let layout = material_layout(&doc).unwrap();
        assert_eq!(layout.source_by_row.len(), 128);
        assert_eq!(layout.row_by_group[128..], [0, 1]);
        assert_eq!(layout.write_order[126..], [0, 1]);
        doc.materials[129].pngs[LotmMaterial::PNG_INTERIOR] = Some(vec![1]);
        assert!(
            material_layout(&doc)
                .err()
                .unwrap()
                .contains("129 unique material rows")
        );
        doc.materials[129] = doc.materials[1].clone();
        doc.materials[129].params_f32.as_mut().unwrap()[0] ^= 1;
        assert!(material_layout(&doc).is_err());
    }

    #[test]
    fn leaves_material_rows_unchanged_within_the_limit() {
        let mut doc = fixture(128);
        doc.materials[127] = doc.materials[0].clone();
        let layout = material_layout(&doc).unwrap();
        assert_eq!(layout.source_by_row.len(), 128);
        assert_eq!(layout.row_by_group[127], 127);
    }
}
