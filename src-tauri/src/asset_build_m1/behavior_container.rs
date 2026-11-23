//! Business-free host. Common Toy records are copied with the same leaf-first
//! flattening used by the native tourism profile; no tower menu/category is
//! imported. The native base's exclusions disable stock staffing/services.
use super::{
    behavior::PROPERTY_TYPE,
    connector::Resources,
    menu,
    native_profile::flat_chain,
    property_edit::{encode, set_array_with_flags, set_scalar},
};
use dbpf::ResourceId;
use sc_properties::{Kind, PropType, PropertyFile, Value};

pub(super) const SCOPE: &str = "$building-container";
const ENABLE: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
const EXCLUDED: u32 = 0x0A02_09B2;
const TOGGLE: u32 = 0x44F5_CF6C;
pub(super) const GARBAGE: u32 = 0xD7C6_2954;
const GARBAGE_AMOUNT: u32 = 0x0C21_6562;
const RESIDENCE_STATE: &[u32] = &[
    0x182D_2924, // zoned construction and state transitions
    0x933F_0FBF, // ground plop occupancy and state transitions
    0x999F_8484, 0x999F_8485, 0xA797_4BCE, 0x8D9A_F497, // ground move-in
    0x78A7_30CD, 0x78A7_30CE, 0x78A7_30CF, // MegaTower move-in
    0xEBFC_1753, 0xEBFC_1750, 0xEBFC_1751, // MegaTower apartment lifecycle
];
// D381A8C4 effective B2; additional leaf-specific behaviors never belong here.
const DISABLED: &[u32] = &[
    GARBAGE,
    0x54AB_8B16,
    0x4893_9D96,
    0x825B_4CA4,
    0x1B94_6073,
    0xDFFA_74FC,
    0xCDE7_D1E2,
    0xE0C0_2E1F,
    0xB741_FA37,
    0xF236_BB93,
    0x11D2_71CD,
    0x18CA_40A1,
    0xEE31_E6B8,
    0xD9EA_45D4,
    0x2B1E_8F1D,
    0x8272_746D,
    0x4A42_F50E,
    0x8BA6_D35D,
];
const BUSINESS: &[u32] = &[
    0xD07C_5A04,
    0x1E16_EDA9,
    0x6A32_6F41,
    0xF51A_B185,
    0xE153_69A2,
    0x75D2_2FD0,
    0x75D2_2FD1,
    0x75D2_2FD2,
    0x6EA8_E701,
    0x265F_B8DD,
    0x265F_B8DE,
    0x265F_B8DF,
    0x3A82_63E1,
    0x3A82_63E2,
    0x3A82_63E3,
    0x1C1F_5978,
    0x1C1F_597A,
    0x1C1F_597B,
    0xAB9A_0A3E,
    0xD4C3_0C02,
    0xA8CB_9DE1,
    0xAC8B_8A48,
    0x6C04_2366,
    0x6C04_2367,
];

fn id(instance: u32) -> ResourceId {
    ResourceId {
        type_id: PROPERTY_TYPE,
        group: menu::UNIT_TEMPLATE_GROUP,
        instance,
    }
}
fn common() -> Result<PropertyFile, String> {
    let resources: Resources = [
        (
            id(0x2A25_CF66),
            include_bytes!("../../resources/sc5/tourism-unit-anchors/2a25cf66.bin").to_vec(),
        ),
        (
            id(0xF98B_CB35),
            include_bytes!("../../resources/sc5/tourism-unit-anchors/f98bcb35.bin").to_vec(),
        ),
        (
            id(0x9EC4_74FF),
            include_bytes!("../../resources/sc5/tourism-unit-anchors/9ec474ff.bin").to_vec(),
        ),
        (
            id(0x593B_61D2),
            include_bytes!("../../resources/sc5/tourism-unit-anchors/593b61d2.bin").to_vec(),
        ),
    ]
    .into_iter()
    .collect();
    flat_chain(&resources, id(0x2A25_CF66)).map(|(file, _)| file)
}
fn active(unit: &PropertyFile, bundle: u32) -> bool {
    let has = |hash| {
        unit.get(hash).is_some_and(|p| {
            matches!(&p.kind,Kind::Array(values)
        if values.iter().any(|v|matches!(v,Value::Key(k) if k.instance==bundle)))
        })
    };
    ENABLE.into_iter().any(has) && !has(EXCLUDED)
}
pub(super) fn patch(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let identity = PropertyFile::parse(bytes).map_err(|e| e.to_string())?;
    let mut host = common()?;
    host.values.retain(|p| p.hash != menu::HASH_PARENT);
    for hash in [
        0x08E1_7ED7,
        0x0ACF_FA64,
        0x09FB_78CB,
        0x0A8A_41DA,
        0x09AE_19D7,
    ] {
        host.values.retain(|p| p.hash != hash);
        if let Some(p) = identity.get(hash) {
            host.values.push(p.clone());
        }
    }
    let exclusions = DISABLED
        .iter()
        .chain(BUSINESS)
        .filter(|&&bundle| bundle != GARBAGE || !active(&identity, GARBAGE))
        .map(|&instance| {
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance,
            })
        })
        .collect();
    set_array_with_flags(&mut host, EXCLUDED, PropType::Key, exclusions, 0x809C);
    if active(&identity, GARBAGE) {
        host.values.retain(|property| property.hash != GARBAGE_AMOUNT);
        if let Some(amount) = identity.get(GARBAGE_AMOUNT) {
            host.values.push(amount.clone());
        }
        if !active(&host, GARBAGE) {
            return Err("BEHAVIOR_CONTAINER_GARBAGE: inherited host service was lost".into());
        }
    }
    for hash in [
        0x0A79_17C3,
        0x0A79_17C5,
        0x0A79_17C6,
        0x0A79_17C7,
        0x0A79_17C8,
    ] {
        set_scalar(&mut host, hash, PropType::Float, Value::Float(0.0));
    }
    verify(&host)?;
    encode(&host)
}
pub(super) fn verify(host: &PropertyFile) -> Result<(), String> {
    verify_with_staffing(host, None)
}

