//! UnitLocation encoding and behavior-selected native vignette bindings.

use super::behavior_nodes::ResolvedBehavior;
use super::native_profile::{array, rotate_matrix};
use super::scene::{SceneDocument, SceneUnit};
use sc_properties::region_3d::fnv1_lower;
use sc_properties::{Key, Kind, PropType, Property, PropertyFile, Transform, Value};
use std::collections::{BTreeMap, HashSet};

const SPAWNER_IDS: u32 = 0x0E1B_AC61;
const SPAWNER_TRANSFORMS: u32 = 0x0E1B_AC62;
const VIGNETTE_ID: u32 = 0x0D2E_8A55;
const VIGNETTE_DEFINITION: u32 = 0x0D5A_28B8;
const VIGNETTE_SINK: u32 = 0x0D50_908D;
// cVignette::FillFromProps treats this native value like an omitted sink column.
const NO_SINK: u32 = 0x2CA3_3BDB;
pub(super) const VIGNETTE_GROUP: u32 = 0x40E1_E800;
pub(super) const LOCATION_COLUMNS: [u32; 2] = [SPAWNER_IDS, SPAWNER_TRANSFORMS];
pub(super) const LOT_COLUMNS: [u32; 5] = [
    VIGNETTE_ID,
    VIGNETTE_DEFINITION,
    VIGNETTE_SINK,
    SPAWNER_IDS,
    SPAWNER_TRANSFORMS,
];

/// The scene supplies every location. Empty location columns are omitted.
pub(super) fn compile_spawner_fields(
    scene: Option<&SceneDocument>,
    degrees: u32,
) -> Result<Vec<Property>, String> {
    let Some(scene) = scene else {
        return Ok(Vec::new());
    };
    let mut authored = authored_locations(scene, degrees)?;
    if authored.is_empty() {
        return Ok(Vec::new());
    }
    authored.sort_by_key(|(key, _)| key.instance);
    let (ids, transforms) = authored
        .into_iter()
        .map(|(id, transform)| (Value::Key(id), Value::Transform(transform)))
        .unzip();
    Ok(vec![
        array(SPAWNER_IDS, PropType::Key, ids),
        array(SPAWNER_TRANSFORMS, PropType::Transform, transforms),
    ])
}

fn authored_locations(scene: &SceneDocument, rotation: u32) -> Result<Vec<(Key, Transform)>, String> {
    let mut authored = Vec::new();
    for unit in &scene.units {
        let SceneUnit::Spawner { transform, id } = unit else {
            continue;
        };
        // The editor places spawners in lot space, outside the model transform.
        let matrix = rotate_matrix(&transform.matrix, rotation)?;
        authored.push((
            id.clone(),
            Transform {
                flags: 14,
                unknown: None,
                matrix: matrix.to_vec(),
            },
        ));
    }
    Ok(authored)
}

struct Recipe {
    lot: u32,
    snapshot: &'static [u8],
    preferred_slots: &'static [u32],
    // Lot-owned location names referenced by start/end, directed and improv
    // columns in each native definition. Actor-local IDs are not UnitLocations.
    definitions: &'static [(u32, &'static [u32])],
}

