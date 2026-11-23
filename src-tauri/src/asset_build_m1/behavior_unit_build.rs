//! Secondary-unit assembly using CreateSecondaryUnits' parallel property table.
//! Native references: EP1 50E18927's model-free, non-base config; retail
//! CreateSecondaryUnits/ConnectUnits at 647416/634933 in the retained decompile.
use super::{
    behavior,
    behavior_nodes::ResolvedBehavior,
    connector::Resources,
    menu,
    package::BuildResources,
    property_edit::{encode, set_array, set_scalar},
    stages::BuildContext,
};
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropType, PropertyFile, Value};
use sha2::{Digest, Sha256};

const CHILDREN: u32 = 0x07E0_AD0A;
const OFFSETS: u32 = 0x07E0_AD0B;
const CONNECT: u32 = 0x0D36_3DF3;
const CONFIG: u32 = 0x08E1_7ED7;
const PATHS: [u32; 7] = [
    0x0CC8_FE7A,
    0x0CAA_6841,
    0x0CAA_6832,
    0x0D9A_4E54,
    0x0D9A_A2BB,
    0x0CAA_680D,
    0x0CB0_0ED8,
];

fn id(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: behavior::PROPERTY_TYPE,
        group,
        instance,
    }
}
fn key(group: u32, instance: u32) -> Value {
    Value::Key(Key {
        type_id: behavior::PROPERTY_TYPE,
        group,
        instance,
    })
}
pub(super) fn instance(host: u32, scope: &str) -> u32 {
    let mut hash = Sha256::new();
    hash.update(b"openscp-independent-behavior-unit-v1\0");
    hash.update(host.to_le_bytes());
    hash.update(scope.as_bytes());
    u32::from_le_bytes(hash.finalize()[..4].try_into().unwrap())
}
fn parse(resources: &Resources, resource: ResourceId) -> Result<PropertyFile, String> {
    PropertyFile::parse(
        resources
            .get(&resource)
            .ok_or_else(|| format!("BEHAVIOR_UNIT_RESOURCE: {resource}"))?,
    )
    .map_err(|e| format!("BEHAVIOR_UNIT_RESOURCE: {resource}: {e}"))
}
fn values(file: &PropertyFile, hash: u32) -> Result<Vec<Value>, String> {
    match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(v)) => Ok(v.clone()),
        None => Ok(vec![]),
        _ => Err(format!("BEHAVIOR_UNIT_TABLE: {hash:08X}")),
    }
}

fn attach(unit: &mut PropertyFile, child: u32) -> Result<(), String> {
    let mut children = values(unit, CHILDREN)?;
    if children
        .iter()
        .any(|v| matches!(v,Value::Key(k) if k.instance==child))
    {
        return Err("BEHAVIOR_UNIT_TABLE: duplicate child".into());
    }
    let count = children.len();
    for (hash, ty, default) in [
        (OFFSETS, PropType::Vector3, Value::Vector3([0.0; 3])),
        (0x0863_5E14, PropType::Float, Value::Float(0.0)),
        (0x0863_5E15, PropType::Vector3, Value::Vector3([0.0; 3])),
        (0x0863_5E13, PropType::Float, Value::Float(1.0)),
        (CONNECT, PropType::Bool, Value::Bool(false)),
    ] {
        let mut row = values(unit, hash)?;
        if row.is_empty() && hash != OFFSETS {
            row.resize(count, default.clone());
        }
        if row.len() != count {
            return Err(format!("BEHAVIOR_UNIT_TABLE: {hash:08X} row count"));
        }
        row.push(if hash == CONNECT {
            Value::Bool(true)
        } else {
            default
        });
        set_array(unit, hash, ty, row);
        if hash == CONNECT {
            // Bool has one byte per row. The shared legacy array helper does
            // not declare a Bool stride; keep this new table's encoding local.
            unit.values
                .iter_mut()
                .find(|p| p.hash == CONNECT)
                .unwrap()
                .encoding
                .array_item_size = Some(1);
        }
    }
    children.push(key(menu::UNIT_GROUP, child));
    set_array(unit, CHILDREN, PropType::Key, children);
    Ok(())
}

