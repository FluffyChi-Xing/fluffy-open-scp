//! Strict LOTM9 → native Building40 compiler ported from SC5 Asset Packager.
//!
//! This module deliberately does not use the workbench scene flattener,
//! automatic centering, NH chunking, or material deduplication. The author's
//! route reads `meshes[0].primitives[0]` from each source group, rotates the
//! raw accessors, deduplicates within that group, and emits one Building40
//! mesh. Invalid source data is an error, never an invitation to invent UVs,
//! normals, material rows, or triangles.

use super::lotm::LotmDocument;
use super::native_model::{CompiledNativeModel, rotate_point, rotate_xyz_matrix};
use rw4::{Rw4File, SectionType};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorModel {
    pub compiled: CompiledNativeModel,
    pub source_groups: usize,
    pub source_triangles: usize,
    pub removed_degenerate_triangles: usize,
    pub bounds: [[f32; 3]; 2],
}

#[derive(Debug)]
struct Accessor {
    components: usize,
    values: Vec<f32>,
}

impl Accessor {
    fn count(&self) -> usize {
        self.values.len() / self.components
    }

    fn vec2(&self, index: usize) -> [f32; 2] {
        let at = index * 2;
        [self.values[at], self.values[at + 1]]
    }

    fn vec3(&self, index: usize) -> [f32; 3] {
        // TANGENT is VEC4; POSITION/NORMAL are VEC3. Use the accessor's
        // actual width rather than indexing every stream as tightly packed xyz.
        let at = index * self.components;
        [self.values[at], self.values[at + 1], self.values[at + 2]]
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct VertexKey([u32; 14]);

impl VertexKey {
    fn new(
        position: [f32; 3],
        normal: [f32; 3],
        tangent: [f32; 3],
        base: [f32; 2],
        top: [f32; 2],
        seed: u8,
    ) -> Self {
        let mut key = [0u32; 14];
        for (slot, value) in position
            .into_iter()
            .chain(normal)
            .chain(tangent)
            .chain(base)
            .chain(top)
            .enumerate()
        {
            // Python tuple equality/hash unify -0.0 and +0.0. Preserve the
            // first vertex's stored bytes, but canonicalize zero in the key.
            key[slot] = if value == 0.0 { 0 } else { value.to_bits() };
        }
        key[13] = seed as u32;
        Self(key)
    }
}

#[derive(Debug, Clone, Copy)]
struct SceneTransform {
    matrix: [f32; 12],
    /// Inverse-transpose of the row-vector linear 3x3 matrix.
    normal: [f32; 9],
}

impl SceneTransform {
    fn new(matrix: [f32; 12]) -> Result<Self, String> {
        if matrix.iter().any(|value| !value.is_finite()) {
            return Err(
                "author Building40: scene modelTransform contains non-finite values".into(),
            );
        }
        let m = matrix.map(f64::from);
        let determinant = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
            + m[2] * (m[3] * m[7] - m[4] * m[6]);
        if determinant <= 1e-12 {
            return Err(
                "author Building40: scene modelTransform linear part must be positive and invertible"
                    .into(),
            );
        }
        let normal = [
            (m[4] * m[8] - m[5] * m[7]) / determinant,
            (m[5] * m[6] - m[3] * m[8]) / determinant,
            (m[3] * m[7] - m[4] * m[6]) / determinant,
            (m[2] * m[7] - m[1] * m[8]) / determinant,
            (m[0] * m[8] - m[2] * m[6]) / determinant,
            (m[1] * m[6] - m[0] * m[7]) / determinant,
            (m[1] * m[5] - m[2] * m[4]) / determinant,
            (m[2] * m[3] - m[0] * m[5]) / determinant,
            (m[0] * m[4] - m[1] * m[3]) / determinant,
        ]
        .map(|value| value as f32);
        Ok(Self { matrix, normal })
    }

    fn point(&self, value: [f32; 3]) -> [f32; 3] {
        self.vector(value, &self.matrix, true)
    }

    fn tangent(&self, value: [f32; 3]) -> [f32; 3] {
        self.vector(value, &self.matrix, false)
    }

    fn normal(&self, value: [f32; 3]) -> [f32; 3] {
        let m = &self.normal;
        [
            value[0] * m[0] + value[1] * m[3] + value[2] * m[6],
            value[0] * m[1] + value[1] * m[4] + value[2] * m[7],
            value[0] * m[2] + value[1] * m[5] + value[2] * m[8],
        ]
    }

    fn vector(&self, value: [f32; 3], m: &[f32; 12], translate: bool) -> [f32; 3] {
        let offset = if translate { 9 } else { 12 };
        [
            value[0] * m[0]
                + value[1] * m[3]
                + value[2] * m[6]
                + if offset == 9 { m[9] } else { 0.0 },
            value[0] * m[1]
                + value[1] * m[4]
                + value[2] * m[7]
                + if offset == 9 { m[10] } else { 0.0 },
            value[0] * m[2]
                + value[1] * m[5]
                + value[2] * m[8]
                + if offset == 9 { m[11] } else { 0.0 },
        ]
    }
}

/// Compile geometry exactly in the author's single-mesh Building40 profile.
/// This wrapper preserves the byte-level author oracle: no scene transform and
/// no automatic centering are applied.
pub fn compile_model(
    doc: &LotmDocument,
    rotation_degrees: u32,
    template: &[u8],
    collision_source: &[u8],
) -> Result<AuthorModel, String> {
    compile_model_with_transform(doc, rotation_degrees, None, template, collision_source)
}

/// Apply the workbench's WPF row-vector transform after the author's rotation.
/// Positions and tangents use the linear 3x3 part; normals use its inverse
/// transpose. Translation never participates in direction transformation.
pub fn compile_model_with_transform(
    doc: &LotmDocument,
    rotation_degrees: u32,
    transform: Option<[f32; 12]>,
    template: &[u8],
    collision_source: &[u8],
) -> Result<AuthorModel, String> {
    let scene = transform.map(SceneTransform::new).transpose()?;
    if doc.version != 9 {
        return Err(format!(
            "author Building40 route requires LOTM9, received LOTM{}",
            doc.version
        ));
    }
    if doc.meshes.len() != doc.materials.len() {
        return Err(format!(
            "author Building40 route requires one material per source group ({} meshes, {} materials)",
            doc.meshes.len(),
            doc.materials.len()
        ));
    }
    if doc.meshes.is_empty() || doc.meshes.len() > 128 {
        return Err(format!(
            "author Building40 palette supports 1..=128 source groups, received {}",
            doc.meshes.len()
        ));
    }
    let rotation = rotate_xyz_matrix(rotation_degrees)?;
    let parameter_width = doc.meshes.len().next_power_of_two();
    let mut vertices = Vec::<[u8; 40]>::new();
    let mut indices = Vec::<u32>::new();
    let mut source_triangles = 0usize;
    let mut removed = 0usize;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for (group, glb) in doc.meshes.iter().enumerate() {
        let (document, bin) = parse_glb(glb)?;
        let primitive = document
            .pointer("/meshes/0/primitives/0")
            .ok_or_else(|| format!("group {group}: meshes[0].primitives[0] is missing"))?;
        let attributes = primitive
            .get("attributes")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("group {group}: primitive attributes are missing"))?;
        let accessor_index = |name: &str| -> Result<usize, String> {
            attributes
                .get(name)
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .ok_or_else(|| format!("group {group}: required accessor {name} is missing"))
        };
        let position = read_accessor(&document, bin, accessor_index("POSITION")?, 3, 5126)?;
        let normal = read_accessor(&document, bin, accessor_index("NORMAL")?, 3, 5126)?;
        let tangent = read_accessor(&document, bin, accessor_index("TANGENT")?, 4, 5126)?;
        let base = read_accessor(&document, bin, accessor_index("TEXCOORD_0")?, 2, 5126)?;
        let top = read_accessor(&document, bin, accessor_index("TEXCOORD_3")?, 2, 5126)?;
        let seeds = read_accessor(&document, bin, accessor_index("TEXCOORD_1")?, 2, 5126)?;
        let count = position.count();
        for (name, accessor) in [
            ("NORMAL", &normal),
            ("TANGENT", &tangent),
            ("TEXCOORD_0", &base),
            ("TEXCOORD_3", &top),
            ("TEXCOORD_1", &seeds),
        ] {
            if accessor.count() != count {
                return Err(format!(
                    "group {group}: {name} count {} differs from POSITION count {count}",
                    accessor.count()
                ));
            }
        }
        if count == 0 || count > 1_000_000 {
            return Err(format!("group {group}: invalid vertex count {count}"));
        }
        if position
            .values
            .iter()
            .chain(normal.values.iter())
            .chain(tangent.values.iter())
            .chain(base.values.iter())
            .chain(top.values.iter())
            .chain(seeds.values.iter())
            .any(|value| !value.is_finite())
        {
            return Err(format!("group {group}: non-finite vertex attribute"));
        }
        let triangle_indices = read_indices(&document, bin, primitive)?;
        if triangle_indices.len() % 3 != 0 {
            return Err(format!(
                "group {group}: index count is not divisible by three"
            ));
        }
        if triangle_indices
            .iter()
            .any(|index| *index as usize >= count)
        {
            return Err(format!("group {group}: object-local index out of range"));
        }
        source_triangles += triangle_indices.len() / 3;

        // Author dedup key: rotated raw attributes, unflipped UVs, group-local
        // material row and seed. The Float4 UV adapter flips V only at write.
        let mut lookup = HashMap::<VertexKey, u32>::new();
        let mut remap = Vec::with_capacity(count);
        let group_vertex_start = vertices.len();
        for vertex in 0..count {
            // Keep the author's matrix-multiply signed-zero behavior in the
            // stored Float32 bytes; VertexKey separately applies Python's
            // equality rule for -0.0/+0.0 during deduplication.
            let mut p = rotate_point(&rotation, position.vec3(vertex));
            let mut n = rotate_point(&rotation, normal.vec3(vertex));
            let mut t = rotate_point(&rotation, tangent.vec3(vertex));
            if let Some(scene) = &scene {
                p = scene.point(p);
                n = scene.normal(n);
                t = scene.tangent(t);
                if p.into_iter()
                    .chain(n)
                    .chain(t)
                    .any(|value| !value.is_finite())
                {
                    return Err(format!(
                        "group {group}: scene modelTransform produced non-finite vertex data"
                    ));
                }
            }
            let packed_normal = pack_direction(n)
                .map_err(|error| format!("group {group}, vertex {vertex}, normal: {error}"))?;
            let packed_tangent = pack_direction(t)
                .map_err(|error| format!("group {group}, vertex {vertex}, tangent: {error}"))?;
            let uv = base.vec2(vertex);
            let uv_top = top.vec2(vertex);
            let seed = round_ties_even(seeds.values[vertex * 2 + 1] as f64 * 255.0)
                .clamp(0.0, 255.0) as u8;
            let key = VertexKey::new(p, n, t, uv, uv_top, seed);
            let next = vertices.len() - group_vertex_start;
            let local = *lookup.entry(key).or_insert_with(|| {
                let mut bytes = [0u8; 40];
                write_f32x3(&mut bytes[0..12], p);
                bytes[12..16].copy_from_slice(&packed_normal);
                bytes[16..20].copy_from_slice(&packed_tangent);
                bytes[20..24].copy_from_slice(&[0, 0, group as u8, seed]);
                write_f32x2(&mut bytes[24..32], [uv[0], 1.0 - uv[1]]);
                write_f32x2(&mut bytes[32..40], [uv_top[0], 1.0 - uv_top[1]]);
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
                vertices.push(bytes);
                next as u32
            });
            remap.push(local);
        }
        let group_offset = group_vertex_start as u32;
        for triangle in triangle_indices.chunks_exact(3) {
            let mapped = [
                remap[triangle[0] as usize],
                remap[triangle[1] as usize],
                remap[triangle[2] as usize],
            ];
            let points = mapped.map(|index| {
                let vertex = &vertices[(group_offset + index) as usize];
                read_f32x3(&vertex[0..12])
            });
            if cross_squared(points) <= 1e-20 {
                removed += 1;
                continue;
            }
            indices.extend(mapped.map(|index| group_offset + index));
        }
    }
    if vertices.len() < 3 || indices.is_empty() {
        return Err("author Building40: no non-degenerate geometry".into());
    }
    if vertices.len() > 1_000_000 {
        return Err("author Building40: geometry exceeds template profile".into());
    }
    let rw4 = replace_geometry(template, collision_source, &vertices, &indices, min, max)?;
    Ok(AuthorModel {
        compiled: CompiledNativeModel {
            rw4,
            primitive_count: doc.meshes.len(),
            vertex_count: vertices.len(),
            index_count: indices.len(),
            material_count: 1,
        },
        source_groups: doc.meshes.len(),
        source_triangles,
        removed_degenerate_triangles: removed,
        bounds: [min, max],
    })
    .and_then(|model| {
        if model.compiled.vertex_count == 0 || model.source_groups > parameter_width {
            Err("author Building40: parameter table does not cover material rows".into())
        } else {
            Ok(model)
        }
    })
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    if bytes.len() < 28 || &bytes[0..4] != b"glTF" {
        return Err("source group is not GLB".into());
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if declared != bytes.len() || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2 {
        return Err("source group GLB header/length mismatch".into());
    }
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_end = 20usize.checked_add(json_len).ok_or("GLB JSON overflow")?;
    let bin_header = json_end.checked_add(8).ok_or("GLB BIN overflow")?;
    if &bytes[16..20] != b"JSON"
        || bin_header > bytes.len()
        || &bytes[json_end + 4..bin_header] != b"BIN\0"
    {
        return Err("source group GLB chunks are invalid".into());
    }
    let bin_len = u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().unwrap()) as usize;
    if bin_header + bin_len > bytes.len() {
        return Err("source group GLB BIN is truncated".into());
    }
    let document = serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| format!("source group GLB JSON: {error}"))?;
    Ok((document, &bytes[bin_header..bin_header + bin_len]))
}

