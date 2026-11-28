use super::{MODEL, composition, property_id, set_last, vector2};
use crate::asset_build_m1::native_model::{
    GlbPrimitive, compile_merged_primitives, placement_from_bounds,
};
use dbpf::{OverlayEntry, ResourceId};
use sc_properties::{Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};

mod raster;

const RW4: u32 = 0x2F4E_681B;
const BOUNDS: u32 = 0x00F9_EFBA;
const MODEL_RESOURCE: u32 = 0x00F9_EFBB;
const ERROR: &str = "road_draft_component_missing";

pub(super) struct Surface {
    pub uv: [[f32; 2]; 2],
    pub period: f32,
    material: Vec<Property>,
}

impl Surface {
    pub(super) fn from_ribbon(file: &PropertyFile) -> Result<Self, String> {
        let period = vector2(file, 0x0D94_2BD1)?[1];
        if period <= 0.0 || !period.is_finite() {
            return Err(ERROR.into());
        }
        Ok(Self {
            uv: [vector2(file, 0x0D94_2BCF)?, vector2(file, 0x0D94_2BD0)?],
            period,
            material: file
                .values
                .iter()
                .filter(|p| {
                    matches!(
                        p.hash,
                        0x0955_8821 | 0x0E53_4076..=0x0E53_4079 | 0x0EE9_943C
                    )
                })
                .cloned()
                .collect(),
        })
    }

    pub(super) fn shares_material(&self, other: &Self) -> bool {
        self.material == other.material
    }
}

pub(super) struct SurfaceMesh {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

pub(super) enum SweepBounds {
    EndCapDepth(f32),
    ShoulderEdge(f32),
}

impl SurfaceMesh {
    pub(super) fn new() -> Self {
        Self {
            positions: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    // Points alternate between the two edges of the strip.
    pub(super) fn strip(&mut self, points: impl Iterator<Item = ([[f32; 3]; 2], [[f32; 2]; 2])>) {
        let start = self.positions.len() as u32;
        for (positions, uvs) in points {
            self.positions.extend(positions);
            self.uvs.extend(uvs);
        }
        let end = self.positions.len() as u32;
        for i in (start..end.saturating_sub(2)).step_by(2) {
            self.indices.extend([i, i + 1, i + 2, i + 1, i + 3, i + 2]);
        }
    }

    pub(super) fn encode(
        self,
        model: ResourceId,
        material: &Surface,
        sweep_bounds: SweepBounds,
        reserve: &mut impl FnMut(ResourceId) -> Result<(), &'static str>,
    ) -> Result<[OverlayEntry; 2], String> {
        if self.indices.is_empty() || self.positions.iter().flatten().any(|v| !v.is_finite()) {
            return Err(ERROR.into());
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for point in &self.positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        match sweep_bounds {
            SweepBounds::EndCapDepth(depth) => {
                min[1] = 0.0;
                max[1] = depth;
            }
            SweepBounds::ShoulderEdge(edge) => {
                // The intersection template assigns surface tiles by these
                // lateral bounds. Cover the outer edge of its centered box.
                min[0] = min[0].min(edge);
                max[0] = max[0].max(edge);
            }
        }
        let count = self.positions.len();
        let primitive = GlbPrimitive {
            positions: self.positions,
            normals: vec![[0.0, 0.0, 1.0]; count],
            uvs: self.uvs,
            facade_uvs: Vec::new(),
            tint_column: Vec::new(),
            seeds: Vec::new(),
            tangents: vec![[1.0, 0.0, 0.0]; count],
            indices: self.indices,
        };
        let [x, y, z] = placement_from_bounds([&primitive]).origin;
        let preserve_coordinates = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, -x, -y, -z];
        let compiled =
            compile_merged_primitives(vec![vec![primitive]], [0; 6], Some(preserve_coordinates))?;
        let bytes = raster::complete(&compiled.rw4)?;
        let rw4 = ResourceId {
            type_id: RW4,
            ..model
        };
        reserve(model)?;
        reserve(rw4)?;
        let mut props = PropertyFile {
            values: material.material.clone(),
            claimed_count: 0,
        };
        props.values.extend([
            Property {
                hash: BOUNDS,
                prop_type: PropType::BoundingBox,
                kind: Kind::Array(vec![Value::BoundingBox { min, max }]),
                encoding: PropertyEncoding {
                    flags: 0x801C,
                    array_item_size: Some(24),
                },
            },
            Property {
                hash: MODEL_RESOURCE,
                prop_type: PropType::Key,
                kind: Kind::Scalar(Value::Key(Key {
                    type_id: RW4,
                    group: rw4.group,
                    instance: rw4.instance,
                })),
                encoding: Default::default(),
            },
        ]);
        Ok([
            OverlayEntry::new(model, props.encode_canonical().map_err(|e| e.to_string())?),
            OverlayEntry::new(rw4, bytes),
        ])
    }
}

pub(super) fn id(appearance: u32, feature: &str, index: usize) -> ResourceId {
    let digest = crate::mod_flow::digest(
        format!("openscp/road/{feature}/{appearance:08X}/{index}").as_bytes(),
    );
    property_id(
        0,
        u32::from_str_radix(&digest[..8], 16).expect("digest is hexadecimal"),
    )
}

pub(super) fn append(
    target: &mut PropertyFile,
    donor: &PropertyFile,
    row: usize,
    model: ResourceId,
) -> Result<(), String> {
    composition::append_component(target, donor, row, None)?;
    let key = Value::Key(Key {
        type_id: 0,
        group: model.group,
        instance: model.instance,
    });
    set_last(target, super::KEYS, key.clone())?;
    set_last(target, MODEL, key)
}