const ELEMENTARY: Recipe = Recipe {
    lot: 0xC264_A268,
    snapshot: include_bytes!("../../resources/sc5/spawner-elementary.bin"),
    preferred_slots: &[0x3B8A_DFE2, 0x3B8A_DFE5],
    definitions: &[
        (
            0x7A5C_0A91,
            &[
                0x125D_19FA,
                0x980F_070E,
                0x684C_5FCA,
                0x7007_70E0,
                0x33A4_F764,
                0x3B8A_DFE0,
                0x3B8A_DFE3,
                0x3B8A_DFE2,
                0x3B8A_DFE5,
            ],
        ),
        (
            0xBE2F_BD12,
            &[0x16C1_AE28, 0x16C1_AE2B, 0x16C1_AE2D, 0x16C1_AE2C],
        ),
    ],
};
const COLLEGE: Recipe = Recipe {
    lot: 0xA826_FA3E,
    snapshot: include_bytes!("../../resources/sc5/spawner-college.bin"),
    preferred_slots: &[0x3B8A_DFE3, 0x3B8A_DFE5, 0x3B8A_DFE0, 0x3B8A_DFE2],
    definitions: &[
        (0x00F0_F854, &[0x980F_070E, 0x684C_5FCA, 0x7007_70E0]),
        (
            0xACE2_5EE3,
            &[
                0x141E_ED53,
                0x3B8A_DFE0,
                0x3B8A_DFE3,
                0x3B8A_DFE2,
                0x3B8A_DFE5,
            ],
        ),
    ],
};
const UNIVERSITY: Recipe = Recipe {
    lot: 0x4250_AE35,
    snapshot: include_bytes!("../../resources/sc5/spawner-university.bin"),
    preferred_slots: COLLEGE.preferred_slots,
    definitions: &[
        (0xBBCC_6A6D, COLLEGE.definitions[1].1),
        (0xA429_4F4A, &[0x980F_070E, 0x684C_5FCA]),
    ],
};
const TOURISM: Recipe = Recipe {
    lot: 0x084D_B48F,
    snapshot: include_bytes!("../../resources/sc5/spawner-tourism.bin"),
    preferred_slots: &[0x3B8A_DFE0, 0x3B8A_DFE3, 0x3B8A_DFE2, 0x3B8A_DFE5],
    definitions: &[(0x6372_21E5, COLLEGE.preferred_slots)],
};

fn recipes(program: &ResolvedBehavior) -> Result<Vec<&'static Recipe>, String> {
    let mut selected = Vec::new();
    for signature in &program.education_signatures {
        let recipe = match signature {
            0x6C04_2367 => &ELEMENTARY,
            0x6C04_2366 => &COLLEGE,
            0x3256_C0A2 => &UNIVERSITY,
            _ => continue,
        };
        if !selected.is_empty() {
            return Err(
                "[spawner] education leaves select different native vignette recipes".into(),
            );
        }
        selected.push(recipe);
    }
    if program.native_tourism.is_some() || program.author_tourism {
        selected.push(&TOURISM);
    }
    Ok(selected)
}

pub(super) fn has_recipes(program: &ResolvedBehavior) -> Result<bool, String> {
    Ok(!recipes(program)?.is_empty())
}

fn read_array(file: &PropertyFile, hash: u32) -> Result<&[Value], String> {
    match file.get(hash).map(|property| &property.kind) {
        None => Ok(&[]),
        Some(Kind::Array(values)) => Ok(values),
        _ => Err(format!("[spawner] {hash:08X} must be an array")),
    }
}

fn locations(file: &PropertyFile) -> Result<Vec<(Key, Transform)>, String> {
    let ids = read_array(file, SPAWNER_IDS)?;
    let transforms = read_array(file, SPAWNER_TRANSFORMS)?;
    if ids.len() != transforms.len() {
        return Err("[spawner] UnitLocation IDs and transforms have different lengths".into());
    }
    ids.iter()
        .zip(transforms)
        .map(|(id, transform)| match (id, transform) {
            (Value::Key(id), Value::Transform(transform)) if id.instance != 0 => {
                Ok((id.clone(), transform.clone()))
            }
            _ => Err("[spawner] invalid UnitLocation row".into()),
        })
        .collect()
}