fn child_lot(host: &PropertyFile) -> Result<Vec<u8>, String> {
    let mut lot = PropertyFile::default();
    // Exact native invisible secondary config identity, not the host's model.
    set_scalar(
        &mut lot,
        menu::HASH_PARENT,
        PropType::Key,
        key(0x40E1_C000, 0x2D7B_DB01),
    );
    set_scalar(&mut lot, 0x0AD9_0138, PropType::Bool, Value::Bool(false));
    set_scalar(&mut lot, 0x0AFE_2FEB, PropType::Bool, Value::Bool(true));
    super::behavior_container_ui::copy_bounds(host, &mut lot);
    for hash in PATHS {
        if let Some(property) = host.get(hash) {
            lot.values.push(property.clone());
        }
    }
    verify_paths(&lot)?;
    encode(&lot)
}

fn verify_paths(lot: &PropertyFile) -> Result<(), String> {
    let entries = values(lot, PATHS[0])?;
    let pairs = values(lot, PATHS[1])?;
    let indices = values(lot, PATHS[2])?;
    let points = values(lot, PATHS[5])?;
    if entries.is_empty()
        || pairs.len() != entries.len() * 2
        || values(lot, PATHS[3])?.len() != entries.len()
        || values(lot, PATHS[4])?.len() != entries.len()
        || points.is_empty()
        || indices.is_empty()
    {
        return Err("BEHAVIOR_UNIT_PATH: incomplete path tables".into());
    }
    let index = |v: &Value| match v {
        Value::Int32(n) if *n >= 0 => Some(*n as usize),
        Value::UInt32(n) => Some(*n as usize),
        _ => None,
    };
    if indices
        .iter()
        .any(|v| index(v).is_none_or(|i| i >= points.len()))
        || pairs
            .iter()
            .any(|v| index(v).is_none_or(|i| i > indices.len()))
    {
        return Err("BEHAVIOR_UNIT_PATH: path index out of bounds".into());
    }
    Ok(())
}

