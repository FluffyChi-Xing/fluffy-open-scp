//! Wealth-specific street admission from the three EP1 mall donors.
use super::behavior_templates::key;
use super::property_edit::{encode, set_array_with_flags};
use dbpf::ResourceId;
use sc_properties::{PropType, PropertyFile, Value};

pub(super) const PROFILE: &str = "ground-mall-mixed-v1";
pub(super) const STREET: [u32; 3] = [0x1C1F_5978, 0x1C1F_597B, 0x1C1F_597A];
pub(super) const CAPACITY_FIELDS: [u32; 3] = [0xA00F_E301, 0xA00F_E302, 0xA00F_E303];
pub(super) const ACTIVE_TIER_COUNT: u32 = 0xA00F_E304;
const SHOPPERS: [u32; 3] = [0xEAE4_65EE, 0xEAE4_65ED, 0xEAE4_65EC];
const MALL_DEMAND_MARKER: u32 = 0xC608_5FBF;

pub(super) fn bundle_id(unit: ResourceId) -> ResourceId {
    ResourceId {
        group: 0x40E0_C700,
        ..unit
    }
}

pub(super) fn bundle() -> Result<Vec<u8>, String> {
    // A referenced bundle applies the final capacity overrides after the
    // stock admission bundles declare their native ShopperW1/W2/W3 bins.
    let mut property = PropertyFile::default();
    set_array_with_flags(
        &mut property,
        0x0A6F_E749,
        PropType::Key,
        SHOPPERS
            .into_iter()
            .chain([MALL_DEMAND_MARKER])
            .map(key)
            .collect(),
        0x809C,
    );
    set_array_with_flags(
        &mut property,
        0x0A6F_E74A,
        PropType::UInt32,
        CAPACITY_FIELDS
            .into_iter()
            .chain([ACTIVE_TIER_COUNT])
            .map(Value::UInt32)
            .collect(),
        0x809C,
    );
    set_array_with_flags(
        &mut property,
        0x0A6F_E74D,
        PropType::Float,
        vec![Value::Float(1.0); 4],
        0x809C,
    );
    encode(&property)
}