pub(super) fn verify_authored_locations(
    file: &PropertyFile,
    scene: &SceneDocument,
    rotation: u32,
) -> Result<serde_json::Value, String> {
    let mut written = BTreeMap::new();
    for (key, transform) in locations(file)? {
        if written.insert(key.instance, transform).is_some() {
            return Err(format!("[spawner] duplicate location {:08X}", key.instance));
        }
    }
    let expected = authored_locations(scene, rotation)?;
    let standard = standard_anchor_ids();
    let mut report = Vec::new();
    for (id, transform) in expected
        .iter()
        .filter(|(id, _)| !standard.contains(&id.instance))
    {
        if written.remove(&id.instance).as_ref() != Some(transform) {
            return Err(format!(
                "[spawner] authored location {:08X} missing or transformed incorrectly",
                id.instance
            ));
        }
        report.push(format!("{:08X}", id.instance));
    }
    for (id, transform) in expected
        .iter()
        .filter(|(id, _)| standard.contains(&id.instance))
    {
        // Generic names may be replaced by a recipe slot. Match each placed
        // transform once, after accounting for all explicitly named anchors.
        let matched = written
            .get(&id.instance)
            .filter(|actual| *actual == transform)
            .map(|_| id.instance)
            .or_else(|| written.iter().find_map(|(key, actual)| (actual == transform).then_some(*key)))
            .ok_or_else(|| format!("[spawner] authored location {:08X} is missing", id.instance))?;
        written.remove(&matched);
        report.push(format!("{:08X}", id.instance));
    }
    if !written.is_empty() {
        return Err("[spawner] output contains unplaced locations".into());
    }
    Ok(serde_json::json!({ "authored": report, "totalLocations": expected.len() }))
}

fn rows(file: &PropertyFile) -> Result<Vec<[Key; 3]>, String> {
    let ids = read_array(file, VIGNETTE_ID)?;
    let definitions = read_array(file, VIGNETTE_DEFINITION)?;
    let sinks = read_array(file, VIGNETTE_SINK)?;
    if ids.len() != definitions.len() || (!sinks.is_empty() && ids.len() != sinks.len()) {
        return Err("[spawner] native vignette columns have different lengths".into());
    }
    ids.iter()
        .zip(definitions)
        .enumerate()
        .map(|(index, (id, definition))| {
            let sink = match sinks.get(index) {
                None => Key {
                    instance: NO_SINK,
                    group: 0,
                    type_id: 0,
                },
                Some(Value::Key(key)) => key.clone(),
                _ => return Err("[spawner] native vignette sink is not a Key".into()),
            };
            match (id, definition) {
                (Value::Key(id), Value::Key(definition)) => {
                    Ok([id.clone(), definition.clone(), sink])
                }
                _ => Err("[spawner] native vignette row contains a non-Key value".into()),
            }
        })
        .collect()
}

fn standard_anchor_ids() -> HashSet<u32> {
    // Keep existing scene JSON compatible with suggestAnchorName() in the editor.
    std::iter::once(fnv1_lower("anchor"))
        .chain((2..1000).map(|index| fnv1_lower(&format!("anchor{index}"))))
        .collect()
}

pub(super) struct Bindings {
    pub(super) bytes: Vec<u8>,
    pub(super) native_definitions: HashSet<(u32, u32)>,
    pub(super) report: serde_json::Value,
}