pub(super) fn apply(
    ctx: &mut BuildContext,
    program: &ResolvedBehavior,
    seed: &[u8],
) -> Result<serde_json::Value, String> {
    if program.units.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    let host = ctx
        .menu_instance
        .ok_or("BEHAVIOR_UNIT_RESOURCE: isolated units require new menu")?;
    let host_id = id(menu::UNIT_GROUP, host);
    let host_lot = parse(ctx.resources.all(), id(menu::lot_group(), host))?;
    let mut host_unit = parse(ctx.resources.all(), host_id)?;
    let mut additions = Resources::new();
    let mut report = vec![];
    for scope in &program.units {
        if scope.program.title_override.is_some() || scope.program.presentation.is_some() {
            return Err(
                "BEHAVIOR_UNIT_PRESENTATION: child may not overwrite host presentation".into(),
            );
        }
        let child = instance(host, &scope.id);
        if child == 0
            || child == host
            || [menu::UNIT_GROUP, menu::lot_group(), menu::MENU_GROUP]
                .iter()
                .any(|&group| {
                    ctx.resources.contains_key(&id(group, child))
                        || additions.contains_key(&id(group, child))
                })
        {
            return Err("BEHAVIOR_UNIT_RESOURCE: identity collision".into());
        }
        let raw = serde_json::json!({"$schema":"openscp.asset","schema_version":2,
            "id":"independent-behavior-unit","kind":"building-asset",
            "reference":{"package":"source.package","lot":format!("00B1B104:40E1C000:{child:08X}")},
            "scene":{"document":"asset.lot.json"},
            "outputs":{"package":"independent.package","mode":"full"}})
        .to_string();
        let mut plan = super::plan::plan_from_schema(&raw)?;
        plan.behavior_program = Some(scope.program.clone());
        plan.menu = ctx.plan.menu.clone();
        plan.selected_menu_categories = ctx.plan.selected_menu_categories.clone();
        let mut child_ctx = BuildContext::new(ctx.root.clone(), plan);
        child_ctx.menu_instance = Some(child);
        child_ctx.presentation_resources = ctx.presentation_resources.clone();
        child_ctx.resources = BuildResources::from_source(ctx.resources.all().clone());
        let mut unit = PropertyFile::parse(seed).map_err(|e| e.to_string())?;
        unit.values.retain(|p| {
            ![
                CHILDREN,
                OFFSETS,
                CONNECT,
                0x0863_5E13,
                0x0863_5E14,
                0x0863_5E15,
            ]
            .contains(&p.hash)
        });
        set_scalar(
            &mut unit,
            CONFIG,
            PropType::Key,
            key(menu::lot_group(), child),
        );
        // The building's author-owned construction/maintenance budget stays on
        // its host. Independent rules do not duplicate that building budget.
        for hash in [0x0A8A_41DA, 0x09AE_19D7] {
            set_scalar(&mut unit, hash, PropType::Float, Value::Float(0.0));
        }
        child_ctx
            .resources
            .insert(id(menu::UNIT_GROUP, child), encode(&unit)?);
        child_ctx
            .resources
            .insert(id(menu::lot_group(), child), child_lot(&host_lot)?);
        // Temporary menu is solely the existing binder's freeze baseline. It
        // is never emitted: the player places exactly one host menu entry.
        let menu_bytes = ctx
            .resources
            .get(&id(menu::MENU_GROUP, host))
            .ok_or("BEHAVIOR_UNIT_RESOURCE: host menu")?
            .clone();
        child_ctx
            .resources
            .insert(id(menu::MENU_GROUP, child), menu_bytes);
        let mut detail = behavior::apply(&mut child_ctx)?;
        let car_arrivals = super::vehicle_paths::has_supported_arrivals(
            child_ctx
                .resources
                .get(&id(menu::UNIT_GROUP, child))
                .unwrap(),
        )?;
        if super::vehicle_paths::authored(ctx.scene_document.as_ref()) && car_arrivals {
            let (unit, lot) = super::vehicle_paths::bind(
                child_ctx
                    .resources
                    .get(&id(menu::UNIT_GROUP, child))
                    .unwrap(),
                child_ctx
                    .resources
                    .get(&id(menu::lot_group(), child))
                    .unwrap(),
            )?;
            child_ctx
                .resources
                .insert(id(menu::UNIT_GROUP, child), unit);
            child_ctx
                .resources
                .insert(id(menu::lot_group(), child), lot);
            detail["ordinaryVehiclePath"] = serde_json::Value::Bool(true);
        } else if !car_arrivals {
            detail["ordinaryVehiclePath"] = serde_json::json!({"status":"not-provided-by-current-profile","pedestrianPathsRetained":true});
        }
        for (resource, bytes) in child_ctx.resources.all() {
            if *resource == id(menu::MENU_GROUP, child) {
                continue;
            }
            if ctx.resources.get(resource) != Some(bytes) {
                if additions
                    .get(resource)
                    .is_some_and(|previous| previous != bytes)
                {
                    return Err("BEHAVIOR_UNIT_RESOURCE: child dependency collision".into());
                }
                if ctx.resources.contains_key(resource) {
                    return Err(format!(
                        "BEHAVIOR_UNIT_RESOURCE: child rewrote existing {resource}"
                    ));
                }
                additions.insert(*resource, bytes.clone());
            }
        }
        attach(&mut host_unit, child)?;
        report.push(serde_json::json!({"scope":scope.id,"instance":format!("{child:08X}"),"binding":detail}));
    }
    for (resource, bytes) in additions {
        ctx.resources.insert(resource, bytes);
    }
    ctx.resources.insert(host_id, encode(&host_unit)?);
    verify(ctx.resources.all(), host, program)?;
    Ok(serde_json::json!({"host":format!("{host:08X}"),"units":report,"runtimeVerified":false}))
}