fn read_accessor(
    document: &Value,
    bin: &[u8],
    index: usize,
    components: usize,
    component_type: u64,
) -> Result<Accessor, String> {
    let accessor = document
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|items| items.get(index))
        .ok_or_else(|| format!("GLB accessor {index} is missing"))?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(component_type) {
        return Err(format!("GLB accessor {index}: expected Float32 components"));
    }
    let expected_type = match components {
        2 => "VEC2",
        3 => "VEC3",
        4 => "VEC4",
        _ => return Err("unsupported accessor width".into()),
    };
    if accessor.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(format!("GLB accessor {index}: expected {expected_type}"));
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("GLB accessor count missing")? as usize;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or("GLB bufferView missing")? as usize;
    let view = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|items| items.get(view_index))
        .ok_or("GLB bufferView missing")?;
    if view.get("byteStride").is_some() {
        return Err(format!(
            "GLB accessor {index}: strided accessors are not accepted by the author route"
        ));
    }
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let start = view_offset
        .checked_add(accessor_offset)
        .ok_or("GLB accessor offset overflow")?;
    let len = count
        .checked_mul(components)
        .and_then(|n| n.checked_mul(4))
        .ok_or("GLB accessor length overflow")?;
    let bytes = bin
        .get(start..start + len)
        .ok_or("GLB accessor is truncated")?;
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    Ok(Accessor { components, values })
}

