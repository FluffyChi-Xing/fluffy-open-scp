//! Parser for the workbench LOTM envelope.
//!
//! Two on-disk shapes exist:
//! - Workbench containers (v8/v9/v10, mirrors the frontend
//!   `parseLotModelContainer`): `magic|version|mesh_count|per-mesh GLB…|
//!   material_count|per-material [PNGs] paramsF32 paramCols|per-mesh
//!   (material_index u32, uv_kind u8)|diag_len|diag`. PNG count is 8 for v9
//!   and 9 for v8/v10 (v8's 9th is the retired relief map, v10's is the raw
//!   slot0 diffuse).
//! - Legacy single-GLB envelopes (any other version):
//!   `magic|version|metadata|glb_len|GLB`.
//!
//! The v1 compiler misread containers as legacy envelopes — `mesh_count`
//! became "metadata" and only the first mesh GLB was compiled, which is how
//! built packages lost all but one mesh (user report 2026-10-30).

#[derive(Debug, Clone, PartialEq)]
pub struct LotmMaterial {
    /// Decoded slot PNGs in container order: base, normal, roughness, ao,
    /// tint, palette, shader, interior, then v10 slot0 / v8 relief.
    pub pngs: Vec<Option<Vec<u8>>>,
    /// Raw f32 parameter table bytes (row-major param_cols × 4 floats).
    pub params_f32: Option<Vec<u8>>,
    pub param_cols: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshMaterialRef {
    pub material: u32,
    pub uv_kind: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LotmDocument {
    pub version: u32,
    pub meshes: Vec<Vec<u8>>,
    pub materials: Vec<LotmMaterial>,
    /// Per-mesh material binding; defaults to material 0 when absent.
    pub mesh_material: Vec<MeshMaterialRef>,
    pub diagnostics: String,
}

impl LotmMaterial {
    /// PNG index constants within [`LotmMaterial::pngs`].  Not every slot is
    /// consumed by the resolver; the constants document the container order.
    pub const PNG_BASE: usize = 0;
    pub const PNG_NORMAL: usize = 1;
    pub const PNG_ROUGHNESS: usize = 2;
    #[allow(dead_code)]
    pub const PNG_AO: usize = 3;
    pub const PNG_TINT: usize = 4;
    pub const PNG_PALETTE: usize = 5;
    pub const PNG_SHADER: usize = 6;
    pub const PNG_INTERIOR: usize = 7;
    pub const PNG_SLOT0: usize = 8;
}

/// Cheap shape probe: does this file use the v8/v9/v10 container layout
/// (as opposed to a legacy single-GLB envelope or a bare GLB)?
pub fn is_container(bytes: &[u8]) -> bool {
    bytes.len() >= 8
        && &bytes[..4] == b"LOTM"
        && (8..=10).contains(&u32::from_le_bytes(bytes[4..8].try_into().unwrap()))
}

pub fn parse(bytes: &[u8]) -> Result<LotmDocument, String> {
    if bytes.len() < 12 || &bytes[..4] != b"LOTM" {
        return Err("invalid LOTM header".into());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if (8..=10).contains(&version) {
        parse_container(bytes, version)
    } else {
        parse_legacy_envelope(bytes, version)
    }
}

fn parse_container(bytes: &[u8], version: u32) -> Result<LotmDocument, String> {
    let pngs_per_material = if version == 9 { 8usize } else { 9usize };
    // magic(0..4) + version(4..8) already consumed; mesh_count lives at 8..12.
    let mut cursor = 8usize;
    let mesh_count = take_u32(bytes, &mut cursor, "mesh count")? as usize;
    let mut meshes = Vec::with_capacity(mesh_count);
    for _ in 0..mesh_count {
        let len = take_u32(bytes, &mut cursor, "mesh length")? as usize;
        let glb = take_bytes(bytes, &mut cursor, len, "mesh payload")?.to_vec();
        if glb.len() < 12 || &glb[..4] != b"glTF" {
            return Err("LOTM mesh payload is not a GLB document".into());
        }
        meshes.push(glb);
    }
    let material_count = take_u32(bytes, &mut cursor, "material count")? as usize;
    let mut materials = Vec::with_capacity(material_count);
    for _ in 0..material_count {
        let mut pngs = Vec::with_capacity(pngs_per_material);
        for _ in 0..pngs_per_material {
            let len = take_u32(bytes, &mut cursor, "texture length")? as usize;
            let png = take_bytes(bytes, &mut cursor, len, "texture payload")?;
            pngs.push((!png.is_empty()).then(|| png.to_vec()));
        }
        let params_len = take_u32(bytes, &mut cursor, "params length")? as usize;
        let params_f32 = take_bytes(bytes, &mut cursor, params_len, "params payload")?;
        materials.push(LotmMaterial {
            pngs,
            params_f32: (!params_f32.is_empty()).then(|| params_f32.to_vec()),
            param_cols: take_u32(bytes, &mut cursor, "param cols")?,
        });
    }
    let mut mesh_material = Vec::with_capacity(mesh_count);
    for _ in 0..mesh_count {
        mesh_material.push(MeshMaterialRef {
            material: take_u32(bytes, &mut cursor, "mesh material")?,
            uv_kind: take_bytes(bytes, &mut cursor, 1, "mesh uv kind")?[0],
        });
    }
    let diag_len = take_u32(bytes, &mut cursor, "diagnostics length")? as usize;
    let diagnostics =
        String::from_utf8_lossy(take_bytes(bytes, &mut cursor, diag_len, "diagnostics")?)
            .into_owned();
    LotmDocument {
        version,
        meshes,
        materials,
        mesh_material,
        diagnostics,
    }
    .validate()
}

fn take_u32(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u32, String> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| format!("LOTM {label} is truncated"))?;
    if end > bytes.len() {
        return Err(format!("LOTM {label} is truncated"));
    }
    let value = u32::from_le_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn take_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| format!("LOTM {label} is truncated"))?;
    if end > bytes.len() {
        return Err(format!("LOTM {label} is truncated"));
    }
    let slice = &bytes[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn parse_legacy_envelope(bytes: &[u8], version: u32) -> Result<LotmDocument, String> {
    if bytes.len() < 28 {
        return Err("invalid LOTM header".into());
    }
    let glb_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if glb_len < 12
        || 16usize
            .checked_add(glb_len)
            .is_none_or(|end| end > bytes.len())
    {
        return Err("LOTM GLB payload is truncated".into());
    }
    let glb = bytes[16..16 + glb_len].to_vec();
    if &glb[..4] != b"glTF" || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2 {
        return Err("LOTM payload is not a GLB v2 document".into());
    }
    let declared = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if declared != glb.len() {
        return Err(format!(
            "GLB length mismatch: header {declared}, actual {}",
            glb.len()
        ));
    }
    LotmDocument {
        version,
        meshes: vec![glb],
        materials: Vec::new(),
        mesh_material: vec![MeshMaterialRef {
            material: 0,
            uv_kind: 0,
        }],
        diagnostics: String::new(),
    }
    .validate()
}

impl LotmDocument {
    fn validate(self) -> Result<Self, String> {
        if self.meshes.is_empty() {
            return Err("LOTM container has no meshes".into());
        }
        for r in &self.mesh_material {
            if self.materials.is_empty() {
                if r.material != 0 {
                    return Err(format!(
                        "LOTM mesh references material {} but the container has none",
                        r.material
                    ));
                }
            } else if r.material as usize >= self.materials.len() {
                return Err(format!(
                    "LOTM mesh material {} out of range ({})",
                    r.material,
                    self.materials.len()
                ));
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container(version: u32, mesh_count: usize) -> Vec<u8> {
        let mut bytes = b"LOTM".to_vec();
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(mesh_count as u32).to_le_bytes());
        for index in 0..mesh_count {
            let mut glb = b"glTF".to_vec();
            glb.extend_from_slice(&2u32.to_le_bytes());
            glb.extend_from_slice(&12u32.to_le_bytes());
            glb.push(index as u8);
            bytes.extend_from_slice(&(glb.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&glb);
        }
        bytes.extend_from_slice(&1u32.to_le_bytes()); // material_count
        let png_count = if version == 9 { 8 } else { 9 };
        for _ in 0..png_count {
            bytes.extend_from_slice(&0u32.to_le_bytes()); // empty png lengths
        }
        bytes.extend_from_slice(&4u32.to_le_bytes()); // params length
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes()); // param cols
        for _ in 0..mesh_count {
            bytes.extend_from_slice(&0u32.to_le_bytes()); // material index
            bytes.push(2); // uv kind
        }
        bytes.extend_from_slice(&0u32.to_le_bytes()); // diag length
        bytes
    }

    #[test]
    fn parses_v9_container_with_all_meshes_and_materials() {
        let doc = parse(&container(9, 3)).unwrap();
        assert_eq!(doc.version, 9);
        assert_eq!(doc.meshes.len(), 3);
        assert_eq!(doc.materials.len(), 1);
        assert_eq!(doc.materials[0].pngs.len(), 8);
        assert_eq!(doc.materials[0].params_f32.as_deref().unwrap().len(), 4);
        assert_eq!(doc.mesh_material.len(), 3);
        assert_eq!(doc.mesh_material[2].uv_kind, 2);
    }

    #[test]
    fn v10_carries_nine_pngs_per_material() {
        let doc = parse(&container(10, 1)).unwrap();
        assert_eq!(doc.materials[0].pngs.len(), 9);
    }

    #[test]
    fn mesh_material_out_of_range_is_rejected() {
        let mut bytes = container(9, 1);
        // layout tail: [material u32][uv u8][diag_len u32]
        let material_at = bytes.len() - 9;
        bytes[material_at..material_at + 4].copy_from_slice(&7u32.to_le_bytes());
        let error = parse(&bytes).unwrap_err();
        assert!(error.contains("out of range"), "{error}");
    }

    #[test]
    fn extracts_and_validates_embedded_legacy_glb() {
        let mut bytes = b"LOTM".to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0x4Au32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.version, 1);
        assert_eq!(doc.meshes.len(), 1);
        assert_eq!(doc.meshes[0], bytes[16..]);
    }
}
