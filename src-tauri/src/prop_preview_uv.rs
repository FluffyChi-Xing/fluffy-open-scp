//! Vertex declarations alone cannot identify building tint shaders.
//! DLC airship BF45A31D has COLOR + FLOAT4, but slot0 is DXT1 diffuse.
pub(super) fn prop_preview_uv_kind(mesh: &rw4::DecodedMesh, has_params: bool) -> u8 {
    let kind = super::mesh_uv_kind(mesh);
    if kind == 2 && !has_params {
        u8::from(mesh.vertices.iter().any(|vertex| vertex.uv().is_some()))
    } else {
        kind
    }
}
