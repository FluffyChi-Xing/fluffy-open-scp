use super::*;

const CAN_CREATE: u32 = 0x0832_7D48;
const BUNDLE: u32 = 0x40E0_C700;
const ZONE: u32 = 0x092F_9CB4;
const GRAPHICS_SELECTION: u32 = 0x0E5D_9DE0;
const CONSTRUCTION_PROGRESS: u32 = 0x5682_82EB;
const ON_CREATE: u32 = CAN_CREATE + 1;

pub(super) fn materialize_unit(
    packages: &[Package],
    unit: PropertyFile,
    chain: &[PropertyFile],
) -> Result<PropertyFile, String> {
    let (merged, merged_parents) = sc_properties::flatten_parent_inheritance_traced(unit, |id| {
        read_property(packages, property_id(id.group, id.instance)).ok()
    });
    if merged_parents.len() + 1 != chain.len() {
        return Err("RCI Unit inheritance was not fully materialized".into());
    }
    // The deployed v2 Unit only received bins from its own A3/A4 columns.
    // Verify every effective bundle slot and the exclusion list.
    for hash in (0x0A02_09A2..=0x0A02_09A5).chain([0x0A02_09B2]) {
        if merged.get(hash) != chain.iter().find_map(|file| file.get(hash)) {
            return Err(format!(
                "RCI effective bundle column {hash:08X} was not preserved"
            ));
        }
    }
    Ok(merged)
}

pub(super) fn inheritance(
    packages: &[Package],
    leaf: &PropertyFile,
) -> Result<Vec<PropertyFile>, String> {
    let mut chain = vec![leaf.clone()];
    let mut seen = std::collections::HashSet::new();
    while let Some(Value::Key(parent)) = chain
        .last()
        .and_then(|file| file.get(PARENT))
        .and_then(Property::scalar)
    {
        let id = property_id(parent.group, parent.instance);
        if !seen.insert(id) || seen.len() > 32 {
            return Err("invalid Unit inheritance chain".into());
        }
        chain.push(read_property(packages, id)?);
    }
    Ok(chain)
}

pub(super) fn is_growable(chain: &[PropertyFile], lot_id: ResourceId) -> bool {
    lot_id.group != menu::lot_group()
        && chain
            .iter()
            .find_map(|file| file.get(ZONE))
            .and_then(Property::scalar)
            .is_some_and(|value| matches!(value, Value::Key(key) if key.instance != 0))
}

