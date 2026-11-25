//! Native commercial bindings from EP1 floors BBF5B017, 8A807C96 and 29CF9867.
pub(super) const PROFILE: &str = "ground-megatower-mall-w2-v1";
pub(super) const STREET_SHOPPERS: u32 = 0x1C1F_597B;
pub(super) const STREET_CARS: &[u32] = &[0x8A97_E8AF, 0xAD48_46DB];
pub(super) const BUNDLES: [u32; 3] = [STREET_SHOPPERS, 0xDD6E_83EC, 0xF51A_B185];
pub(super) const BASE_SHOPPERS: u32 = 0x3A82_63E2;
pub(super) const BASE_PROFIT_CYCLE: u32 = 0x6827_D126;
pub(super) const INTERNAL: [u32; 3] = [0x8A72_31A2, 0x8A72_31A1, 0x8A72_31A0];
pub(super) const CUSTOMER_STATE: [u32; 3] = [0xC3EC_8B4E, 0xC3EC_8B4D, 0xC3EC_8B4C];
pub(super) const COMMERCIAL_CLASS: [u32; 3] = [0xF25B_DF56, 0x9957_A6A5, 0xCA24_5C9C];
// C2BF00DB/D8/D9 specialize their return transport interval from this vector.
pub(super) const RETURN_INTERVAL: u32 = 0x0ACF_FA64;
pub(super) const RETURN_INTERVAL_VALUE: [f32; 2] = [20.0, 20.0];
pub(super) const SALES_MULTIPLIER: u32 = 0x0FC6_E602;
pub(super) const SALES_MULTIPLIERS: [f32; 3] = [10.0, 28.0, 100.0];
pub(super) const FLOOR_INCOME_INPUTS: [f32; 3] = [730.0, 1150.0, 2150.0];

pub(super) fn ep1_bundle(id: u32) -> bool {
    BUNDLES.contains(&id)
        || super::behavior_commercial_mixed::STREET.contains(&id)
        || INTERNAL.contains(&id)
}

// The floor's local economy consumes these parameters. The ground host retains
// its own worker schedule and building-site service ownership.
pub(super) const INCOME_CAPACITY: u32 = 0x09AE_19D7;
pub(super) const PROFIT_CAPACITY: f32 = 5680.0;
pub(super) const SALES_CAPACITY: f32 = 56800.0;

pub(super) fn schedule(jobs: i32, minimum_workers: i32) -> [i32; 8] {
    // The 250-job W2 floor requests 150 workers in each of its six slots.
    let on_duty = (jobs * 150 / 250).max(minimum_workers);
    [on_duty, on_duty, on_duty, on_duty, on_duty, on_duty, 0, 0]
}

pub(super) fn verify_installation(
    game_data: &std::path::Path,
    bundles: &[u32],
) -> Result<(), String> {
    let required: Vec<_> = bundles
        .iter()
        .copied()
        .filter(|&id| ep1_bundle(id))
        .map(|instance| dbpf::ResourceId {
            type_id: super::behavior::PROPERTY_TYPE,
            group: 0x61EF_C700,
            instance,
        })
        .collect();
    for path in crate::game_assets::package_paths(game_data) {
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("SimCityDLCEP1-Scripts_"))
        {
            continue;
        }
        if let Ok(package) = dbpf::Package::open(&path)
            && required
                .iter()
                .all(|id| package.entries().iter().any(|entry| entry.id == *id))
        {
            return Ok(());
        }
    }
    Err("COMMERCIAL_RESOURCE: MegaTower mall requires the Cities of Tomorrow script package".into())
}
