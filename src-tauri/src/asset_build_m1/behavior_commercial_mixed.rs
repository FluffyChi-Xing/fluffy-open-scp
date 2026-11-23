//! Wealth-specific street admission from the three EP1 mall donors.
use super::{behavior_nodes::LaborSpec, behavior_templates::key};
use super::property_edit::{encode, set_array_with_flags};
use dbpf::ResourceId;
use sc_properties::{PropType, PropertyFile, Value};

pub(super) const PROFILE: &str = "ground-mall-mixed-v1";
pub(super) const STREET: [u32; 3] = [0x1C1F_5978, 0x1C1F_597B, 0x1C1F_597A];
pub(super) const CAPACITY_FIELDS: [u32; 3] = [0xA00F_E301, 0xA00F_E302, 0xA00F_E303];
pub(super) const ACTIVE_TIER_COUNT: u32 = 0xA00F_E304;
const SHOPPERS: [u32; 3] = [0xEAE4_65EE, 0xEAE4_65ED, 0xEAE4_65EC];
const MALL_DEMAND_MARKER: u32 = 0xC608_5FBF;
pub(super) const SHOPPER_INPUTS: [&str; 3] = ["shoppersW1", "shoppersW2", "shoppersW3"];

pub(super) fn split_staffing(capacities: [i32; 3], labor: LaborSpec) -> [LaborSpec; 3] {
    // Preserve each authored total. Rounding follows the order of the three
    // income tiers, with any remainder assigned to the last applicable tier.
    fn distribute(total: i32, weights: [i32; 3]) -> [i32; 3] {
        let denominator: i32 = weights.iter().sum();
        if denominator == 0 {
            return [0; 3];
        }
        let mut prefix = 0i64;
        let mut assigned = 0;
        weights.map(|weight| {
            prefix += i64::from(weight);
            let through_tier = (i64::from(total) * prefix / i64::from(denominator)) as i32;
            let count = through_tier - assigned;
            assigned = through_tier;
            count
        })
    }

    let w1 = distribute(labor.w1, capacities);
    let w2 = distribute(labor.w2, capacities);
    let w3 = distribute(labor.w3, capacities);
    let tier_jobs = std::array::from_fn(|i| w1[i] + w2[i] + w3[i]);
    let minimum = distribute(labor.min_workers, tier_jobs);
    let slots = labor.slots.map(|slots| slots.map(|value| distribute(value, capacities)));
    let schedule = labor.schedule.map(|schedule| schedule.map(|value| distribute(value, capacities)));
    std::array::from_fn(|i| LaborSpec {
        w1: w1[i],
        w2: w2[i],
        w3: w3[i],
        min_workers: minimum[i],
        schedule: schedule.map(|values| values.map(|tiers| tiers[i])),
        slots: slots.map(|values| values.map(|tiers| tiers[i])),
    })
}

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