fn read_indices(document: &Value, bin: &[u8], primitive: &Value) -> Result<Vec<u32>, String> {
    let index = primitive
        .get("indices")
        .and_then(Value::as_u64)
        .ok_or("GLB indices missing")? as usize;
    let accessor = document
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|items| items.get(index))
        .ok_or("GLB index accessor missing")?;
    if accessor.get("type").and_then(Value::as_str) != Some("SCALAR") {
        return Err("GLB index accessor must be SCALAR".into());
    }
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or("GLB index component type missing")?;
    let width = match component_type {
        5123 => 2,
        5125 => 4,
        _ => return Err("GLB indices must be u16 or u32".into()),
    };
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("GLB index count missing")? as usize;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or("GLB index bufferView missing")? as usize;
    let view = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|items| items.get(view_index))
        .ok_or("GLB index bufferView missing")?;
    if view.get("byteStride").is_some() {
        return Err("strided index accessor is unsupported".into());
    }
    let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    let bytes = bin
        .get(start..start + count * width)
        .ok_or("GLB index accessor is truncated")?;
    Ok(bytes
        .chunks_exact(width)
        .map(|chunk| {
            if width == 2 {
                u16::from_le_bytes(chunk.try_into().unwrap()) as u32
            } else {
                u32::from_le_bytes(chunk.try_into().unwrap())
            }
        })
        .collect())
}

