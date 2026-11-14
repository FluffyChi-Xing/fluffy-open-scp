//! SC5 Asset Packager place_effects.py:24–74,125–156.
//! The source records are the author's hash-pinned Tokyo Tower and park
//! chains. They are inputs only. The output owns just its construction bundle.

use super::behavior::PROPERTY_TYPE;
use super::behavior_nodes::ResolvedBehavior;
use super::connector::Resources;
use super::menu::{HASH_PARENT, UNIT_GROUP, UNIT_TEMPLATE_GROUP, lot_group};
use super::native_profile::flat_chain;
use super::property_edit::{encode, set_array_with_flags, set_scalar};
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropType, PropertyFile, Value};
use sha2::{Digest, Sha256};

const BUNDLE_GROUP: u32 = 0x40E0_C700;
const CONSTRUCTION_SOURCE: u32 = 0x8A60_B969;
const CONSTRUCTION_FIXED: u32 = 0xF173_C172;
const CONSTRUCTION_ROW: u32 = 0x0808_0D58;
const CONSTRUCTION_RULE: u32 = 0xEC4F_A440;
const LAND_BUNDLE: u32 = 0x8BA6_D35D;
const A4: u32 = 0x0A02_09A4;
const A5: u32 = 0x0A02_09A5;
const EXCLUDED: u32 = 0x0A02_09B2;
const KEY_ARRAY_FLAGS: u16 = 0x809C;
const ACTIVE_UNIT: u32 = 0x084D_B48F;
const WILLIS_HOURLY: u32 = 0xCF11_6B9C;
const LAND_PAIRS: [u32; 10] = [
    0x0E58_5AE0,
    0x0E58_5AE2,
    0x0E58_5AE4,
    0x0E58_5AE6,
    0x0E58_5B00,
    0x0E58_5B02,
    0x0E58_5B04,
    0x0E58_5B06,
    0x0E58_5B20,
    0x0E58_5B24,
];

pub(super) fn selected(nodes: &[serde_json::Value]) -> bool {
    let mut tourism = false;
    for node in nodes {
        match node["type"].as_str() {
            Some("education") => return false,
            Some("residence") => {
                // Copan contributes its residential fields and bundles through
                // Templates::patch after the landmark Unit is built.
                if node["params"]["residenceProfile"]
                    != super::behavior_templates::RESIDENCE_PROFILE
                    && node["params"]["residenceProfile"] != super::behavior_residence_fill::PROFILE
                {
                    return false;
                }
            }
            Some("tourism") => {
                let params = &node["params"];
                if params["lodging"] == true
                    || !params["removeBundles"].is_null()
                    || !params["removeFields"].is_null()
                    || (!params["templateInstance"].is_null()
                        && super::behavior_nodes::hex(
                            &params["templateInstance"],
                            "tourism.templateInstance",
                        ) != Ok(0x4E01_E311))
                {
                    return false;
                }
                tourism = true;
            }
            _ => {}
        }
    }
    tourism
}

fn id(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: PROPERTY_TYPE,
        group,
        instance,
    }
}

fn bundle_key(instance: u32) -> Value {
    Value::Key(Key {
        type_id: PROPERTY_TYPE,
        group: BUNDLE_GROUP,
        instance,
    })
}

fn native_key(instance: u32) -> Value {
    Value::Key(Key {
        type_id: 0,
        group: 0,
        instance,
    })
}

fn anchors() -> Resources {
    macro_rules! anchor {
        ($group:expr, $instance:expr, $file:literal) => {
            (
                id($group, $instance),
                include_bytes!(concat!(
                    "../../resources/sc5/tourism-unit-anchors/",
                    $file,
                    ".bin"
                ))
                .to_vec(),
            )
        };
    }
    [
        anchor!(UNIT_GROUP, ACTIVE_UNIT, "084db48f"),
        anchor!(UNIT_GROUP, 0xFB67_60A4, "fb6760a4"),
        anchor!(UNIT_GROUP, 0xFD44_6873, "fd446873"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x2A25_CF66, "2a25cf66"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x34CE_7E16, "34ce7e16"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x34CE_7E17, "34ce7e17"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x4E01_E311, "4e01e311"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x565B_72BA, "565b72ba"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x565B_72BB, "565b72bb"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x593B_61D2, "593b61d2"),
        anchor!(UNIT_TEMPLATE_GROUP, 0x9EC4_74FF, "9ec474ff"),
        anchor!(UNIT_TEMPLATE_GROUP, 0xF1CF_C5B0, "f1cfc5b0"),
        anchor!(UNIT_TEMPLATE_GROUP, 0xF98B_CB35, "f98bcb35"),
        anchor!(BUNDLE_GROUP, CONSTRUCTION_SOURCE, "8a60b969"),
    ]
    .into_iter()
    .collect()
}