pub(super) fn bind(
    bytes: &[u8],
    program: &ResolvedBehavior,
    scene: Option<&SceneDocument>,
    rotation: u32,
) -> Result<Option<Bindings>, String> {
    let recipes = recipes(program)?;
    if recipes.is_empty() {
        return Ok(None);
    }
    let mut asset =
        PropertyFile::parse(bytes).map_err(|error| format!("[spawner] Asset: {error}"))?;
    let mut positions = match scene {
        Some(scene) => authored_locations(scene, rotation)?,
        None => locations(&asset)?,
    };
    let mut vignette_rows = rows(&asset)?;
    if !program.education_signatures.is_empty() {
        // A stock Asset skeleton can carry the elementary scene independently
        // of the simulation leaf. Replace that school scene with the selection.
        vignette_rows.retain(|[_, key, _]| {
            ![&ELEMENTARY, &COLLEGE, &UNIVERSITY]
                .iter()
                .flat_map(|recipe| recipe.definitions)
                .any(|(id, _)| key.instance == *id)
        });
    }

    let mut native_definitions = HashSet::new();
    let mut slots = Vec::new();
    let mut occupied = HashSet::new();
    let authored_ids: HashSet<_> = positions.iter().map(|(key, _)| key.instance).collect();

    let mut selected_rows: Vec<[Key; 3]> = Vec::new();
    for recipe in &recipes {
        let donor = PropertyFile::parse(recipe.snapshot)
            .map_err(|error| format!("[spawner] native lot {:08X}: {error}", recipe.lot))?;
        let donor_rows = rows(&donor)?;
        let donor_locations: BTreeMap<_, _> = locations(&donor)?
            .into_iter()
            .map(|(key, transform)| (key.instance, (key, transform)))
            .collect();
        let mut required = Vec::new();
        for &(definition, anchors) in recipe.definitions {
            let row = donor_rows
                .iter()
                .find(|[_, key, _]| key.instance == definition && key.group == VIGNETTE_GROUP)
                .ok_or_else(|| {
                    format!(
                        "[spawner] lot {:08X} missing definition {definition:08X}",
                        recipe.lot
                    )
                })?;
            if row[0].instance == 0 || row[2].instance == 0 {
                return Err(format!(
                    "[spawner] incomplete native vignette {definition:08X}"
                ));
            }
            if let Some(previous) = selected_rows
                .iter()
                .find(|r| r[0].instance == row[0].instance)
            {
                if previous != row {
                    return Err(format!(
                        "[spawner] conflicting vignette ID {:08X}",
                        row[0].instance
                    ));
                }
            } else {
                selected_rows.push(row.clone());
            }
            native_definitions.insert((VIGNETTE_GROUP, definition));
            required.extend_from_slice(anchors);
        }
        for id in recipe.preferred_slots.iter().copied().chain(required) {
            if !occupied.insert(id) {
                continue;
            }
            let (key, stock) = donor_locations.get(&id).ok_or_else(|| {
                format!("[spawner] lot {:08X} missing anchor {id:08X}", recipe.lot)
            })?;
            if scene.is_some() {
                if !authored_ids.contains(&id) {
                    slots.push(key.clone());
                }
                continue;
            }
            let mut transform = stock.clone();
            transform.matrix = rotate_matrix(&transform.matrix, rotation)?.to_vec();
            if let Some(previous) = positions.iter_mut().find(|(k, _)| k.instance == id) {
                previous.1 = transform;
            } else {
                positions.push((key.clone(), transform));
            }
        }
    }
    let mut mapped = Vec::new();
    if scene.is_some() {
        let standard = standard_anchor_ids();
        let generic = positions.iter_mut().filter(|(key, _)| {
            standard.contains(&key.instance) && !occupied.contains(&key.instance)
        });
        for ((key, _), slot) in generic.zip(slots) {
            mapped.push(format!("{:08X}", slot.instance));
            *key = slot;
        }
    }
    for row in &selected_rows {
        vignette_rows.retain(|existing| existing[0].instance != row[0].instance);
        vignette_rows.push(row.clone());
    }
    for (index, hash) in [VIGNETTE_ID, VIGNETTE_DEFINITION, VIGNETTE_SINK]
        .into_iter()
        .enumerate()
    {
        write_array(
            &mut asset,
            hash,
            PropType::Key,
            vignette_rows
                .iter()
                .map(|row| Value::Key(row[index].clone()))
                .collect(),
        );
    }
    let (ids, transforms) = positions
        .iter()
        .map(|(key, transform)| (Value::Key(key.clone()), Value::Transform(transform.clone())))
        .unzip();
    asset.values.retain(|property| !LOCATION_COLUMNS.contains(&property.hash));
    if !positions.is_empty() {
        write_array(&mut asset, SPAWNER_IDS, PropType::Key, ids);
        write_array(&mut asset, SPAWNER_TRANSFORMS, PropType::Transform, transforms);
    }
    let bytes = super::property_edit::encode(&asset)?;
    let output =
        PropertyFile::parse(&bytes).map_err(|error| format!("[spawner] output: {error}"))?;
    if locations(&output)? != positions || rows(&output)? != vignette_rows {
        return Err("[spawner] compiled Asset lost a required native vignette binding".into());
    }
    if let Some(scene) = scene {
        verify_authored_locations(&output, scene, rotation)?;
    }
    Ok(Some(Bindings {
        bytes,
        native_definitions,
        report: serde_json::json!({
            "nativeLots": recipes.iter().map(|recipe| format!("{:08X}", recipe.lot)).collect::<Vec<_>>(),
            "vignettes": selected_rows.iter().map(|row| format!("{:08X}", row[1].instance)).collect::<Vec<_>>(),
            "mappedAnchors": mapped,
        }),
    }))
}

