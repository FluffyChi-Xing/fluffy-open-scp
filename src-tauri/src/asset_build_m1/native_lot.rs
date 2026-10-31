//! Rust port of the Blender plug-in's native retail Lot property contract.
//!
//! The plug-in does not serialize a generic JSON lot at runtime. It emits a
//! compact PropertyFile with the hashes below and points the lot at a mask
//! Raster and the native surface RW4. Keeping this encoder byte-for-byte
//! compatible lets the OpenSCP builder reuse the game's proven Lot resources.

use dbpf::ResourceId;

pub const LOT_TYPE: u32 = 0x00B1_B104;
pub const RASTER_TYPE: u32 = 0x2F4E_681C;
pub const MODEL_TYPE: u32 = 0x2F4E_681B;
pub const SURFACE_MODEL: u32 = 0xA0DA_9E6C;

#[derive(Debug, Clone, PartialEq)]
pub struct NativeLotSpec {
    pub size: [f32; 2],
    pub offset: [f32; 2],
    pub period: [f32; 2],
    pub base_tile: i32,
    pub color_heights: [f32; 4],
    pub border_heights: [f32; 4],
    pub colors: [[f32; 4]; 4],
    pub border_colors: [[f32; 4]; 4],
    pub border_widths: [f32; 4],
}

impl Default for NativeLotSpec {
    fn default() -> Self {
        Self {
            size: [80.0, 80.0],
            offset: [0.0, 0.0],
            period: [15.0, 15.0],
            base_tile: 2,
            color_heights: [0.0; 4],
            border_heights: [0.0; 4],
            colors: [[1.0; 4]; 4],
            border_colors: [[1.0; 4]; 4],
            border_widths: [0.0; 4],
        }
    }
}

fn property(hash: u32, kind: u16, payload: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&hash.to_be_bytes());
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(payload);
}

fn vec2(hash: u32, value: [f32; 2], out: &mut Vec<u8>) {
    let mut p = Vec::with_capacity(8);
    for v in value { p.extend_from_slice(&v.to_be_bytes()); }
    property(hash, 48, &p, out);
}

fn key(hash: u32, id: ResourceId, out: &mut Vec<u8>) {
    let mut p = Vec::with_capacity(12);
    p.extend_from_slice(&id.instance.to_be_bytes());
    p.extend_from_slice(&id.type_id.to_be_bytes());
    p.extend_from_slice(&id.group.to_be_bytes());
    property(hash, 32, &p, out);
}

fn vec4(hash: u32, value: [f32; 4], out: &mut Vec<u8>) {
    let mut p = Vec::with_capacity(16);
    for v in value { p.extend_from_slice(&v.to_be_bytes()); }
    property(hash, 51, &p, out);
}

/// Encode the exact PropertyFile payload emitted by `lot_native.py`.
pub fn encode_property(spec: &NativeLotSpec, mask: ResourceId) -> Vec<u8> {
    let mut values = Vec::new();
    vec2(0x0CCB_7FC8, spec.size, &mut values);
    vec2(0x0CCB_7FC9, spec.offset, &mut values);
    vec2(0x0CCB_7FD0, spec.period, &mut values);
    key(0x0CCB_7FD5, mask, &mut values);
    key(0x0CCB_7FD4, ResourceId { type_id: MODEL_TYPE, group: 0, instance: SURFACE_MODEL }, &mut values);
    property(0x0CCB_7FD6, 9, &spec.base_tile.to_be_bytes(), &mut values);
    vec4(0x0D02_D58A, spec.color_heights, &mut values);
    vec4(0x0D02_D58B, spec.border_heights, &mut values);
    for i in 0..4 {
        vec4(0x0D02_D586 + i as u32, spec.colors[i], &mut values);
        vec4(0x0D7A_F042 + i as u32, spec.border_colors[i], &mut values);
        property(0x0D7A_F046 + i as u32, 13, &spec.border_widths[i].to_be_bytes(), &mut values);
    }
    let count = 3 + 2 + 1 + 2 + 12;
    let mut out = Vec::with_capacity(4 + values.len());
    out.extend_from_slice(&(count as u32).to_be_bytes());
    out.extend_from_slice(&values);
    out
}

pub fn mask_id(instance: u32) -> ResourceId {
    ResourceId { type_id: RASTER_TYPE, group: 0, instance }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_property_has_expected_count_and_keys() {
        let bytes = encode_property(&NativeLotSpec::default(), mask_id(0x8123_4567));
        assert_eq!(u32::from_be_bytes(bytes[..4].try_into().unwrap()), 20);
        assert!(bytes.windows(4).any(|w| w == 0x0CCB_7FD5u32.to_be_bytes()));
        assert!(bytes.windows(4).any(|w| w == 0x0CCB_7FD4u32.to_be_bytes()));
    }
}
