//! Packed building vertex motion, as consumed by building5MorphRotationAnimVS.
use rw4::{ComponentValue, DeclarationUsage, DecodedMesh};

#[derive(Clone, Copy)]
pub(crate) struct VertexMotion {
    axis_slot: [u8; 4],
    pivot_angle: [i16; 4],
}

/// Require the complete packed declaration, not just the byte marker used by
/// the generic GLB exporter's approximate skin fallback.
pub(crate) fn packed_motion(mesh: &DecodedMesh) -> Option<Vec<VertexMotion>> {
    mesh.vertices
        .iter()
        .map(|vertex| {
            if vertex.blend_indices_raw().is_some() {
                return None;
            }
            let axis_slot = vertex.shell_marker()?;
            let pivot_angle =
                vertex
                    .components
                    .iter()
                    .find_map(|(element, value)| match value {
                        ComponentValue::Short4(raw)
                            if element.usage == DeclarationUsage::TexCoord
                                && element.index == 2 =>
                        {
                            Some(*raw)
                        }
                        _ => None,
                    })?;
            (axis_slot[3] < 4).then_some(VertexMotion {
                axis_slot,
                pivot_angle,
            })
        })
        .collect()
}

impl VertexMotion {
    pub(crate) fn affects_slot(self, slot: u8) -> bool {
        if self.axis_slot[3] != slot {
            return false;
        }
        let length_squared: i32 = self.axis_slot[..3]
            .iter()
            .map(|&x| (i32::from(x) - 127).pow(2))
            .sum();
        if 2 * length_squared < 127 * 127 {
            self.pivot_angle[..3] != [0, 0, 0]
        } else {
            self.pivot_angle[3] != 0
        }
    }

    pub(crate) fn write(self, bytes: &mut Vec<u8>) {
        bytes.extend(self.axis_slot);
        for value in self.pivot_angle {
            bytes.extend(value.to_le_bytes());
        }
    }
}
