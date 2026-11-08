//! Minimal GLB v2 assembly for one native material group.
//!
//! The LOTM container stores one GLB per material group.  Geometry layout
//! follows the Asset Editor: POSITION, NORMAL, TANGENT, TEXCOORD_0..3, indices.

use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct GlbMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    /// TEXCOORD_0..3.
    pub uvs: [Vec<[f32; 2]>; 4],
    pub indices: Vec<u32>,
    pub preview_png: Option<Vec<u8>>,
}

pub fn encode(mesh: &GlbMesh) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<serde_json::Value> = Vec::new();
    let mut accessors: Vec<serde_json::Value> = Vec::new();

    let mut push_view = |bin: &mut Vec<u8>, data: &[u8]| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(data);
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": data.len()}));
        views.len() - 1
    };

    let mut image_texture = None;
    if let Some(png) = &mesh.preview_png {
        let view = push_view(&mut bin, png);
        image_texture = Some((view, png.len()));
    }

    let float_bytes = |values: &[f32]| -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    };
    let mut attribute = |values: &[f32], kind: &str, count: usize, components: usize| -> usize {
        let view = push_view(&mut bin, &float_bytes(values));
        let mut accessor = json!({
            "bufferView": view,
            "componentType": 5126,
            "count": count,
            "type": kind,
        });
        if kind == "VEC3" && accessors.is_empty() {
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for chunk in values.chunks(3) {
                for index in 0..3 {
                    min[index] = min[index].min(chunk[index]);
                    max[index] = max[index].max(chunk[index]);
                }
            }
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        let _ = components;
        accessors.push(accessor);
        accessors.len() - 1
    };

    let positions: Vec<f32> = mesh.positions.iter().flatten().copied().collect();
    let normals: Vec<f32> = mesh.normals.iter().flatten().copied().collect();
    let tangents: Vec<f32> = mesh.tangents.iter().flatten().copied().collect();
    let position = attribute(&positions, "VEC3", mesh.positions.len(), 3);
    let normal = attribute(&normals, "VEC3", mesh.normals.len(), 3);
    let tangent = attribute(&tangents, "VEC4", mesh.tangents.len(), 4);
    let mut uv_accessors = Vec::new();
    for set in &mesh.uvs {
        let values: Vec<f32> = set.iter().flatten().copied().collect();
        uv_accessors.push(attribute(&values, "VEC2", set.len(), 2));
    }

    let index_bytes: Vec<u8> = mesh
        .indices
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let index_view = push_view(&mut bin, &index_bytes);
    accessors.push(json!({
        "bufferView": index_view,
        "componentType": 5125,
        "count": mesh.indices.len(),
        "type": "SCALAR",
    }));
    let index_accessor = accessors.len() - 1;

    let mut primitives = json!({
        "attributes": {
            "POSITION": position,
            "NORMAL": normal,
            "TANGENT": tangent,
            "TEXCOORD_0": uv_accessors[0],
            "TEXCOORD_1": uv_accessors[1],
            "TEXCOORD_2": uv_accessors[2],
            "TEXCOORD_3": uv_accessors[3],
        },
        "indices": index_accessor,
    });
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut images = Vec::new();
    if let Some((view, _)) = image_texture {
        images.push(json!({"bufferView": view, "mimeType": "image/png"}));
        textures.push(json!({"source": 0, "sampler": 0}));
        materials.push(json!({
            "name": mesh.name,
            "alphaMode": "OPAQUE",
            "doubleSided": true,
            "pbrMetallicRoughness": {
                "baseColorTexture": {"index": 0},
                "metallicFactor": 0.0,
                "roughnessFactor": 0.65,
            },
        }));
        primitives["material"] = json!(0);
    }

    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let document = json!({
        "asset": {"version": "2.0", "generator": "OpenSCP native Rust"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": mesh.name, "mesh": 0}],
        "meshes": [{"name": mesh.name, "primitives": [primitives]}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
        "materials": materials,
        "textures": textures,
        "images": images,
        "samplers": [{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}],
    });
    let mut json_bytes = serde_json::to_vec(&document).expect("GLB JSON");
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&0x4654_6C67u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glb_has_header_and_four_uv_sets() {
        let mesh = GlbMesh {
            name: "NY_Native_000".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
            uvs: [
                vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                vec![[0.0, 0.0]; 3],
                vec![[0.0, 0.0]; 3],
                vec![[0.0, 0.0]; 3],
            ],
            indices: vec![0, 1, 2],
            preview_png: None,
        };
        let bytes = encode(&mesh);
        assert_eq!(&bytes[..4], b"glTF");
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(length, bytes.len());
    }
}