pub(super) fn prepare(
    packages: &[Package],
    chain: &[PropertyFile],
    unit: &mut PropertyFile,
    lot: &PropertyFile,
    lot_id: ResourceId,
    output: &mut Vec<OverlayEntry>,
) -> Result<Vec<Json>, String> {
    let states = lot
        .get(STATES)
        .and_then(Property::array)
        .ok_or("missing RCI graphics state table")?;
    let names = lot
        .get(STATES + 1)
        .and_then(Property::array)
        .ok_or("missing RCI graphics state IDs")?;
    if states.len() != names.len() || states.is_empty() {
        return Err("RCI graphics state table and IDs must align".into());
    }
    let menu_entry = output
        .iter_mut()
        .find(|entry| entry.id == property_id(0x0987_8A01, lot_id.instance))
        .ok_or("missing generated RCI menu entry")?;
    let mut menu_file = PropertyFile::parse(&menu_entry.data).map_err(|error| error.to_string())?;
    // Use the community RCI tool shape under the original Plop template.
    // Keep the project's park category, generated localization and artwork.
    menu_file.values.retain(|prop| {
        matches!(
            prop.hash,
            0x0975_695F
                | 0x0977_AA8F
                | 0x0A09_F5FA
                | 0x0A09_F5FB
                | 0x0DB9_FC63
                | 0x0DC1_E3E0
                | 0x0DDE_EE56
        )
    });
    scalar(
        &mut menu_file,
        PARENT,
        PropType::Key,
        Value::Key(key(property_id(0x48E1_EB00, 0xAF04_2E9A))),
    );
    scalar(
        &mut menu_file,
        0x0975_695E,
        PropType::Key,
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: 0xF2FF_41C8,
        }),
    );
    // Region-mode placement uses a different ray-pick path. Use the
    // city cursor projection before applying the usual road snapping.
    scalar(
        &mut menu_file,
        0x0E3B_4147,
        PropType::Int32,
        Value::Int32(0),
    );
    scalar(
        &mut menu_file,
        0x0E82_87C8,
        PropType::Bool,
        Value::Bool(true),
    );
    scalar(
        &mut menu_file,
        GRAPHICS_SELECTION,
        PropType::Key,
        Value::Key(Key {
            type_id: 0,
            group: lot_id.group,
            instance: u32::MAX,
        }),
    );
    menu_entry.data = encode(&menu_file)?;

    let mut has_construction = false;
    let mut has_construction_initializer = false;
    let excluded = sc_properties::debug_tools::key_instances(unit, 0x0A02_09B2);
    let mut replacements: HashMap<ResourceId, ResourceId> = HashMap::new();
    let mut audit = Vec::new();
    for hash in 0x0A02_09A2..=0x0A02_09A5 {
        let Some(field) = chain.iter().find_map(|file| file.get(hash)) else {
            continue;
        };
        let mut field = field.clone();
        let Kind::Array(rows) = &mut field.kind else {
            continue;
        };
        let mut changed = false;
        for row in rows {
            let Value::Key(reference) = row else {
                return Err("invalid RCI bundle reference".into());
            };
            let id = property_id(
                if reference.group == 0 {
                    BUNDLE
                } else {
                    reference.group
                },
                reference.instance,
            );
            if excluded.contains(&id.instance) {
                continue;
            }
            let Ok(mut bundle) = read_property(packages, id) else {
                continue;
            };
            has_construction |= sc_properties::debug_tools::key_instances(&bundle, 0x07E0_AD23)
                .contains(&CONSTRUCTION_PROGRESS);
            if id.instance == 0x182D_2924 {
                has_construction_initializer =
                    sc_properties::debug_tools::key_instances(&bundle, ON_CREATE)
                        .contains(&0x6F51_DACC);
            }
            // Exact CanCreate-only donor records observed in the community package
            // for this construction family. Keep tax checks and other event slots.
            let rules = sc_properties::debug_tools::key_instances(&bundle, CAN_CREATE);
            if !matches!(
                (id.instance, rules.as_slice()),
                (0x08FD_D02F, [0x8F86_3629]) | (0x6B01_41B0, [0x8086_1E4D])
            ) {
                continue;
            }
            let replacement = if let Some(replacement) = replacements.get(&id) {
                *replacement
            } else {
                let seed = format!("vanilla_variant/rci/{}/{id}", lot_id.instance);
                let bytes = Sha256::digest(seed.as_bytes());
                let replacement =
                    property_id(id.group, u32::from_be_bytes(bytes[..4].try_into().unwrap()));
                let can_create = bundle
                    .values
                    .iter_mut()
                    .find(|p| p.hash == CAN_CREATE)
                    .unwrap();
                can_create.kind = Kind::Array(Vec::new());
                output.push(OverlayEntry::new(replacement, encode(&bundle)?));
                replacements.insert(id, replacement);
                audit.push(json!({"source": id.to_string(), "variant": replacement.to_string(), "removedCanCreate": rules}));
                replacement
            };
            reference.instance = replacement.instance;
            changed = true;
        }
        if changed {
            unit.values.retain(|property| property.hash != hash);
            unit.values.push(field);
        }
    }
    if !has_construction {
        return Err("RCI source is missing the original construction progress bundle".into());
    }
    // Industrial rule 427E0347 requires flag 0x2000, which the original
    // OnCreate rule sets. A progress bin alone can stay at zero indefinitely.
    if sc_properties::debug_tools::key_instances(unit, 0x0A02_09A4).contains(&0x7424_049C)
        && !has_construction_initializer
    {
        return Err("RCI construction scheduler requires its original OnCreate initializer".into());
    }
    Ok(audit)
}