fn parse(resources: &Resources, resource: ResourceId) -> Result<PropertyFile, String> {
    let bytes = resources
        .get(&resource)
        .ok_or_else(|| format!("tourism contract: missing {resource}"))?;
    PropertyFile::parse(bytes).map_err(|error| format!("tourism contract: {resource}: {error}"))
}

fn array(file: &PropertyFile, hash: u32) -> Result<&[Value], String> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Array(values)) => Ok(values),
        _ => Err(format!("tourism contract: missing array {hash:08X}")),
    }
}

fn refers_to(value: &Value, instance: u32) -> bool {
    matches!(value, Value::Key(key) if key.instance == instance)
}

fn set_keys(file: &mut PropertyFile, hash: u32, values: Vec<Value>) {
    set_array_with_flags(file, hash, PropType::Key, values, KEY_ARRAY_FLAGS);
}

fn construction_bundle(anchors: &Resources) -> Result<Vec<u8>, String> {
    let mut donor = parse(anchors, id(BUNDLE_GROUP, CONSTRUCTION_SOURCE))?;
    donor
        .values
        .retain(|property| property.hash == CONSTRUCTION_ROW);
    if array(&donor, CONSTRUCTION_ROW)? != [native_key(CONSTRUCTION_RULE)] {
        return Err("tourism contract: construction rule 08080D58 → EC4FA440 changed".into());
    }
    encode(&donor)
}

fn allocate(resources: &Resources, unit: u32) -> ResourceId {
    for salt in 0u32.. {
        let mut hash = Sha256::new();
        hash.update(b"tourism-construction-bundle-v2");
        hash.update(unit.to_le_bytes());
        hash.update(salt.to_le_bytes());
        let instance = u32::from_le_bytes(hash.finalize()[..4].try_into().unwrap());
        let target = id(BUNDLE_GROUP, instance);
        if ![0, CONSTRUCTION_SOURCE, CONSTRUCTION_FIXED].contains(&instance)
            && !resources.contains_key(&target)
        {
            return target;
        }
    }
    unreachable!("an in-memory resource map cannot occupy every instance")
}

