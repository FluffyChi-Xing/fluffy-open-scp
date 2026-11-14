//! Rewrite facade UVs in a LOTM's GLB meshes so a single atlas material can
//! sample each source material's own cell.  Only float payloads are mutated;
//! GLB chunk lengths stay unchanged.

use super::lotm::LotmDocument;
use serde_json::Value;

const GLB_JSON: u32 = 0x4E4F_534A;
const GLB_BIN: u32 = 0x004E_4942;

pub fn remap_facade_uvs(
    doc: &LotmDocument,
    mesh_cells: &[[f32; 4]],
) -> Result<LotmDocument, String> {
    if mesh_cells.len() < doc.meshes.len() {
        return Err(format!(
            "atlas UV remap covers {} of {} meshes",
            mesh_cells.len(),
            doc.meshes.len()
        ));
    }
    let mut out = doc.clone();
    for (index, glb) in out.meshes.iter_mut().enumerate() {
        patch_glb(glb, mesh_cells[index])?;
    }
    Ok(out)
}

fn patch_glb(glb: &mut [u8], cell: [f32; 4]) -> Result<(), String> {
    if glb.len() < 20 || &glb[..4] != b"glTF" {
        return Err("atlas UV remap: not a GLB".into());
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json_type = u32::from_le_bytes(glb[16..20].try_into().unwrap());
    if json_type != GLB_JSON || 20 + json_len > glb.len() {
        return Err("atlas UV remap: JSON chunk missing".into());
    }
    let json: Value = serde_json::from_slice(&glb[20..20 + json_len])
        .map_err(|error| format!("atlas UV remap: GLB JSON: {error}"))?;
    let bin_header = 20 + json_len;
    if bin_header + 8 > glb.len() {
        return Err("atlas UV remap: BIN chunk missing".into());
    }
    let bin_len = u32::from_le_bytes(glb[bin_header..bin_header + 4].try_into().unwrap()) as usize;
    let bin_type = u32::from_le_bytes(glb[bin_header + 4..bin_header + 8].try_into().unwrap());
    if bin_type != GLB_BIN || bin_header + 8 + bin_len > glb.len() {
        return Err("atlas UV remap: BIN chunk malformed".into());
    }
    let bin_start = bin_header + 8;
    let views = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or("atlas UV remap: bufferViews missing")?;
    let accessors = json
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or("atlas UV remap: accessors missing")?;
    let meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or("atlas UV remap: meshes missing")?;
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or("atlas UV remap: primitives missing")?;
        for primitive in primitives {
            let attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or("atlas UV remap: attributes missing")?;
            for semantic in ["TEXCOORD_2", "TEXCOORD_3"] {
                if let Some(index) = attributes.get(semantic).and_then(Value::as_u64) {
                    patch_accessor(glb, bin_start, views, accessors, index as usize, cell)?;
                }
            }
        }
    }
    Ok(())
}

fn patch_accessor(
    glb: &mut [u8],
    bin_start: usize,
    views: &[Value],
    accessors: &[Value],
    index: usize,
    cell: [f32; 4],
) -> Result<(), String> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| format!("atlas UV remap: accessor {index} missing"))?;
    if accessor.get("type").and_then(Value::as_str) != Some("VEC2")
        || accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
    {
        return Ok(());
    }
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or("atlas UV remap: accessor bufferView missing")? as usize;
    let view = views
        .get(view_index)
        .ok_or("atlas UV remap: bufferView missing")?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(8) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("atlas UV remap: accessor count missing")? as usize;
    let start = bin_start + view_offset + accessor_offset;
    for vertex in 0..count {
        let at = start + vertex * stride;
        if at + 8 > glb.len() {
            return Err("atlas UV remap: accessor out of range".into());
        }
        for (slot, component) in [(0usize, 0usize), (1, 1)] {
            let offset = at + slot * 4;
            let value = f32::from_le_bytes(glb[offset..offset + 4].try_into().unwrap());
            let transformed = fract(value) * cell[component] + cell[component + 2];
            glb[offset..offset + 4].copy_from_slice(&transformed.to_le_bytes());
        }
    }
    Ok(())
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}