pub(super) fn verify_program(host: &PropertyFile, program: &super::behavior_nodes::ResolvedBehavior) -> Result<(), String> {
    verify_with_staffing(host, program.host_parking_staff.then_some(program))
}

fn verify_with_staffing(host: &PropertyFile, staffing: Option<&super::behavior_nodes::ResolvedBehavior>) -> Result<(), String> {
    let labor_bundle = 0xD07C_5A04;
    if host.get(menu::HASH_PARENT).is_some()
        || BUSINESS.iter().any(|&b| active(host, b) && !(b == labor_bundle && staffing.is_some()))
    {
        return Err("BEHAVIOR_UNIT_SCOPE: container must not own a business".into());
    }
    if staffing.is_some() && !active(host, labor_bundle) {
        return Err("BEHAVIOR_CONTAINER_COMMUTE: host labor provider is missing".into());
    }
    for hash in [0x0A79_17C3, 0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7, 0x0A79_17C8] {
        let expected = match staffing {
            Some(program) => *program.set_float_fields.get(&hash)
                .ok_or_else(|| format!("BEHAVIOR_CONTAINER_COMMUTE: missing planned job field {hash:08X}"))? as f32,
            None => 0.0,
        };
        if host.get(hash).and_then(|p| p.scalar()) != Some(&Value::Float(expected)) {
            return Err(format!("BEHAVIOR_CONTAINER_COMMUTE: unexpected host job field {hash:08X}"));
        }
    }
    // Housing state belongs to a child slot. Keep its vacancy and abandonment
    // rules and the native FailUnit fallback off the visible, toggleable host.
    if RESIDENCE_STATE.iter().any(|&bundle| active(host, bundle))
        || host.get(0x07E0_AD09).is_some()
        || host.get(0x0A79_17CB).is_some_and(|property| {
            property.scalar() != Some(&Value::Float(0.0))
        })
    {
        return Err("BEHAVIOR_CONTAINER_RESIDENCE: host owns housing or a failure target".into());
    }
    // Ordinary containers retain the stock economy/lifecycle infrastructure.
    // The tower exclusion list removes it because towers supply an EP1
    // replacement; our ordinary host does not. Without its ActiveBuilding /
    // WorkersNeeded bookkeeping, the stock landmark activation rule cannot
    // drive the visible host's interior controller, despite valid graphics.
    if !active(host, 0x3064_5625) {
        return Err("BEHAVIOR_CONTAINER_LIGHTING: native lifecycle provider 30645625 is missing".into());
    }
    verify_toggle(host)
}
pub(super) fn exclude_child_garbage(unit: &mut PropertyFile) -> Result<(), String> {
    use super::behavior_templates::{array, key};
    let mut excluded = array(unit, EXCLUDED)?.to_vec();
    if excluded.iter().any(|v| matches!(v, Value::Key(key) if key.instance == GARBAGE)) {
        return Ok(());
    }
    excluded.push(key(GARBAGE));
    set_array_with_flags(unit, EXCLUDED, PropType::Key, excluded, 0x809C);
    Ok(())
}

pub(super) fn verify_child_garbage(unit: &PropertyFile) -> Result<(), String> {
    if active(unit, GARBAGE) {
        return Err("BEHAVIOR_CONTAINER_GARBAGE: child duplicates the host service".into());
    }
    Ok(())
}

pub(super) fn verify_toggle(unit: &PropertyFile) -> Result<(), String> {
    if !active(unit, TOGGLE) {
        return Err("BEHAVIOR_UNIT_TABLE: missing native building toggle provider".into());
    }
    Ok(())
}