fn profile(
    anchors: &Resources,
    identity: &PropertyFile,
    program: &ResolvedBehavior,
    bundle: u32,
) -> Result<PropertyFile, String> {
    let (mut unit, _) = flat_chain(anchors, id(UNIT_GROUP, ACTIVE_UNIT))?;
    if unit
        .get(0x07E0_AD0A)
        .is_some_and(|property| matches!(&property.kind, Kind::Array(values) if !values.is_empty()))
        || unit
            .get(0x0CC8_BE66)
            .and_then(|property| property.scalar())
            .is_some_and(|value| value != &Value::Float(0.0))
    {
        return Err("tourism contract: native Unit has default submodules".into());
    }
    let mut active = array(&unit, A5).unwrap_or_default().to_vec();
    let mut excluded = array(&unit, EXCLUDED)?.to_vec();
    active.retain(|value| !refers_to(value, LAND_BUNDLE));
    excluded.retain(|value| !refers_to(value, LAND_BUNDLE));
    active.push(native_key(LAND_BUNDLE));
    for &instance in &program.ensure_bundles {
        let enabled = [0x0A02_09A2, 0x0A02_09A3, A4]
            .into_iter()
            .flat_map(|hash| array(&unit, hash).unwrap_or_default())
            .chain(active.iter())
            .any(|value| refers_to(value, instance));
        if !enabled {
            active.push(native_key(instance));
        }
        excluded.retain(|value| !refers_to(value, instance));
    }
    program.templates.prepare_landmark_residence(
        &mut unit,
        &mut active,
        &mut excluded,
        &program.remove_excluded_bundles,
    )?;
    set_keys(&mut unit, A5, active);
    set_keys(&mut unit, EXCLUDED, excluded);
    for property in &mut unit.values {
        if [0x0A02_09A2, 0x0A02_09A3, A4, A5].contains(&property.hash)
            && let Kind::Array(values) = &mut property.kind
        {
            values.retain(|value| {
                !program
                    .remove_bundles
                    .iter()
                    .any(|instance| refers_to(value, *instance))
            });
        }
    }

    // The author's default is medium land value. The existing OpenSCP
    // wealth override selects the high park donor for high wealth.
    let park = if program.set_int_fields.get(&0x0E58_5AE1) == Some(&3) {
        0xFD44_6873
    } else {
        0xFB67_60A4
    };
    let (park, _) = flat_chain(anchors, id(UNIT_GROUP, park))?;
    for hash in LAND_PAIRS.into_iter().flat_map(|hash| [hash, hash + 1]) {
        if let Some(property) = park.get(hash) {
            unit.values.retain(|record| record.hash != hash);
            unit.values.push(property.clone());
        }
    }

    unit.values.retain(|record| {
        ![
            HASH_PARENT,
            0x0E51_C90F,
            0x0EC5_D62E,
            0x0BF5_D2E3,
            0x0BF5_D2E4,
        ]
        .contains(&record.hash)
    });
    // 0DF1467C already comes from the flattened Tokyo Tower chain.
    set_scalar(&mut unit, 0x0CC8_BE51, PropType::Float, Value::Float(100.0));
    let mut references = array(&unit, A4)?.to_vec();
    if references
        .iter()
        .filter(|value| refers_to(value, CONSTRUCTION_SOURCE))
        .count()
        != 1
    {
        return Err("tourism contract: expected one native construction bundle in 0A0209A4".into());
    }
    for reference in &mut references {
        if refers_to(reference, CONSTRUCTION_SOURCE) {
            *reference = bundle_key(bundle);
        }
    }
    set_keys(&mut unit, A4, references);

    for (hash, value) in [
        (0x0E54_2F39, 400.0),
        (0x0D8C_29F6, 100.0),
        (0x0D8C_29FE, 50.0),
        (0x0D8C_2A01, 25.0),
        (0x0A79_17C8, 0.0),
    ] {
        set_scalar(&mut unit, hash, PropType::Float, Value::Float(value));
    }
    for (&hash, &value) in &program.set_float_fields {
        set_scalar(&mut unit, hash, PropType::Float, Value::Float(value as f32));
    }
    for (&hash, &value) in &program.set_int_fields {
        set_scalar(&mut unit, hash, PropType::Int32, Value::Int32(value));
    }
    if [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7]
        .iter()
        .all(|hash| program.set_float_fields.contains_key(hash))
    {
        let total = [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7]
            .iter()
            .map(|hash| program.set_float_fields[hash] as f32)
            .sum();
        set_scalar(&mut unit, 0x0A79_17C3, PropType::Float, Value::Float(total));
    }
    // behavior_profiles.py:106–123 protects the new identity and footprint.
    for hash in [
        0x08E1_7ED7,
        0x0ACF_FA64,
        0x09FB_78CB,
        0x0A8A_41DA,
        0x09AE_19D7,
    ] {
        unit.values.retain(|record| record.hash != hash);
        if let Some(property) = identity.get(hash) {
            unit.values.push(property.clone());
        }
    }
    Ok(unit)
}

pub(super) fn build(
    resources: &Resources,
    unit_id: ResourceId,
    program: &ResolvedBehavior,
) -> Result<(Vec<u8>, ResourceId, Vec<u8>), String> {
    let anchors = anchors();
    let bundle = allocate(resources, unit_id.instance);
    let identity = parse(resources, unit_id)?;
    let unit = profile(&anchors, &identity, program, bundle.instance)?;
    Ok((encode(&unit)?, bundle, construction_bundle(&anchors)?))
}

