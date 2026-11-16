use super::*;
use std::collections::{BTreeMap, BTreeSet};

const PREPLACED: u32 = 0x07E0_AD0A;
const PLACEMENT_MODULES: u32 = 0x0E9F_828E;
const SUBMENU: u32 = 0x0AFE_27CA;
const CATEGORIES: u32 = 0x0DB9_FC63;
const MENU_POSITIONS: u32 = 0x0DC1_E3E0;
const MENU_GROUP: u32 = 0x0987_8A01;
const OUTPUT: Field = (0x0AFB_9881, "electricity");
const WIND: Field = (0x0D2F_EBC0, "windElectricity");
type Field = (u32, &'static str);

const FOSSIL: &[Field] = &[
    OUTPUT,
    (0x0BBB_D59A, "fuel"),
    (0x0C13_1CC4, "airPollution"),
    (0x0C13_1CFC, "groundPollution"),
];

struct Plant {
    storage: Option<Field>,
    modules: &'static [u32],
    fields: &'static [Field],
}

fn profile(id: ResourceId) -> Option<Plant> {
    if id.type_id != TYPE || id.group != UNIT {
        return None;
    }
    let plant = match id.instance {
        0xB592_3139 => Plant {
            storage: None,
            modules: &[0xD552_C1F3, 0x2C75_D49B, 0x67B0_22FF],
            fields: &[WIND],
        },
        0x94C2_43EB => Plant {
            storage: Some((0x0DB7_5506, "coalStorage")),
            modules: &[0x2EFE_7959, 0x84AD_36A0, 0x9AC0_6C93],
            fields: FOSSIL,
        },
        0x6E29_6726 => Plant {
            storage: Some((0x0DB7_5503, "oilStorage")),
            modules: &[0x3A29_00B1, 0xA2EE_7C8C, 0x3B10_72ED],
            fields: FOSSIL,
        },
        0x9122_C84D => Plant {
            storage: None,
            modules: &[0xA230_F2DC, 0x0300_4230],
            fields: &[OUTPUT],
        },
        0xDA87_B792 => Plant {
            storage: None,
            modules: &[0x5120_1103, 0xF272_F26C, 0xEBC3_621B],
            fields: &[OUTPUT],
        },
        _ => return None,
    };
    Some(plant)
}

pub(super) fn supports(id: ResourceId) -> bool {
    profile(id).is_some()
}

fn float(chain: &[PropertyFile], (hash, name): Field) -> Result<f32, String> {
    match chain
        .iter()
        .find_map(|file| file.get(hash))
        .and_then(Property::scalar)
    {
        Some(Value::Float(value)) if value.is_finite() && *value >= 0.0 => Ok(*value),
        _ => Err(format!(
            "original power property {name} ({hash:08X}) is missing or invalid"
        )),
    }
}

fn read_key(file: &PropertyFile, hash: u32) -> Option<Key> {
    match file.get(hash)?.scalar()? {
        Value::Key(key) if key.instance != 0 => Some(*key),
        _ => None,
    }
}

fn keys(file: &PropertyFile, hash: u32) -> impl Iterator<Item = &Key> {
    file.get(hash)
        .and_then(Property::array)
        .into_iter()
        .flatten()
        .filter_map(|v| match v {
            Value::Key(key) => Some(key),
            _ => None,
        })
}

struct Source {
    container: u32,
    menus: BTreeMap<u32, PropertyFile>,
}

fn source_menu(
    packages: &[Package],
    chain: &[PropertyFile],
    plant: &Plant,
) -> Result<Source, String> {
    let container = chain
        .iter()
        .find_map(|file| read_key(file, SUBMENU))
        .ok_or("original power plant has no module menu")?
        .instance;
    let ids = packages
        .iter()
        .flat_map(|package| package.entries())
        .filter(|entry| entry.id.type_id == TYPE && entry.id.group == MENU_GROUP)
        .map(|entry| entry.id.instance)
        .collect::<BTreeSet<_>>();
    let mut menus = BTreeMap::new();
    for id in ids {
        let file = read_property(packages, property_id(MENU_GROUP, id))?;
        if keys(&file, CATEGORIES).any(|key| key.instance == container) {
            menus.insert(id, file);
        }
    }
    for instance in plant.modules {
        if !menus.values().any(|file| {
            read_key(file, menu::HASH_MENU_UNIT).is_some_and(|key| key.instance == *instance)
        }) {
            return Err(format!(
                "original power module {instance:08X} is missing from its menu"
            ));
        }
    }
    Ok(Source { container, menus })
}

fn original_module(
    packages: &[Package],
    instance: u32,
) -> Result<(PropertyFile, ResourceId, PropertyFile), String> {
    let unit = read_property(packages, property_id(UNIT, instance))?;
    let link = read_key(&unit, LOT_LINK)
        .filter(|key| key.instance == instance && matches!(key.type_id, 0 | TYPE))
        .ok_or_else(|| format!("original power module {instance:08X} has no valid Lot link"))?;
    let lot_id = property_id(link.group, link.instance);
    let lot = read_property(packages, lot_id)?;
    Ok((unit, lot_id, lot))
}

pub(super) fn describe(
    packages: &[Package],
    id: ResourceId,
    chain: &[PropertyFile],
) -> Result<Json, String> {
    let Some(plant) = profile(id) else {
        return Ok(Json::Null);
    };
    source_menu(packages, chain, &plant)?;
    let mut groups = serde_json::Map::new();
    let mut describe_group =
        |id: String, fields: &[Field], chain: &[PropertyFile]| -> Result<(), String> {
            let mut result = serde_json::Map::new();
            for &field @ (hash, name) in fields {
                let value = float(chain, field)?;
                result.insert(
                    name.into(),
                    json!({"hash":format!("{hash:08X}"),"original":value,"value":value}),
                );
            }
            groups.insert(id, Json::Object(result));
            Ok(())
        };
    if let Some(storage) = plant.storage {
        describe_group("plant".into(), &[storage], chain)?;
    }
    for &instance in plant.modules {
        let (unit, _, _) = original_module(packages, instance)?;
        let chain = rci::inheritance(packages, &unit)?;
        describe_group(format!("{instance:08X}"), plant.fields, &chain)?;
    }
    Ok(Json::Object(groups))
}

fn overrides(
    values: &Json,
    fields: &[Field],
    chain: &[PropertyFile],
) -> Result<Vec<(u32, f32)>, String> {
    if values.is_null() {
        return Ok(Vec::new());
    }
    let values = values.as_object().ok_or("invalid power fields")?;
    values
        .iter()
        .filter_map(|(key, row)| {
            let Some(&field @ (hash, name)) = fields.iter().find(|(_, name)| *name == key) else {
                return Some(Err(format!("unsupported power field {key}")));
            };
            let Some(value) = row["value"].as_f64() else {
                return Some(Err(format!("missing value for {name}")));
            };
            match float(chain, field) {
                Ok(original) if f64::from(original) == value => return None,
                Err(error) => return Some(Err(error)),
                _ => {}
            }
            let min = if matches!(name, "airPollution" | "groundPollution") {
                0.0
            } else {
                1.0
            };
            if !(min..=16_777_216.0).contains(&value)
                || value.fract() != 0.0
                || (name == "windElectricity" && value % 4.0 != 0.0)
            {
                return Some(Err(format!(
                    "invalid integer or wind output increment for {name}"
                )));
            }
            Some(Ok((hash, value as f32)))
        })
        .collect()
}

fn retarget(file: &mut PropertyFile, hash: u32, replacements: &BTreeMap<u32, u32>) {
    let Some(p) = file.values.iter_mut().find(|p| p.hash == hash) else {
        return;
    };
    let values = match &mut p.kind {
        Kind::Scalar(value) => std::slice::from_mut(value),
        Kind::Array(values) => values.as_mut_slice(),
        Kind::Empty => return,
    };
    for value in values {
        if let Value::Key(key) = value
            && let Some(&instance) = replacements.get(&key.instance)
        {
            key.instance = instance;
        }
    }
}

pub(super) fn apply(
    packages: &[Package],
    original: ResourceId,
    new_unit: ResourceId,
    values: &Json,
    unit: &mut PropertyFile,
    output: &mut Vec<OverlayEntry>,
) -> Result<Vec<Json>, String> {
    if values.is_null() {
        return Ok(Vec::new());
    }
    let plant =
        profile(original).ok_or("power editing is unsupported for the selected original")?;
    let groups = values.as_object().ok_or("invalid power groups")?;
    for group in groups.keys() {
        if !(group == "plant" && plant.storage.is_some())
            && !plant.modules.iter().any(|id| *group == format!("{id:08X}"))
        {
            return Err(format!("unsupported power group {group}"));
        }
    }
    let chain = rci::inheritance(packages, &read_property(packages, original)?)?;
    let mut report = Vec::new();
    let mut fuel_capacity = None;
    if let Some(storage) = plant.storage {
        fuel_capacity = Some(float(&chain, storage)?);
        for (hash, value) in overrides(&values["plant"], &[storage], &chain)? {
            fuel_capacity = Some(value);
            scalar(unit, hash, PropType::Float, Value::Float(value));
            report.push(
                json!({"unit":new_unit.to_string(),"field":format!("{hash:08X}"),"value":value}),
            );
        }
    }

    let mut changed = BTreeMap::new();
    for &instance in plant.modules {
        let field_values = &values[format!("{instance:08X}")];
        if field_values.is_null() && fuel_capacity.is_none() {
            continue;
        }
        let (child, lot_id, lot) = original_module(packages, instance)?;
        let chain = rci::inheritance(packages, &child)?;
        let edits = overrides(field_values, plant.fields, &chain)?;
        if let Some(capacity) = fuel_capacity {
            let fuel = (0x0BBB_D59A, "fuel");
            let amount = edits
                .iter()
                .find(|(hash, _)| *hash == fuel.0)
                .map(|(_, value)| *value)
                .unwrap_or(float(&chain, fuel)?);
            if capacity < amount {
                return Err(format!(
                    "power plant storage is smaller than one burn of module {instance:08X}"
                ));
            }
        }
        if !edits.is_empty() {
            changed.insert(instance, (child, lot_id, lot, edits));
        }
    }
    if changed.is_empty() {
        return Ok(report);
    }

    let source = source_menu(packages, &chain, &plant)?;
    let mut used = packages
        .iter()
        .flat_map(|package| package.entries())
        .map(|entry| entry.id.instance)
        .chain(output.iter().map(|entry| entry.id.instance))
        .chain([new_unit.instance])
        .collect::<BTreeSet<_>>();
    let mut allocate = |role: &str, instance| -> Result<u32, String> {
        let input = format!(
            "openscp:power:{:08X}:{role}:{instance:08X}",
            new_unit.instance
        );
        let bytes = Sha256::digest(input.as_bytes());
        let id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if id == 0 || !used.insert(id) {
            Err(format!("power identity collision ({role} {instance:08X})"))
        } else {
            Ok(id)
        }
    };
    let container = allocate("container", source.container)?;
    let mut replacements = BTreeMap::new();
    for (instance, (mut child, lot_id, mut lot, edits)) in changed {
        let target = allocate("module", instance)?;
        let target_lot = property_id(lot_id.group, target);
        let module_map = BTreeMap::from([(instance, target)]);
        retarget(&mut child, LOT_LINK, &module_map);
        retarget(&mut lot, STATES, &module_map);
        for &(hash, value) in &edits {
            scalar(&mut child, hash, PropType::Float, Value::Float(value));
        }
        let target_unit = property_id(UNIT, target);
        output.push(OverlayEntry::new(target_unit, encode(&child)?));
        output.push(OverlayEntry::new(target_lot, encode(&lot)?));
        replacements.insert(instance, target);
        report.push(json!({"original":property_id(UNIT,instance).to_string(),"unit":target_unit.to_string(),"lot":target_lot.to_string(),"fields":edits}));
    }

    if unit.get(PREPLACED).is_none()
        && let Some(preplaced) = chain.iter().find_map(|file| file.get(PREPLACED))
    {
        unit.values.push(preplaced.clone());
    }
    retarget(unit, PREPLACED, &replacements);
    let mut link = chain
        .iter()
        .find_map(|file| read_key(file, SUBMENU))
        .ok_or("power submenu link missing")?;
    link.instance = container;
    scalar(unit, SUBMENU, PropType::Key, Value::Key(link));
    for (id, mut file) in source.menus {
        let category_count = keys(&file, CATEGORIES).count();
        let (category_index, category) = keys(&file, CATEGORIES)
            .copied()
            .enumerate()
            .find(|(_, key)| key.instance == source.container)
            .ok_or("power module menu lost its category")?;
        let category = Key {
            instance: container,
            ..category
        };
        if category_count > 1
            && let Some(positions) = file.values.iter_mut().find(|p| p.hash == MENU_POSITIONS)
            && let Kind::Array(rows) = &mut positions.kind
            && rows.len() == category_count
        {
            *rows = vec![rows[category_index].clone()];
        }
        let property = file
            .values
            .iter_mut()
            .find(|p| p.hash == CATEGORIES)
            .ok_or("power module categories missing")?;
        property.kind = Kind::Array(vec![Value::Key(category)]);
        retarget(&mut file, menu::HASH_MENU_UNIT, &replacements);
        retarget(&mut file, PLACEMENT_MODULES, &replacements);
        output.push(OverlayEntry::new(
            property_id(MENU_GROUP, allocate("menu", id)?),
            encode(&file)?,
        ));
    }
    Ok(report)
}