pub(super) fn verify(
    resources: &Resources,
    host: u32,
    program: &ResolvedBehavior,
) -> Result<(), String> {
    if program.units.is_empty() {
        return Ok(());
    }
    let unit = parse(resources, id(menu::UNIT_GROUP, host))?;
    super::behavior_container::verify(&unit)?;
    let host_config = parse(resources, id(menu::lot_group(), host))?;
    if host_config.get(0x0AFE_2FEB).and_then(|p| p.scalar()) == Some(&Value::Bool(true)) {
        return Err("BEHAVIOR_UNIT_TABLE: container must be a native base unit".into());
    }
    let children = values(&unit, CHILDREN)?;
    let connected = values(&unit, CONNECT)?;
    let offsets = values(&unit, OFFSETS)?;
    for (hash, ty) in [
        (CHILDREN, PropType::Key),
        (OFFSETS, PropType::Vector3),
        (0x0863_5E14, PropType::Float),
        (0x0863_5E15, PropType::Vector3),
        (0x0863_5E13, PropType::Float),
        (CONNECT, PropType::Bool),
    ] {
        if unit.get(hash).is_none_or(|p| p.prop_type != ty)
            || values(&unit, hash)?.len() != children.len()
        {
            return Err(format!(
                "BEHAVIOR_UNIT_TABLE: {hash:08X} native column type or row count"
            ));
        }
    }
    if connected.len() != children.len() || offsets.len() != children.len() {
        return Err("BEHAVIOR_UNIT_TABLE: final row count".into());
    }
    for scope in &program.units {
        let child = instance(host, &scope.id);
        let positions = children
            .iter()
            .enumerate()
            .filter_map(|(i, v)| matches!(v,Value::Key(k) if k.instance==child).then_some(i))
            .collect::<Vec<_>>();
        if positions.len() != 1 || connected[positions[0]] != Value::Bool(true) {
            return Err("BEHAVIOR_UNIT_TABLE: child not connected".into());
        }
        let unit = parse(resources, id(menu::UNIT_GROUP, child))?;
        super::behavior_container::verify_toggle(&unit)?;
        if unit.get(CONFIG).map(|p| &p.kind) != Some(&Kind::Scalar(key(menu::lot_group(), child))) {
            return Err("BEHAVIOR_UNIT_RESOURCE: wrong child config".into());
        }
        let lot = parse(resources, id(menu::lot_group(), child))?;
        if lot.get(super::behavior_container_ui::BOUNDS) != host_config.get(super::behavior_container_ui::BOUNDS) {
            return Err("BEHAVIOR_UNIT_TABLE: component render bounds differ from building".into());
        }
        if lot.get(0x0AFE_2FEB).and_then(|p| p.scalar()) != Some(&Value::Bool(true)) {
            return Err("BEHAVIOR_UNIT_TABLE: child is not a native attached module".into());
        }
        if lot.get(0x0AD9_0138).and_then(|p| p.scalar()) != Some(&Value::Bool(false))
            || lot.get(0x0CD4_E49C).is_some()
        {
            return Err("BEHAVIOR_UNIT_RESOURCE: child has a player control surface".into());
        }
        verify_paths(&lot)?;
        if resources.contains_key(&id(menu::MENU_GROUP, child)) {
            return Err("BEHAVIOR_UNIT_RESOURCE: unexpected child menu".into());
        }
        if let Some(office) = &scope.program.office {
            office.verify(&unit)?;
        }
        if scope.id == super::behavior_units::BUILDING_STAFF {
            for hash in [0x0A79_17C3, 0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7, 0x0A79_17C8] {
                let expected = scope.program.set_float_fields.get(&hash)
                    .map(|value| Value::Float(*value as f32));
                if expected.is_none() || unit.get(hash).and_then(|property| property.scalar()) != expected.as_ref() {
                    return Err(format!("BEHAVIOR_UNIT_STAFF: invalid job field {hash:08X}"));
                }
            }
        }
        if let Some(commercial) = &scope.program.commercial {
            commercial.verify(&unit)?;
            commercial.verify_mixed_capacity(&unit, id(menu::UNIT_GROUP, child), resources)?;
        }
        if !scope.program.templates.is_empty() {
            scope.program.templates.verify(resources, child)?;
        }
        if let Some(hotel) = &scope.program.hotel {
            hotel.verify(&unit)?;
        }
        if scope.program.native_tourism.is_some() {
            let bytes = resources.get(&id(menu::UNIT_GROUP, child)).unwrap();
            if super::behavior_tourism_lifetime::patch(bytes)? != *bytes {
                return Err("BEHAVIOR_UNIT_RESOURCE: missing visitor lifetime".into());
            }
        }
    }
    Ok(())
}