fn write_array(file: &mut PropertyFile, hash: u32, kind: PropType, values: Vec<Value>) {
    let mut property = array(hash, kind, values);
    property.encoding.flags = 0x809C;
    file.values.retain(|existing| existing.hash != hash);
    file.values.push(property);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::behavior_nodes::compile;
    use serde_json::json;

    #[test]
    fn spawner_recipes_resolve_the_native_definition_rows() {
        let empty = super::super::property_edit::encode(&PropertyFile::default()).unwrap();
        for (signature, expected) in [
            ("0x6C042367", vec!["7A5C0A91", "BE2FBD12"]),
            ("0x6C042366", vec!["00F0F854", "ACE25EE3"]),
            ("0x3256C0A2", vec!["BBCC6A6D", "A4294F4A"]),
        ] {
            let program = compile(
                None,
                &[json!({
                    "id": "school",
                    "type": "education",
                    "params": { "signatureBundle": signature },
                })],
            )
            .unwrap();
            let output = bind(&empty, &program, None, 0).unwrap().unwrap();
            assert_eq!(output.report["vignettes"], json!(expected));
            let file = PropertyFile::parse(&output.bytes).unwrap();
            file.validate_declared_strides().unwrap();
            assert_eq!(rows(&file).unwrap().len(), 2);
        }
        let park = compile(None, &[json!({ "id": "park", "type": "park" })]).unwrap();
        assert!(bind(&empty, &park, None, 0).unwrap().is_none());
    }

    #[test]
    fn spawner_combines_leaves_and_maps_generic_and_named_anchors() {
        let program = compile(
            None,
            &[
                json!({ "id": "school", "type": "education", "params": {
                    "signatureBundle": "0x6C042366"
                }}),
                json!({ "id": "visitors", "type": "tourism", "params": {
                    "tourismProfile": "native-tourism-v1"
                }}),
            ],
        )
        .unwrap();
        let unit = |instance, z| SceneUnit::Spawner {
            id: Key {
                instance,
                group: 0,
                type_id: 0,
            },
            transform: Transform {
                flags: 14,
                unknown: None,
                matrix: vec![1., 0., 0., 0., 1., 0., 0., 0., 1., 2., 3., z],
            },
        };
        let scene = SceneDocument {
            units: vec![
                unit(fnv1_lower("anchor"), 4.),
                unit(fnv1_lower("anchor2"), 42.),
                unit(0x3B8A_DFE0, 8.),
            ],
            model_transform: Some([1., 0., 0., 0., 1., 0., 0., 0., 1., 50., 0., 0.]),
            ..SceneDocument::default()
        };
        let old = include_bytes!("../../resources/sc5/elementary-school-asset.bin");
        let output = bind(old, &program, Some(&scene), 90).unwrap().unwrap();
        assert_eq!(
            output.report["vignettes"],
            json!(["00F0F854", "ACE25EE3", "637221E5"])
        );
        assert_eq!(
            output.report["mappedAnchors"],
            json!(["3B8ADFE3", "3B8ADFE5"])
        );
        let file = PropertyFile::parse(&output.bytes).unwrap();
        assert_eq!(rows(&file).unwrap().len(), 3);
        let locations = locations(&file).unwrap();
        assert_eq!(locations.len(), 3);
        for (id, z) in [(0x3B8A_DFE3, 4.), (0x3B8A_DFE5, 42.), (0x3B8A_DFE0, 8.)] {
            let (_, transform) = locations
                .iter()
                .find(|(key, _)| key.instance == id)
                .unwrap();
            assert_eq!(&transform.matrix[9..], &[-3., 2., z]);
        }
        let empty_scene = SceneDocument::default();
        let empty = bind(old, &program, Some(&empty_scene), 90).unwrap().unwrap();
        let file = PropertyFile::parse(&empty.bytes).unwrap();
        assert!(super::locations(&file).unwrap().is_empty());
    }
}
