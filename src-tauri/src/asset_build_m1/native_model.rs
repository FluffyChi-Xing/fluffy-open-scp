//! Workbench LOTM/GLB to a small native RW4 static model.
//!
//! The workbench stores editable geometry as a LOTM envelope containing a
//! standard glTF 2.0 binary.  This module deliberately keeps the first native
//! lowering small and deterministic: one or more triangle primitives, with
//! POSITION and optional NORMAL/TEXCOORD_0 attributes, are emitted as a static
//! RW4 mesh.  Skinning and morph targets remain outside this lowering.

use super::lotm::parse as parse_lotm;
use rw4::{Rw4File, SectionType, build_file, writer_section};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct GlbPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledNativeModel {
    pub rw4: Vec<u8>,
    pub primitive_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
}

pub fn compile_lotm_static(bytes: &[u8]) -> Result<CompiledNativeModel, String> {
    let glb = parse_lotm(bytes)?.glb;
    let primitives = parse_glb_primitives(&glb)?;
    compile_primitives_with_textures(&primitives, &[0; 6])
}

pub fn compile_lotm_static_with_textures(
    bytes: &[u8],
    texture_instances: &[u32; 6],
) -> Result<CompiledNativeModel, String> {
    let glb = parse_lotm(bytes)?.glb;
    let primitives = parse_glb_primitives(&glb)?;
    compile_primitives_with_textures(&primitives, texture_instances)
}

pub fn parse_glb_primitives(glb: &[u8]) -> Result<Vec<GlbPrimitive>, String> {
    if glb.len() < 12 || &glb[..4] != b"glTF" {
        return Err("invalid GLB header".into());
    }
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported GLB version {version}"));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total > glb.len() || total < 20 {
        return Err("GLB length is truncated".into());
    }
    let mut cursor = 12usize;
    let mut json = None;
    let mut bin = None;
    while cursor + 8 <= total {
        let len = u32::from_le_bytes(glb[cursor..cursor + 4].try_into().unwrap()) as usize;
        let kind = u32::from_le_bytes(glb[cursor + 4..cursor + 8].try_into().unwrap());
        cursor = cursor
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .ok_or("GLB chunk overflow")?;
        if cursor > total {
            return Err("GLB chunk is truncated".into());
        }
        let start = cursor - len;
        match kind {
            0x4E4F534A => json = Some(&glb[start..cursor]),
            0x004E4942 => bin = Some(&glb[start..cursor]),
            _ => {}
        }
    }
    let document: Value = serde_json::from_slice(json.ok_or("GLB JSON chunk missing")?)
        .map_err(|e| format!("invalid GLB JSON: {e}"))?;
    let bin = bin.ok_or("GLB BIN chunk missing")?;
    let buffers = document
        .get("buffers")
        .and_then(Value::as_array)
        .ok_or("GLB buffers missing")?;
    if buffers.is_empty() {
        return Err("GLB has no buffers".into());
    }
    let views = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or("GLB bufferViews missing")?;
    let accessors = document
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or("GLB accessors missing")?;
    let meshes = document
        .get("meshes")
        .and_then(Value::as_array)
        .ok_or("GLB meshes missing")?;
    let mut out = Vec::new();
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or("mesh primitives missing")?;
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err("only triangle GLB primitives are supported".into());
            }
            let attrs = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .ok_or("primitive attributes missing")?;
            let position_accessor = attrs
                .get("POSITION")
                .and_then(Value::as_u64)
                .ok_or("primitive POSITION missing")? as usize;
            let positions = read_vec3(accessors, views, bin, position_accessor, "POSITION")?;
            let normals = attrs
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|i| read_vec3(accessors, views, bin, i as usize, "NORMAL"))
                .transpose()?
                .unwrap_or_default();
            let uvs = attrs
                .get("TEXCOORD_0")
                .and_then(Value::as_u64)
                .map(|i| read_vec2(accessors, views, bin, i as usize, "TEXCOORD_0"))
                .transpose()?
                .unwrap_or_default();
            let indices = if let Some(i) = primitive.get("indices").and_then(Value::as_u64) {
                read_indices(accessors, views, bin, i as usize)?
            } else {
                (0..positions.len() as u32).collect()
            };
            if positions.is_empty() || indices.len() % 3 != 0 {
                return Err("primitive has no vertices or non-triangle indices".into());
            }
            if normals.len() != 0 && normals.len() != positions.len()
                || uvs.len() != 0 && uvs.len() != positions.len()
            {
                return Err("attribute count does not match POSITION".into());
            }
            out.push(GlbPrimitive {
                positions,
                normals,
                uvs,
                indices,
                material_index: primitive
                    .get("material")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
            });
        }
    }
    if out.is_empty() {
        return Err("GLB has no mesh primitives".into());
    }
    Ok(out)
}