pub(super) fn owned_resources(
    resources: &Resources,
    host: u32,
    program: &ResolvedBehavior,
) -> Result<std::collections::HashSet<ResourceId>, String> {
    verify(resources, host, program)?;
    let mut owned = std::collections::HashSet::new();
    for scope in &program.units {
        let child = instance(host, &scope.id);
        owned.insert(id(menu::UNIT_GROUP, child));
        owned.insert(id(menu::lot_group(), child));
        if scope.program.author_tourism {
            owned.insert(super::author_tourism::construction_id(resources, child)?);
        }
        if scope.program.commercial.is_some() {
            let capacity = id(0x40E0_C700, child);
            if resources.contains_key(&capacity) {
                owned.insert(capacity);
            }
        }
    }
    Ok(owned)
}

pub(super) fn verify_vehicle_bindings(
    resources: &Resources,
    host: u32,
    program: &ResolvedBehavior,
) -> Result<(), String> {
    for scope in &program.units {
        let child = instance(host, &scope.id);
        let unit = resources.get(&id(menu::UNIT_GROUP, child))
            .ok_or("BEHAVIOR_UNIT_RESOURCE: missing vehicle Unit")?;
        if super::vehicle_paths::has_supported_arrivals(unit)? {
            let lot = resources.get(&id(menu::lot_group(), child))
                .ok_or("BEHAVIOR_UNIT_RESOURCE: missing vehicle Asset")?;
            super::vehicle_paths::verify(unit, lot)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_preserves_existing_rows_and_connects_only_new_child() {
        let mut unit = PropertyFile::default();
        set_array(
            &mut unit,
            CHILDREN,
            PropType::Key,
            vec![key(menu::UNIT_GROUP, 123)],
        );
        set_array(
            &mut unit,
            OFFSETS,
            PropType::Vector3,
            vec![Value::Vector3([1.0, 2.0, 3.0])],
        );
        attach(&mut unit, 456).unwrap();
        assert_eq!(
            values(&unit, CONNECT).unwrap(),
            vec![Value::Bool(false), Value::Bool(true)]
        );
        assert_eq!(
            values(&unit, OFFSETS).unwrap()[0],
            Value::Vector3([1.0, 2.0, 3.0])
        );
        assert!(attach(&mut unit, 456).is_err());
        assert_eq!(unit.get(0x0863_5E15).unwrap().prop_type, PropType::Vector3);
        assert_eq!(
            values(&unit, 0x0863_5E15).unwrap(),
            vec![Value::Vector3([0.0; 3]); 2]
        );
        let decoded = PropertyFile::parse(&encode(&unit).unwrap()).unwrap();
        assert_eq!(
            decoded.get(CONNECT).unwrap().encoding.array_item_size,
            Some(1)
        );
        assert_ne!(instance(42, "office"), instance(42, "tourism"));
        assert_ne!(instance(42, "office"), instance(43, "office"));
    }

    #[test]
    fn office_and_tourism_emit_distinct_units_without_cross_writing_or_extra_menu() {
        use serde_json::json;
        let program=super::super::behavior_nodes::compile(None,&[
            json!({"id":"office","type":"office","unit":"work","params":{}}),
            json!({"id":"visitors","type":"tourism","unit":"visits","params":{"tourismProfile":"native-tourism-v1"}}),
        ]).unwrap();
        let raw=json!({"$schema":"openscp.asset","schema_version":2,"id":"test-unit-host",
            "kind":"building-asset","reference":{"package":"source.package","lot":"00B1B104:40E1C000:12345678"},
            "scene":{"document":"asset.lot.json"},
            "outputs":{"package":"test.package","mode":"full"}}).to_string();
        let mut plan = super::super::plan::plan_from_schema(&raw).unwrap();
        plan.behavior_program = Some(program.clone());
        let host = 0x1234_5678;
        let mut ctx = BuildContext::new(".".into(), plan);
        ctx.menu_instance = Some(host);
        let empty = encode(&PropertyFile::default()).unwrap();
        let seed = super::super::behavior_container::patch(&empty).unwrap();
        ctx.resources.insert(id(menu::UNIT_GROUP, host), seed);
        ctx.resources
            .insert(id(menu::MENU_GROUP, host), empty.clone());
        let mut lot = PropertyFile::default();
        for (hash, ty, v) in [
            (PATHS[0], PropType::Key, vec![key(0x40E0_C400, 0x418A_18B1)]),
            (
                PATHS[1],
                PropType::Int32,
                vec![Value::Int32(0), Value::Int32(1)],
            ),
            (
                PATHS[2],
                PropType::Int32,
                vec![Value::Int32(0), Value::Int32(1)],
            ),
            (PATHS[3], PropType::Key, vec![key(0, 0x5757_2DAC)]),
            (PATHS[4], PropType::Key, vec![key(0, 0xF0F6_1CED)]),
            (
                PATHS[5],
                PropType::Vector3,
                vec![Value::Vector3([0.0; 3]), Value::Vector3([10.0, 0.0, 0.0])],
            ),
            (
                PATHS[6],
                PropType::Vector3,
                vec![Value::Vector3([0.0; 3]); 2],
            ),
        ] {
            set_array(&mut lot, hash, ty, v);
        }
        ctx.resources
            .insert(id(menu::lot_group(), host), encode(&lot).unwrap());
        behavior::apply(&mut ctx).unwrap();
        verify(ctx.resources.all(), host, &program).unwrap();
        let child = instance(host, "visits");
        let office = parse(
            ctx.resources.all(),
            id(menu::UNIT_GROUP, instance(host, "work")),
        )
        .unwrap();
        let container = parse(ctx.resources.all(), id(menu::UNIT_GROUP, host)).unwrap();
        super::super::behavior_container::verify(&container).unwrap();
        let tourist = parse(ctx.resources.all(), id(menu::UNIT_GROUP, child)).unwrap();
        assert_eq!(
            office.get(0x0C0F_228C).unwrap().kind,
            Kind::Scalar(Value::Float(9.0))
        );
        assert_eq!(
            tourist.get(0x0C0F_228C).unwrap().kind,
            Kind::Scalar(Value::Float(6.0))
        );
        assert!(ctx.resources.get(&id(menu::MENU_GROUP, child)).is_none());
        super::super::behavior_container_ui::verify(ctx.resources.all(), host, &program).unwrap();
        let child_config = parse(ctx.resources.all(), id(menu::lot_group(), child)).unwrap();
        for hash in [0x00F9_EFBB, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE] {
            assert!(child_config.get(hash).is_none());
        }
        let host_config = parse(ctx.resources.all(), id(menu::lot_group(), host)).unwrap();
        assert_eq!(child_config.get(PATHS[5]), host_config.get(PATHS[5]));
        let mut exposed = child_config;
        set_scalar(&mut exposed, 0x0AD9_0138, PropType::Bool, Value::Bool(true));
        let mut exposed_resources = ctx.resources.all().clone();
        exposed_resources.insert(id(menu::lot_group(), child), encode(&exposed).unwrap());
        assert!(verify(&exposed_resources, host, &program).unwrap_err().contains("player control"));
        let mut broken = ctx.resources.all().clone();
        let mut disconnected = container;
        set_array(
            &mut disconnected,
            CONNECT,
            PropType::Bool,
            vec![Value::Bool(false)],
        );
        disconnected
            .values
            .iter_mut()
            .find(|p| p.hash == CONNECT)
            .unwrap()
            .encoding
            .array_item_size = Some(1);
        broken.insert(id(menu::UNIT_GROUP, host), encode(&disconnected).unwrap());
        assert!(verify(&broken, host, &program).is_err());
    }
}