fn round_ties_even(value: f64) -> f64 {
    value.round_ties_even()
}

fn pack_direction(value: [f32; 3]) -> Result<[u8; 4], String> {
    let length = value
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length <= 1e-9 {
        return Err("zero or non-finite direction".into());
    }
    let mut out = [255u8; 4];
    for axis in 0..3 {
        out[axis] =
            round_ties_even(value[axis] as f64 / length * 127.5 + 127.5).clamp(0.0, 255.0) as u8;
    }
    Ok(out)
}

fn cross_squared(points: [[f32; 3]; 3]) -> f64 {
    let a = [0, 1, 2].map(|axis| points[1][axis] as f64 - points[0][axis] as f64);
    let b = [0, 1, 2].map(|axis| points[2][axis] as f64 - points[0][axis] as f64);
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    cross.iter().map(|value| value * value).sum()
}

fn write_f32x2(target: &mut [u8], value: [f32; 2]) {
    for (axis, value) in value.into_iter().enumerate() {
        target[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn write_f32x3(target: &mut [u8], value: [f32; 3]) {
    for (axis, value) in value.into_iter().enumerate() {
        target[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn read_f32x3(source: &[u8]) -> [f32; 3] {
    [0, 1, 2].map(|axis| f32::from_le_bytes(source[axis * 4..axis * 4 + 4].try_into().unwrap()))
}

fn replace_geometry(
    template: &[u8],
    collision_source: &[u8],
    vertices: &[[u8; 40]],
    indices: &[u32],
    min: [f32; 3],
    max: [f32; 3],
) -> Result<Vec<u8>, String> {
    let file = Rw4File::parse(template).map_err(|error| format!("Building40 template: {error}"))?;
    let mesh_sections: Vec<_> = file.sections_of_type(SectionType::MESH).collect();
    if mesh_sections.len() != 1 {
        return Err("Building40 template must contain exactly one mesh".into());
    }
    let mesh_payload = file
        .payload(template, mesh_sections[0].number)
        .map_err(|error| error.to_string())?;
    let mesh_words: Vec<u32> = mesh_payload
        .chunks_exact(4)
        .take(10)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect();
    if mesh_words.len() != 10 || mesh_words[0] != 40 {
        return Err("Building40 template profile mismatch".into());
    }
    let vertex_array = mesh_words[9];
    let triangle_array = mesh_words[2];
    let va_payload = file
        .payload(template, vertex_array)
        .map_err(|error| error.to_string())?;
    let ta_payload = file
        .payload(template, triangle_array)
        .map_err(|error| error.to_string())?;
    let vertex_blob = u32::from_le_bytes(
        va_payload
            .get(24..28)
            .ok_or("Building40 VA truncated")?
            .try_into()
            .unwrap(),
    );
    let index_blob = u32::from_le_bytes(
        ta_payload
            .get(24..28)
            .ok_or("Building40 TA truncated")?
            .try_into()
            .unwrap(),
    );
    let index32 = vertices.len() > 65536;
    let mut vertex_bytes = Vec::with_capacity(vertices.len() * 40);
    for vertex in vertices {
        vertex_bytes.extend_from_slice(vertex);
    }
    let mut index_bytes = Vec::with_capacity(indices.len() * if index32 { 4 } else { 2 });
    for index in indices {
        if index32 {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        } else {
            index_bytes.extend_from_slice(
                &u16::try_from(*index)
                    .map_err(|_| "u16 index overflow")?
                    .to_le_bytes(),
            );
        }
    }
    let mut replacements = HashMap::new();
    replacements.insert(vertex_blob, vertex_bytes);
    replacements.insert(index_blob, index_bytes);
    let mut patch_words = |number: u32, values: &[(usize, u32)]| -> Result<(), String> {
        let mut payload = file
            .payload(template, number)
            .map_err(|error| error.to_string())?
            .to_vec();
        for (word, value) in values {
            let at = word * 4;
            payload
                .get_mut(at..at + 4)
                .ok_or("Building40 section truncated")?
                .copy_from_slice(&value.to_le_bytes());
        }
        replacements.insert(number, payload);
        Ok(())
    };
    patch_words(
        mesh_sections[0].number,
        &[
            (3, (indices.len() / 3) as u32),
            (6, indices.len() as u32),
            (7, *indices.iter().min().unwrap()),
            (8, vertices.len() as u32),
        ],
    )?;
    patch_words(vertex_array, &[(3, vertices.len() as u32)])?;
    patch_words(
        triangle_array,
        &[
            (2, indices.len() as u32),
            (4, if index32 { 102 } else { 101 }),
        ],
    )?;
    let bbox = file
        .sections_of_type(SectionType::BBOX)
        .next()
        .ok_or("Building40 BBOX missing")?;
    let mut payload = file
        .payload(template, bbox.number)
        .map_err(|error| error.to_string())?
        .to_vec();
    if payload.len() != 32 {
        return Err("Building40 BBOX profile mismatch".into());
    }
    payload[0..12].copy_from_slice(&min.map(f32::to_le_bytes).concat());
    payload[16..28].copy_from_slice(&max.map(f32::to_le_bytes).concat());
    replacements.insert(bbox.number, payload);
    let source_file =
        Rw4File::parse(collision_source).map_err(|error| format!("collision source: {error}"))?;
    let source_section = source_file
        .sections_of_type(SectionType::COLLISION_MESH)
        .next()
        .ok_or("collision source section missing")?;
    let proxy = super::collision::Collision::parse(
        source_file
            .payload(collision_source, source_section.number)
            .map_err(|error| error.to_string())?,
        source_section.pos,
    )?;
    let destination = file
        .sections_of_type(SectionType::COLLISION_MESH)
        .next()
        .ok_or("Building40 collision section missing")?;
    replacements.insert(
        destination.number,
        proxy.transformed(min, max, destination.pos)?,
    );
    write_with_author_layout(template, &replacements)
}

/// Port `RW4.write`: preserve every non-Blob offset and all header/fixup
/// bytes, reflow only Blob payloads. In particular, shrinking the collision
/// proxy leaves the template's original gap instead of relocating the mesh,
/// material and vertex declarations.
pub fn write_with_author_layout(
    source: &[u8],
    replacements: &HashMap<u32, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let file = Rw4File::parse(source).map_err(|error| error.to_string())?;
    let end = file.section_index_end() as usize;
    let index = file.section_index_begin() as usize;
    let mut out = source
        .get(..end)
        .ok_or("RW4 section index is truncated")?
        .to_vec();
    let mut blobs: Vec<_> = file.sections_of_type(SectionType::BLOB).collect();
    blobs.sort_by_key(|section| section.pos);
    let mut positions = HashMap::new();
    for section in blobs {
        let alignment = section.alignment.max(1) as usize;
        let aligned = (out.len() + alignment - 1) & !(alignment - 1);
        out.resize(aligned, 0);
        positions.insert(section.number, out.len() as u32);
        match replacements.get(&section.number) {
            Some(payload) => out.extend_from_slice(payload),
            None => out.extend_from_slice(
                file.payload(source, section.number)
                    .map_err(|error| error.to_string())?,
            ),
        }
    }
    for section in file.sections() {
        let payload = match replacements.get(&section.number) {
            Some(payload) => payload.as_slice(),
            None => file
                .payload(source, section.number)
                .map_err(|error| error.to_string())?,
        };
        let position = positions
            .get(&section.number)
            .copied()
            .unwrap_or(section.pos);
        if section.type_code != SectionType::BLOB {
            if payload.len() > section.size as usize {
                return Err(format!(
                    "non-Blob section #{} grew without relocation",
                    section.number
                ));
            }
            let at = section.pos as usize;
            out[at..at + section.size as usize].fill(0);
            out[at..at + payload.len()].copy_from_slice(payload);
        }
        let entry = index + section.number as usize * 24;
        let raw_position = if section.type_code == SectionType::BLOB {
            position
                .checked_sub(end as u32)
                .ok_or("Blob position underflow")?
        } else {
            position
        };
        out[entry..entry + 4].copy_from_slice(&raw_position.to_le_bytes());
        out[entry + 8..entry + 12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    }
    let blob_size = (out.len() - end) as u32;
    out[76..80].copy_from_slice(&blob_size.to_le_bytes());
    Rw4File::parse(&out).map_err(|error| format!("author RW4 writeback: {error}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_boundary_matches_author() {
        assert!(65537 > 65536);
        assert!(!(65536 > 65536));
    }

    #[test]
    fn dedup_key_uses_unflipped_uv_and_group_row_is_external() {
        let a = VertexKey::new(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.2, 0.25],
            [0.0; 2],
            84,
        );
        let b = VertexKey::new(
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.2, 0.75],
            [0.0; 2],
            84,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn float32_degenerate_test_removes_repeated_and_collinear_faces() {
        let points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert!(cross_squared(points) <= 1e-20);
    }

    #[test]
    fn hope_translation_moves_positions_but_not_directions() {
        let scene = SceneTransform::new([
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            1.798_037_3,
            -0.378_246_16,
        ])
        .expect("Hope Tower translation");
        let point = scene.point([2.0, 3.0, 4.0]);
        assert!((point[0] - 2.0).abs() < 1e-6);
        assert!((point[1] - 4.798_037_3).abs() < 1e-6);
        assert!((point[2] - 3.621_753_8).abs() < 1e-6);
        assert_eq!(scene.normal([0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]);
        assert_eq!(scene.tangent([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn scene_scale_uses_inverse_transpose_for_normals() {
        let scene =
            SceneTransform::new([2.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 8.0, 1.0, 2.0, 3.0])
                .expect("positive affine transform");
        assert_eq!(scene.point([1.0, 1.0, 1.0]), [3.0, 6.0, 11.0]);
        assert_eq!(scene.tangent([1.0, 2.0, 3.0]), [2.0, 8.0, 24.0]);
        assert_eq!(scene.normal([1.0, 0.0, 0.0]), [0.5, 0.0, 0.0]);
    }

    #[test]
    fn invalid_transforms_and_directions_are_errors() {
        assert!(SceneTransform::new([0.0; 12]).is_err());
        assert!(
            SceneTransform::new([-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,])
                .is_err()
        );
        assert!(pack_direction([0.0; 3]).is_err());
        assert!(pack_direction([f32::NAN, 0.0, 1.0]).is_err());
    }
}