fn accessor_slice<'a>(
    accessors: &[Value],
    views: &[Value],
    bin: &'a [u8],
    index: usize,
    label: &str,
) -> Result<(&'a [u8], usize, usize), String> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| format!("{label} accessor out of range"))?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} bufferView missing"))? as usize;
    let view = views
        .get(view_index)
        .ok_or_else(|| format!("{label} bufferView out of range"))?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err(format!("{label} uses unsupported external buffer"));
    }
    let component = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} componentType missing"))? as u32;
    let ty = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} type missing"))?;
    let components = match ty {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        _ => return Err(format!("{label} type {ty} unsupported")),
    };
    let component_size = match component {
        5126 | 5125 => 4,
        5123 => 2,
        _ => return Err(format!("{label} component type {component} unsupported")),
    };
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("accessor count missing")? as usize;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or((component_size * components) as u64) as usize;
    let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    let need = start
        .checked_add(stride.checked_mul(count.saturating_sub(1)).unwrap_or(0))
        .and_then(|v| v.checked_add(component_size * components))
        .ok_or("accessor overflow")?;
    if need > bin.len() {
        return Err(format!("{label} data truncated"));
    }
    Ok((&bin[start..need], stride, component as usize))
}

fn read_vec3(
    accessors: &[Value],
    views: &[Value],
    bin: &[u8],
    index: usize,
    label: &str,
) -> Result<Vec<[f32; 3]>, String> {
    let (data, stride, component) = accessor_slice(accessors, views, bin, index, label)?;
    if component != 5126 {
        return Err(format!("{label} must use FLOAT"));
    }
    let count = accessors[index]
        .get("count")
        .and_then(Value::as_u64)
        .unwrap() as usize;
    (0..count)
        .map(|i| {
            let p = i * stride;
            Ok([
                f32::from_le_bytes(data[p..p + 4].try_into().unwrap()),
                f32::from_le_bytes(data[p + 4..p + 8].try_into().unwrap()),
                f32::from_le_bytes(data[p + 8..p + 12].try_into().unwrap()),
            ])
        })
        .collect()
}

fn read_vec2(
    accessors: &[Value],
    views: &[Value],
    bin: &[u8],
    index: usize,
    label: &str,
) -> Result<Vec<[f32; 2]>, String> {
    let (data, stride, component) = accessor_slice(accessors, views, bin, index, label)?;
    if component != 5126 {
        return Err(format!("{label} must use FLOAT"));
    }
    let count = accessors[index]
        .get("count")
        .and_then(Value::as_u64)
        .unwrap() as usize;
    (0..count)
        .map(|i| {
            let p = i * stride;
            Ok([
                f32::from_le_bytes(data[p..p + 4].try_into().unwrap()),
                f32::from_le_bytes(data[p + 4..p + 8].try_into().unwrap()),
            ])
        })
        .collect()
}

fn read_indices(
    accessors: &[Value],
    views: &[Value],
    bin: &[u8],
    index: usize,
) -> Result<Vec<u32>, String> {
    let (data, stride, component) = accessor_slice(accessors, views, bin, index, "indices")?;
    let count = accessors[index]
        .get("count")
        .and_then(Value::as_u64)
        .unwrap() as usize;
    (0..count)
        .map(|i| {
            let p = i * stride;
            Ok(match component {
                5123 => u16::from_le_bytes(data[p..p + 2].try_into().unwrap()) as u32,
                5125 => u32::from_le_bytes(data[p..p + 4].try_into().unwrap()),
                _ => return Err("indices must use UNSIGNED_SHORT or UNSIGNED_INT".into()),
            })
        })
        .collect()
}