pub(super) fn construction_id(resources: &Resources, instance: u32) -> Result<ResourceId, String> {
    construction_reference(&parse(resources, id(UNIT_GROUP, instance))?)
}

fn construction_reference(unit: &PropertyFile) -> Result<ResourceId, String> {
    let mut references = array(unit, A4)?.iter().filter_map(|value| match value {
        Value::Key(key) if key.type_id == PROPERTY_TYPE && key.group == BUNDLE_GROUP => {
            Some(id(key.group, key.instance))
        }
        _ => None,
    });
    match (references.next(), references.next()) {
        (Some(bundle), None) => Ok(bundle),
        _ => Err("tourism contract: expected one typed construction bundle in 0A0209A4".into()),
    }
}

pub(super) fn verify(
    resources: &Resources,
    instance: u32,
    program: &ResolvedBehavior,
    ordinary_vehicle_path: bool,
) -> Result<serde_json::Value, String> {
    let anchors = anchors();
    let unit_id = id(UNIT_GROUP, instance);
    let unit = parse(resources, unit_id)?;
    let lot = parse(resources, id(lot_group(), instance))?;
    for hash in [0x0CA5_2005, super::interior_lighting::ACTIVE_CONTROLLER] {
        if lot.get(hash).and_then(|property| property.scalar()) != Some(&Value::Bool(true)) {
            return Err(format!(
                "tourism contract: Lot {hash:08X} must be Bool(true)"
            ));
        }
    }
    let toggles = array(&lot, 0x0CD4_E49C)?;
    if toggles.is_empty()
        || !toggles
            .iter()
            .all(|value| matches!(value, Value::UInt32(_)))
    {
        return Err("tourism contract: Lot 0CD4E49C needs a facility control list".into());
    }
    let bundle = construction_reference(&unit)?;
    program.templates.verify_landmark_residence(&unit)?;
    super::interior_lighting::verify_landmark_activation(&unit)?;
    if [0, CONSTRUCTION_SOURCE, CONSTRUCTION_FIXED].contains(&bundle.instance)
        || resources.get(&bundle) != Some(&construction_bundle(&anchors)?)
    {
        return Err(format!(
            "tourism contract: missing or altered construction bundle {bundle}"
        ));
    }
    for hash in [0x0A02_09A2, 0x0A02_09A3, A4, A5] {
        if array(&unit, hash)?
            .iter()
            .any(|value| refers_to(value, WILLIS_HOURLY))
        {
            return Err(format!(
                "tourism contract: Willis hourly bundle CF116B9C in {hash:08X}"
            ));
        }
    }
    let expected = encode(&profile(&anchors, &unit, program, bundle.instance)?)?;
    let expected = if program.templates.is_empty() {
        expected
    } else {
        program.templates.patch(&expected)?
    };
    let expected = match &program.office {
        Some(office) => office.patch(&expected)?,
        None => expected,
    };
    let expected = if ordinary_vehicle_path {
        let lot_bytes = resources
            .get(&id(lot_group(), instance))
            .ok_or("tourism contract: missing Lot")?;
        super::vehicle_paths::bind(&expected, lot_bytes)?.0
    } else {
        expected
    };
    if resources.get(&unit_id) != Some(&expected) {
        return Err("tourism contract: written Unit differs from the native tourism recipe".into());
    }
    Ok(serde_json::json!({
        "profile": "native-landmark-tourism-v1",
        "sourceUnit": "00B1B104:40E0C000:084DB48F",
        "constructionBundle": bundle.to_string(),
        "facilitySwitch": "0CA52005",
        "facilityControls": "0CD4E49C",
        "interiorController": "0F169C46",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::property_edit::set_array;
    use serde_json::json;

    #[test]
    fn tourism_only_uses_the_author_route_for_supported_new_menu_profiles() {
        let tourism = json!({"type":"tourism", "params":{"tourismProfile":"native-tourism-v1"}});
        assert!(selected(std::slice::from_ref(&tourism)));
        assert!(!selected(&[tourism.clone(), json!({"type":"education"})]));
        assert!(!selected(&[tourism.clone(), json!({"type":"residence"})]));
        assert!(selected(&[
            tourism,
            json!({"type":"residence", "params":{"residenceProfile":"copan-residence-v1"}}),
        ]));
        assert!(!selected(&[
            json!({"type":"tourism", "params":{"lodging":true}})
        ]));
        assert!(!selected(&[
            json!({"type":"tourism", "params":{"templateInstance":"0xDEADBEEF"}})
        ]));
    }

    #[test]
    fn copan_residence_keeps_its_initial_vacancy_and_intake_bindings() {
        let nodes = [json!({"type":"residence", "params":{"residenceProfile":"copan-residence-v1"}})];
        let program = ResolvedBehavior {
            templates: super::super::behavior_templates::select(&nodes).unwrap(),
            ..ResolvedBehavior::default()
        };
        let unit = profile(&anchors(), &PropertyFile::default(), &program, 0x0123_4567)
            .unwrap();
        let unit = program.templates.patch(&encode(&unit).unwrap()).unwrap();
        let unit = PropertyFile::parse(&unit).unwrap();
        assert_eq!(array(&unit, 0x0A88_D179).unwrap(), &[Value::Int32(49), Value::Int32(22)]);
        program.templates.verify_landmark_residence(&unit).unwrap();
        assert!(array(&unit, A5).unwrap().iter().any(|value| refers_to(value, 0xE648_41C9)));
    }

    #[test]
    fn written_tourism_requires_facility_controls_and_the_author_construction_bundle() {
        let program = ResolvedBehavior::default();
        let instance = 0x0123_4567;
        let unit_id = id(UNIT_GROUP, instance);
        let lot_id = id(lot_group(), instance);
        let mut resources = Resources::from([(unit_id, encode(&PropertyFile::default()).unwrap())]);
        let (unit, bundle_id, bundle) = build(&resources, unit_id, &program).unwrap();
        let built_unit = PropertyFile::parse(&unit).unwrap();
        assert!(built_unit.get(HASH_PARENT).is_none());
        assert!(built_unit.get(0x0E51_C90F).is_none());
        assert!(built_unit.get(0x0EC5_D62E).is_none());
        assert_eq!(
            array(&built_unit, A4).unwrap(),
            &[
                bundle_key(bundle_id.instance),
                native_key(0xBD99_9F77),
                native_key(0x4726_44E9),
                native_key(0x3A70_FAA2),
                native_key(0xC140_44C4),
                native_key(0xE153_69A2),
                native_key(0x75D2_2FD2),
                native_key(0x75D2_2FD1),
                native_key(0x75D2_2FD0),
                native_key(0xF0D5_FBAE),
                native_key(0x53BA_360B),
                native_key(0x5CB4_02D9),
            ]
        );
        assert_eq!(array(&built_unit, A5).unwrap(), [native_key(LAND_BUNDLE)]);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bundle)),
            "9dc22f3e1a681bf9881fcddd9379fedba1e85bc67725214f32952fd461e91fc0"
        );
        resources.insert(unit_id, unit);
        resources.insert(bundle_id, bundle.clone());
        assert_ne!(allocate(&resources, instance), bundle_id);

        let mut lot = PropertyFile::default();
        set_array(
            &mut lot,
            0x0CD4_E49C,
            PropType::UInt32,
            vec![Value::UInt32(0x5128_F316)],
        );
        for hash in [0x0CA5_2005, 0x0F16_9C46] {
            resources.insert(lot_id, encode(&lot).unwrap());
            assert!(
                verify(&resources, instance, &program, false)
                    .unwrap_err()
                    .contains(&format!("{hash:08X}"))
            );
            set_scalar(&mut lot, hash, PropType::Bool, Value::Bool(true));
        }
        resources.insert(lot_id, encode(&lot).unwrap());
        verify(&resources, instance, &program, false).unwrap();
        resources.remove(&bundle_id);
        assert!(
            verify(&resources, instance, &program, false)
                .unwrap_err()
                .contains("construction bundle")
        );
        resources.insert(bundle_id, bundle);
        lot.values.retain(|property| property.hash != 0x0CD4_E49C);
        resources.insert(lot_id, encode(&lot).unwrap());
        assert!(
            verify(&resources, instance, &program, false)
                .unwrap_err()
                .contains("0CD4E49C")
        );
    }
}
