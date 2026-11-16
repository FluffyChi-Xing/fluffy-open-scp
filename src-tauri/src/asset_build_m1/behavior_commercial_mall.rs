//! Street-facing part of the standard W2 mall (61EFC000:8A807C96).
pub(super) const PROFILE: &str = "ground-megatower-mall-w2-v1";
pub(super) const STREET_SHOPPERS: u32 = 0x1C1F_597B;
pub(super) const STREET_CARS: &[u32] = &[0x8A97_E8AF, 0xAD48_46DB];
pub(super) const BUNDLES: [u32; 3] = [STREET_SHOPPERS, 0xDD6E_83EC, 0xF51A_B185];
pub(super) const BASE_SHOPPERS: u32 = 0x3A82_63E2;
pub(super) const BASE_PROFIT_CYCLE: u32 = 0x6827_D126;

// The floor's local economy consumes these parameters. The ground host retains
// its own worker schedule and does not register the tower's internal transport.
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
        .filter(|id| BUNDLES.contains(id) || super::behavior_commercial_mixed::STREET.contains(id))
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