/// Containers deliberately do not own the complete-building donor's business
/// fields or utility bundles. Keep the ordinary menu/geometry/locale checks,
/// then require the stronger component graph contract instead of a field count.
pub(super) fn validate_menu(
    resources: &Resources,
    instance: u32,
    donor: Option<(&Resources, u32)>,
    program: &super::behavior_nodes::ResolvedBehavior,
    panel_plan: Option<&super::panel::PanelPlan>,
) -> Result<super::menu_contract::MenuContractReport, String> {
    if program.unit_scope.as_deref() != Some(SCOPE) || program.units.is_empty() {
        return Err("BEHAVIOR_UNIT_SCOPE: container contract requires business components".into());
    }
    let mut report = super::menu_contract::validate(
        resources, instance, menu::lot_group(), donor, true,
    )?;
    let menu_bytes = resources.get(&ResourceId {
        type_id: PROPERTY_TYPE, group: menu::MENU_GROUP, instance,
    }).ok_or("BEHAVIOR_UNIT_SCOPE: missing container menu")?;
    let panel = PropertyFile::parse(menu_bytes).map_err(|e| e.to_string())?;
    for hash in [0x08F6_72C7, 0x09E6_83AC, 0x09E6_83AD, 0x09E9_1B9C, 0x0C7F_4336] {
        if panel.get(hash).is_none() {
            return Err(format!("BEHAVIOR_UNIT_SCOPE: missing container panel field {hash:08X}"));
        }
    }
    super::behavior_unit_build::verify(resources, instance, program)?;
    let host = PropertyFile::parse(resources.get(&ResourceId {
        type_id: PROPERTY_TYPE, group: menu::UNIT_GROUP, instance,
    }).ok_or("BEHAVIOR_CONTAINER_LIGHTING: missing host")?).map_err(|e|e.to_string())?;
    super::interior_lighting::verify_landmark_activation(&host)?;
    if let Some(panel) = panel_plan {
        super::behavior_container_ui::verify_presentation(resources, instance, program, Some(panel))?;
    } else {
        super::behavior_container_ui::verify(resources, instance, program)?;
    }
    super::behavior_container_bars::verify(resources, instance, program)?;
    report.checks.push(format!("business-free container; components={}; native group toggle providers verified; aggregate panel/runtime acceptance pending", program.units.len()));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_donors_garbage_service_and_amount_on_the_host_only() {
        let mut donor = common().unwrap();
        donor.values.retain(|property| property.hash != EXCLUDED);
        assert!(active(&donor, GARBAGE));
        set_scalar(&mut donor, GARBAGE_AMOUNT, PropType::Float, Value::Float(1.0));
        let host = PropertyFile::parse(&patch(&encode(&donor).unwrap()).unwrap()).unwrap();
        assert!(active(&host, GARBAGE));
        assert_eq!(host.get(GARBAGE_AMOUNT).and_then(|p| p.scalar()), Some(&Value::Float(1.0)));
        assert!(verify_child_garbage(&donor).is_err());
        exclude_child_garbage(&mut donor).unwrap();
        verify_child_garbage(&donor).unwrap();
        let inactive_host = PropertyFile::parse(&patch(&encode(&donor).unwrap()).unwrap()).unwrap();
        assert!(!active(&inactive_host, GARBAGE));
    }

    #[test]
    fn common_host_retains_control_and_identity_without_a_business() {
        let mut source = common().unwrap();
        set_scalar(
            &mut source,
            0x0A8A_41DA,
            PropType::Float,
            Value::Float(120000.0),
        );
        let host = PropertyFile::parse(&patch(&encode(&source).unwrap()).unwrap()).unwrap();
        verify(&host).unwrap();
        assert_eq!(
            host.get(0x0A8A_41DA).unwrap().kind,
            source.get(0x0A8A_41DA).unwrap().kind
        );
        for &bundle in BUSINESS {
            assert!(!active(&host, bundle));
        }
        assert!(active(&host, 0x3064_5625));
        let mut missing_lifecycle = host.clone();
        set_array_with_flags(&mut missing_lifecycle, EXCLUDED, PropType::Key,
            vec![Value::Key(sc_properties::Key {type_id:0, group:0, instance:0x3064_5625})], 0x809C);
        assert!(verify(&missing_lifecycle).unwrap_err().contains("LIGHTING"));
        let mut missing_toggle = host.clone();
        set_array_with_flags(&mut missing_toggle, EXCLUDED, PropType::Key,
            vec![Value::Key(sc_properties::Key {type_id:0, group:0, instance:TOGGLE})], 0x809C);
        assert!(verify(&missing_toggle).is_err());
        let mut housing_host = host.clone();
        let mut bundles = super::super::behavior_templates::array(&housing_host, ENABLE[3]).unwrap().to_vec();
        bundles.push(super::super::behavior_templates::key(0x933F_0FBF));
        set_array_with_flags(&mut housing_host, ENABLE[3], PropType::Key, bundles, 0x809C);
        assert!(verify(&housing_host).unwrap_err().contains("CONTAINER_RESIDENCE"));
        let mut staffed = host;
        set_scalar(&mut staffed, 0x0A79_17C3, PropType::Float, Value::Float(1.0));
        assert!(verify(&staffed).is_err());
    }
}
