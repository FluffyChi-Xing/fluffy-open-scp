//! Preserve geometry already produced by scene lowering through behavior binding.
use super::{connector::Resources, native_lot::{MODEL_TYPE, RASTER_TYPE}};
use dbpf::ResourceId;
use sc_properties::{lot_unit::*, PropertyFile};
use sha2::{Digest, Sha256};

pub(super) struct GeometryGuard {
    lot: ResourceId,
    properties: PropertyFile,
    render: Vec<(ResourceId, [u8; 32])>,
}

fn protected_field(hash: u32) -> bool {
    [PROP_ID_BASE, PROP_TRANSFORM_BASE, PROP_SLOT_BASE, PROP_RANDOMIZE_SLOT_BASE]
        .into_iter().any(|base| (base..base + PROP_BINS as u32).contains(&hash))
        || matches!(hash,
            PATH_POINT_POINTS | PATH_POINT_TANGENTS | SPAWNER_IDS | SPAWNER_TRANSFORMS
            | SPAWNER_COUNT | SPAWNER_COUNT_RANDOM | SPAWNER_AGENT | 0x0F16_9C46
            | LIGHT_SPEC_LEVELS | LIGHT_LENGTHS | LIGHT_CULL_DISTANCES | LIGHT_FALLOFF_STARTS
            | LIGHT_IS_VOLUMETRIC | LIGHT_VOL_STRENGTHS)
        || (LIGHT_IDS..=LIGHT_DEBUG_NAMES).contains(&hash)
}

impl GeometryGuard {
    pub(super) fn capture(resources: &Resources, lot: ResourceId) -> Result<Self, String> {
        let properties = read_lot(resources, lot)?;
        let render = resources.iter().filter(|(id, _)| matches!(id.type_id, MODEL_TYPE | RASTER_TYPE))
            .map(|(id, bytes)| (*id, Sha256::digest(bytes).into())).collect();
        Ok(Self { lot, properties, render })
    }

    pub(super) fn verify(&self, resources: &Resources) -> Result<serde_json::Value, String> {
        let lot = read_lot(resources, self.lot)?;
        for property in self.properties.values.iter().chain(&lot.values).filter(|p| protected_field(p.hash)) {
            if self.properties.get(property.hash) != lot.get(property.hash) {
                return Err(format!("BEHAVIOR_GEOMETRY_DRIFT: {} field {:08X}", self.lot, property.hash));
            }
        }
        for (id, expected) in &self.render {
            if !resources.get(id).is_some_and(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)) == *expected) {
                return Err(format!("BEHAVIOR_GEOMETRY_DRIFT: render resource {id}"));
            }
        }
        Ok(serde_json::json!({ "lot":self.lot.to_string(), "renderResources":self.render.len(),
            "assetFields":self.properties.values.iter().filter(|p| protected_field(p.hash)).count() }))
    }
}

fn read_lot(resources: &Resources, lot: ResourceId) -> Result<PropertyFile, String> {
    let bytes = resources.get(&lot).ok_or_else(|| format!("BEHAVIOR_GEOMETRY_DRIFT: missing Asset {lot}"))?;
    PropertyFile::parse(bytes).map_err(|error| format!("BEHAVIOR_GEOMETRY_DRIFT: {lot}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::property_edit::{encode, set_scalar};
    use sc_properties::{PropType, Value};

    #[test]
    fn rejects_geometry_and_render_drift_but_allows_vehicle_fill_configuration() {
        let lot = ResourceId { type_id: 0x00B1_B104, group: 0x40E1_C000, instance: 1 };
        let model = ResourceId { type_id: MODEL_TYPE, group: 0, instance: 2 };
        let mut properties = PropertyFile::default();
        set_scalar(&mut properties, SPAWNER_COUNT, PropType::Int32, Value::Int32(1));
        let mut resources = [(lot, encode(&properties).unwrap()), (model, vec![1, 2, 3])].into_iter().collect::<Resources>();
        let guard = GeometryGuard::capture(&resources, lot).unwrap();
        set_scalar(&mut properties, PROP_PERCENT_FILL_BASE, PropType::Bool, Value::Bool(false));
        resources.insert(lot, encode(&properties).unwrap());
        guard.verify(&resources).unwrap();
        resources.insert(model, vec![1, 2, 4]);
        assert!(guard.verify(&resources).unwrap_err().contains("render resource"));
        resources.insert(model, vec![1, 2, 3]);
        set_scalar(&mut properties, SPAWNER_COUNT, PropType::Int32, Value::Int32(0));
        resources.insert(lot, encode(&properties).unwrap());
        assert!(guard.verify(&resources).unwrap_err().contains("0E715928"));
    }
}
