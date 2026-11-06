//! LOTM v9 container writer (mirrors `native_asset`'s payload layout).

#[derive(Debug, Clone, Default)]
pub struct LotmMaterial {
    /// Container order: 0 empty, 1 normal, 2 empty, 3 empty, 4 tint,
    /// 5 palette, 6 shader, 7 interior.
    pub maps: [Vec<u8>; 8],
    /// 4×float4 parameter rows.
    pub params: Vec<f32>,
    pub param_cols: u32,
}

impl LotmMaterial {
    pub fn from_recipe(maps: [Vec<u8>; 5], params: [f32; 16], param_cols: u32) -> Self {
        let [tint, normal, shader, palette, interior] = maps;
        let mut ordered: [Vec<u8>; 8] = Default::default();
        ordered[1] = normal;
        ordered[4] = tint;
        ordered[5] = palette;
        ordered[6] = shader;
        ordered[7] = interior;
        Self {
            maps: ordered,
            params: params.to_vec(),
            param_cols,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LotmDocument {
    pub meshes: Vec<Vec<u8>>,
    pub materials: Vec<LotmMaterial>,
    /// Per-mesh material index, paired with `uv_kind` (2 for native export).
    pub mesh_materials: Vec<(u32, u8)>,
    pub diagnostics: String,
}

pub fn encode(document: &LotmDocument) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"LOTM");
    out.extend_from_slice(&9u32.to_le_bytes());
    out.extend_from_slice(&(document.meshes.len() as u32).to_le_bytes());
    for mesh in &document.meshes {
        out.extend_from_slice(&(mesh.len() as u32).to_le_bytes());
        out.extend_from_slice(mesh);
    }
    out.extend_from_slice(&(document.materials.len() as u32).to_le_bytes());
    for material in &document.materials {
        for map in &material.maps {
            out.extend_from_slice(&(map.len() as u32).to_le_bytes());
            out.extend_from_slice(map);
        }
        let params: Vec<u8> = material
            .params
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        out.extend_from_slice(&(params.len() as u32).to_le_bytes());
        out.extend_from_slice(&params);
        out.extend_from_slice(&material.param_cols.to_le_bytes());
    }
    for (material, uv_kind) in &document.mesh_materials {
        out.extend_from_slice(&material.to_le_bytes());
        out.push(*uv_kind);
    }
    out.extend_from_slice(&(document.diagnostics.len() as u32).to_le_bytes());
    out.extend_from_slice(document.diagnostics.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_layout_round_trips_counts() {
        let document = LotmDocument {
            meshes: vec![b"glTF".to_vec()],
            materials: vec![LotmMaterial::from_recipe(
                [b"t".to_vec(), b"n".to_vec(), b"s".to_vec(), b"p".to_vec(), b"i".to_vec()],
                [0.0; 16],
                1,
            )],
            mesh_materials: vec![(0, 2)],
            diagnostics: "{}".into(),
        };
        let bytes = encode(&document);
        assert_eq!(&bytes[..4], b"LOTM");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 9);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
    }
}