pub fn compile_primitives(primitives: &[GlbPrimitive]) -> Result<CompiledNativeModel, String> {
    compile_primitives_with_textures(primitives, &[0; 6])
}

pub fn compile_primitives_with_textures(
    primitives: &[GlbPrimitive],
    texture_instances: &[u32; 6],
) -> Result<CompiledNativeModel, String> {
    if primitives.is_empty() {
        return Err("no primitive".into());
    }
    let vertex_count: usize = primitives.iter().map(|p| p.positions.len()).sum();
    let index_count: usize = primitives.iter().map(|p| p.indices.len()).sum();
    let fmt = vertex_format_payload();
    let mut specs = vec![writer_section(SectionType::VERTEX_FORMAT, fmt.clone())];
    let mut mesh_numbers = Vec::new();
    let mut material_numbers = Vec::new();
    for (primitive_index, primitive) in primitives.iter().enumerate() {
        let mut vertices = Vec::with_capacity(primitive.positions.len() * 32);
        for i in 0..primitive.positions.len() {
            for value in primitive.positions[i] {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            for value in primitive.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]) {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            for value in primitive.uvs.get(i).copied().unwrap_or([0.0, 0.0]) {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut indices = Vec::with_capacity(primitive.indices.len() * 2);
        for &index in &primitive.indices {
            let value = u16::try_from(index)
                .map_err(|_| "RW4 static mesh supports at most 65535 vertices")?;
            indices.extend_from_slice(&value.to_le_bytes());
        }
        let va = specs.len() as u32;
        specs.push(writer_section(
            SectionType::VERTEX_ARRAY,
            vertex_array_payload(0, va + 1, primitive.positions.len() as u32, 32),
        ));
        specs.push(writer_section(SectionType::BLOB, vertices));
        let index_blob = specs.len() as u32;
        specs.push(writer_section(SectionType::BLOB, indices));
        let ta = specs.len() as u32;
        specs.push(writer_section(
            SectionType::TRIANGLE_ARRAY,
            triangle_array_payload(index_blob, primitive.indices.len() as u32),
        ));
        let mesh = specs.len() as u32;
        specs.push(writer_section(
            SectionType::MESH,
            mesh_payload(
                ta,
                (primitive.indices.len() / 3) as u32,
                primitive.positions.len() as u32,
                va,
            ),
        ));
        let mat = specs.len() as u32;
        specs.push(writer_section(
            SectionType::MATERIAL,
            material_payload(&fmt, texture_instances),
        ));
        specs.push(writer_section(
            SectionType::MESH_MATERIAL_ASSIGNMENT,
            assignment_payload(mesh, mat),
        ));
        mesh_numbers.push(mesh);
        material_numbers.push((primitive_index, mat));
    }
    let rw4 = build_file(1, &specs, &[]);
    let file = Rw4File::parse(&rw4).map_err(|e| format!("compiled RW4 parse failed: {e}"))?;
    let mut decoded_vertices = 0;
    let mut decoded_indices = 0;
    for mesh in mesh_numbers {
        let decoded = file
            .decode_mesh(&rw4, mesh)
            .map_err(|e| format!("compiled RW4 mesh failed: {e}"))?;
        decoded_vertices += decoded.vertices.len();
        decoded_indices += decoded.triangles.len() * 3;
    }
    if decoded_vertices != vertex_count || decoded_indices != index_count {
        return Err("compiled RW4 round-trip count mismatch".into());
    }
    Ok(CompiledNativeModel {
        rw4,
        primitive_count: primitives.len(),
        vertex_count: decoded_vertices,
        index_count,
    })
}

fn vertex_format_payload() -> Vec<u8> {
    let mut p = vec![0; 12];
    p.extend_from_slice(&3u16.to_le_bytes());
    p.extend_from_slice(&32u16.to_be_bytes());
    p.extend_from_slice(&[0; 8]);
    for (offset, ty, usage) in [(0u16, 2u16, 0u16), (12, 2, 3), (24, 1, 5)] {
        p.push(0);
        p.extend_from_slice(&offset.to_be_bytes());
        p.extend_from_slice(&ty.to_be_bytes());
        p.extend_from_slice(&usage.to_be_bytes());
        p.extend_from_slice(&[0, 0]);
        p.extend_from_slice(&0u16.to_be_bytes());
        p.push(0);
    }
    p
}
fn vertex_array_payload(fmt: u32, data: u32, count: u32, size: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&(fmt as i32).to_le_bytes());
    p.extend_from_slice(&[0; 8]);
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&8u32.to_le_bytes());
    p.extend_from_slice(&size.to_le_bytes());
    p.extend_from_slice(&(data as i32).to_le_bytes());
    p
}
fn triangle_array_payload(data: u32, count: u32) -> Vec<u8> {
    let mut p = vec![0; 8];
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&8u32.to_le_bytes());
    p.extend_from_slice(&101u32.to_le_bytes());
    p.extend_from_slice(&4u32.to_le_bytes());
    p.extend_from_slice(&(data as i32).to_le_bytes());
    p
}
fn mesh_payload(tri: u32, count: u32, verts: u32, vertex: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&40u32.to_le_bytes());
    p.extend_from_slice(&4u32.to_le_bytes());
    p.extend_from_slice(&(tri as i32).to_le_bytes());
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&1u32.to_le_bytes());
    p.extend_from_slice(&0u32.to_le_bytes());
    p.extend_from_slice(&(count * 3).to_le_bytes());
    p.extend_from_slice(&0u32.to_le_bytes());
    p.extend_from_slice(&verts.to_le_bytes());
    p.extend_from_slice(&(vertex as i32).to_le_bytes());
    p
}
fn material_payload(fmt: &[u8], texture_instances: &[u32; 6]) -> Vec<u8> {
    let size = 4 + 28 + 24 + fmt.len() + 7 * 24;
    let mut p = Vec::new();
    p.extend_from_slice(&(size as u32).to_le_bytes());
    p.extend_from_slice(&[0; 28]);
    p.extend_from_slice(fmt);
    p.extend_from_slice(&[0; 24]);
    for (slot, texture) in [
        (0x2d, 0),
        (0, texture_instances[0]),
        (1, texture_instances[1]),
        (2, texture_instances[2]),
        (3, texture_instances[3]),
        (4, texture_instances[4]),
        (5, texture_instances[5]),
    ] {
        p.extend_from_slice(&(slot as u32).to_le_bytes());
        p.extend_from_slice(&[0; 8]);
        p.extend_from_slice(&texture.to_le_bytes());
        p.extend_from_slice(&[0; 8]);
    }
    p
}
fn assignment_payload(mesh: u32, material: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&mesh.to_le_bytes());
    p.extend_from_slice(&1u32.to_le_bytes());
    p.extend_from_slice(&material.to_le_bytes());
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiles_static_triangle_to_parseable_rw4() {
        let primitive = GlbPrimitive {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 1, 2],
            material_index: 0,
        };
        let model = compile_primitives(&[primitive.clone(), primitive]).unwrap();
        let file = Rw4File::parse(&model.rw4).unwrap();
        assert_eq!(
            file.decode_mesh(&model.rw4, 5).unwrap().triangles,
            vec![[0, 1, 2]]
        );
        assert_eq!(
            file.decode_mesh(&model.rw4, 12).unwrap().triangles,
            vec![[0, 1, 2]]
        );
        assert_eq!(file.decode_mesh(&model.rw4, 5).unwrap().vertices.len(), 3);
        assert_eq!(file.decode_mesh(&model.rw4, 12).unwrap().vertices.len(), 3);
        let material = file.decode_material(&model.rw4, 6).unwrap();
        assert_eq!(material.texture_refs()[1].texture_instance, 0);
    }
}
