//! Compose native behavior records using the leaf-first selection from SC5
//! Asset Packager behavior_profiles.py:62–95,106–123. The recipe data comes
//! from Copan's effective Unit, not from its landmark identity or menu.

use super::behavior::PROPERTY_TYPE;
use super::connector::Resources;
use super::menu::{UNIT_GROUP, lot_group};
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropType, PropertyFile, Value as PropValue};
use serde::Serialize;
use serde_json::Value;

pub(super) const RESIDENCE_PROFILE: &str = "copan-residence-v1";
pub(super) const LAND_PROFILE: &str = "copan-land-value-v1";
pub(super) const ENABLE: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
pub(super) const EXCLUDED: u32 = 0x0A02_09B2;
const PATH_ENTRIES: u32 = 0x0CC8_FE7A;
const PATH_GROUP: u32 = 0x40E0_C400;
const KEY_ARRAY_FLAGS: u16 = 0x809C;
const STOCK_POPULATION: f32 = 1100.0;
const LAND: &[u32] = &[0x8BA6_D35D];
const LANDMARK_SINK_BUNDLE: u32 = 0x5CB4_02D9;
const RESIDENCE_INTAKE_BIN_BUNDLE: u32 = 0x9DEF_44FB;
const INITIAL_CONTROL_FLAGS: u32 = 0x0A88_D179;
const VACANT_FLAG: PropValue = PropValue::Int32(22);
// Subsequence of Copan 4DC31F9B's A4. Includes occupancy, agent sinks,
// access/construction, happiness, and resident health/return transactions.
const RESIDENCE: &[u32] = &[
    0x5C74_D18B,
    0x933F_0FBF,
    0x44F5_CF6C,
    0xE447_343A,
    0x323A_4CF2,
    0x6142_47C7,
    0xAB9A_0A3E,
    0x7378_2B22,
    0x568C_CCA3,
    0xCC2A_40B6,
    0xE648_41C9,
    0x7C37_AF27,
    0x4E7D_5743,
    0xBAC9_6038,
    0x3B72_84AB,
    0xEE66_978E,
    0x999F_8484,
    0x5D5E_F1FF,
    0xA3F5_54BD,
    0xE238_1694,
];

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(super) struct Templates {
    #[serde(skip_serializing_if = "Option::is_none")]
    population: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    land: Option<[f32; 2]>,
}

impl Templates {
    pub(super) fn is_empty(&self) -> bool {
        !self.residence() && !self.land_value()
    }

    pub(super) fn residence(&self) -> bool {
        self.population.is_some()
    }

    pub(super) fn land_value(&self) -> bool {
        self.land.is_some()
    }

    /// Copan 4DC31F9B initializes Vacant, supplies the intake key bin for
    /// BBAAD3D1, and excludes the landmark binding of residential sink 790C2A95.
    pub(super) fn prepare_landmark_residence(
        &self,
        unit: &mut PropertyFile,
        active: &mut Vec<PropValue>,
        excluded: &mut Vec<PropValue>,
        unban: &[u32],
    ) -> Result<(), String> {
        if !self.residence() {
            return Ok(());
        }
        let mut flags = array(unit, INITIAL_CONTROL_FLAGS)?.to_vec();
        if !flags.contains(&VACANT_FLAG) {
            flags.push(VACANT_FLAG);
            set_array(unit, INITIAL_CONTROL_FLAGS, PropType::Int32, flags);
        }
        excluded.retain(|value| {
            instance(value).is_none_or(|id| id != RESIDENCE_INTAKE_BIN_BUNDLE && !unban.contains(&id))
        });
        if !active.iter().any(|value| instance(value) == Some(RESIDENCE_INTAKE_BIN_BUNDLE)) {
            active.push(key(RESIDENCE_INTAKE_BIN_BUNDLE));
        }
        if !excluded.iter().any(|value| instance(value) == Some(LANDMARK_SINK_BUNDLE)) {
            excluded.push(key(LANDMARK_SINK_BUNDLE));
        }
        Ok(())
    }

    pub(super) fn verify_landmark_residence(&self, unit: &PropertyFile) -> Result<(), String> {
        if !self.residence() {
            return Ok(());
        }
        if !array(unit, INITIAL_CONTROL_FLAGS)?.contains(&VACANT_FLAG) {
            return Err("Copan residence requires initial Vacant flag 22 in 0A88D179".into());
        }
        if !contains(unit, &[EXCLUDED], LANDMARK_SINK_BUNDLE) {
            return Err("Copan residence requires excluding landmark sink bundle 5CB402D9".into());
        }
        if contains(unit, &[EXCLUDED], RESIDENCE_INTAKE_BIN_BUNDLE)
            || !contains(unit, &[ENABLE[3]], RESIDENCE_INTAKE_BIN_BUNDLE)
        {
            return Err("Copan residence intake requires A9ECEDE4 bundle 9DEF44FB".into());
        }
        Ok(())
    }

    pub(super) fn bundles(&self) -> impl Iterator<Item = u32> {
        self.population
            .iter()
            .flat_map(|_| RESIDENCE.iter().copied())
            .chain(self.land.iter().flat_map(|_| LAND.iter().copied()))
    }

    fn fields(&self) -> Vec<(u32, PropType, PropValue)> {
        let mut fields = Vec::new();
        if let Some(population) = self.population {
            let population = population as f32;
            let scale = population / STOCK_POPULATION;
            fields.extend(
                [
                    (0x0A79_17CB, population),
                    (0x0A79_17C2, population),
                    (0x0A79_17CC, (100.0 * scale).round()),
                    (0x0E33_502E, (800.0 * scale).round()),
                    (0x0E33_5022, (300.0 * scale).round()),
                    (0x0A79_17C4, 0.0),
                    (0x0C20_0553, 3.0),
                    (0x0D08_4715, 1.0),
                    (0x0DF0_635A, 6.0),
                    (0x0E54_6790, 600.0),
                    (0x0E54_6791, 6600.0),
                    (0x0E54_6792, 66000.0),
                    (0x0E83_DE1C, 4.0),
                    (0x0E83_DE27, 4.0),
                    (0x0D7B_1448, 0.07),
                    (0x0D7B_1449, 0.18),
                    (0x0D7B_144A, 0.30),
                    (0x0D7B_144B, 0.75),
                    (0x0E0C_1481, 13000.0),
                    (0x0C12_C537, 0.2),
                    (0x0D63_43C2, 0.09),
                    (0x0D63_5C98, 243.0),
                    (0x0D89_7622, 60.0),
                ]
                .map(|(hash, value)| (hash, PropType::Float, PropValue::Float(value))),
            );
            fields.extend(
                [(0x0E30_E65A, 0x192F_CF56), (0x0EAC_DB63, 0xB892_8907)]
                    .map(|(hash, instance)| (hash, PropType::Key, key(instance))),
            );
        }
        if let Some([medium, high]) = self.land {
            fields.extend([
                (0x0E58_5AE0, PropType::Float, PropValue::Float(medium)),
                (0x0E58_5AE1, PropType::Int32, PropValue::Int32(2)),
                (0x0E58_5B00, PropType::Float, PropValue::Float(high)),
                (0x0E58_5B01, PropType::Int32, PropValue::Int32(3)),
            ]);
        }
        fields
    }

    pub(super) fn reconcile(&self, program: &mut super::behavior_nodes::ResolvedBehavior) {
        for (hash, _, _) in self.fields() {
            program.remove_fields.retain(|field| *field != hash);
            program.set_float_fields.remove(&hash);
            program.set_int_fields.remove(&hash);
            if !program.declared_unit_columns.contains(&hash) {
                program.declared_unit_columns.push(hash);
            }
        }
        for bundle in self.bundles() {
            program
                .remove_bundles
                .retain(|candidate| *candidate != bundle);
        }
        if !self.is_empty() {
            program
                .notes
                .push(format!("behavior templates: {self:?} (Copan 4DC31F9B)"));
        }
    }

    pub(super) fn patch(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut unit = read(bytes)?;
        let bundles: Vec<_> = self.bundles().collect();
        let mut leaf_bundles = array(&unit, ENABLE[3])?.to_vec();
        let mut excluded = array(&unit, EXCLUDED)?.to_vec();
        let mut changed = false;
        for bundle in bundles.iter().copied() {
            if !contains(&unit, &ENABLE, bundle) {
                leaf_bundles.push(key(bundle));
                changed = true;
            }
        }
        if changed {
            set_array_with_flags(
                &mut unit,
                ENABLE[3],
                PropType::Key,
                leaf_bundles,
                KEY_ARRAY_FLAGS,
            );
        }
        let before = excluded.len();
        excluded.retain(|value| {
            !bundles
                .iter()
                .any(|bundle| instance(value) == Some(*bundle))
        });
        if excluded.len() != before {
            set_array_with_flags(
                &mut unit,
                EXCLUDED,
                PropType::Key,
                excluded,
                KEY_ARRAY_FLAGS,
            );
        }
        for (hash, kind, value) in self.fields() {
            set_scalar(&mut unit, hash, kind, value);
            if let Some(property) = unit
                .values
                .iter_mut()
                .find(|property| property.hash == hash)
            {
                property.encoding.flags = 0x8000;
                property.encoding.array_item_size = None;
            }
        }
        encode(&unit)
    }

    pub(super) fn verify(&self, resources: &Resources, instance: u32) -> Result<(), String> {
        let unit = resource(resources, UNIT_GROUP, instance)?;
        for row in ENABLE.into_iter().chain([EXCLUDED]) {
            array(&unit, row)?;
        }
        let catalog = super::behavior_conflicts::catalog()?;
        for bundle in self.bundles() {
            if !catalog.has_bundle(bundle)
                || !contains(&unit, &ENABLE, bundle)
                || contains(&unit, &[EXCLUDED], bundle)
            {
                return Err(format!(
                    "BEHAVIOR_TEMPLATE_BUNDLE: {instance:08X}/{bundle:08X}"
                ));
            }
        }
        for (hash, kind, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.prop_type == kind && prop.kind == Kind::Scalar(value))
            {
                return Err(format!(
                    "BEHAVIOR_TEMPLATE_FIELD: {instance:08X}/{hash:08X}"
                ));
            }
        }
        if self.residence() {
            let lot = resource(resources, lot_group(), instance)?;
            if !array(&lot, PATH_ENTRIES)?.iter().any(|entry| {
                matches!(entry, PropValue::Key(key) if key.group == PATH_GROUP && key.instance != 0)
            }) {
                return Err(format!("RESIDENCE_PATH: {instance:08X}"));
            }
        }
        Ok(())
    }
}

pub(super) fn select(nodes: &[Value]) -> Result<Templates, String> {
    let mut templates = Templates::default();
    for node in nodes {
        let params = &node["params"];
        let id = node["id"].as_str().unwrap_or("behavior");
        match node["type"].as_str() {
            Some("residence") if !params["residenceProfile"].is_null() => {
                if templates.residence() || params["residenceProfile"] != RESIDENCE_PROFILE {
                    return Err(format!("BEHAVIOR_TEMPLATE_PROFILE: {id}/residence"));
                }
                let value = amount(params, "population", id, f64::from(STOCK_POPULATION), 1.0)?;
                if value.fract() != 0.0 {
                    return Err(format!("BEHAVIOR_TEMPLATE_PARAMETER: {id}/population"));
                }
                templates.population = Some(value as u32);
            }
            Some("landvalue") if !params["landValueProfile"].is_null() => {
                if templates.land_value() || params["landValueProfile"] != LAND_PROFILE {
                    return Err(format!("BEHAVIOR_TEMPLATE_PROFILE: {id}/landvalue"));
                }
                templates.land = Some([
                    amount(params, "value", id, 500.0, -100_000.0)? as f32,
                    amount(params, "value2", id, 500.0, -100_000.0)? as f32,
                ]);
            }
            _ => {}
        }
    }
    for (kind, selected) in [
        ("residence", templates.residence()),
        ("landvalue", templates.land_value()),
    ] {
        if selected && nodes.iter().filter(|node| node["type"] == kind).count() != 1 {
            return Err(format!("BEHAVIOR_TEMPLATE_PROFILE: duplicate {kind}"));
        }
    }
    Ok(templates)
}

fn amount(params: &Value, field: &str, id: &str, default: f64, min: f64) -> Result<f64, String> {
    if params[field].is_null() {
        return Ok(default);
    }
    params[field]
        .as_f64()
        .filter(|value| (min..=100_000.0).contains(value))
        .ok_or_else(|| format!("BEHAVIOR_TEMPLATE_PARAMETER: {id}/{field}"))
}

pub(super) fn key(instance: u32) -> PropValue {
    PropValue::Key(Key {
        type_id: 0,
        group: 0,
        instance,
    })
}

fn instance(value: &PropValue) -> Option<u32> {
    match value {
        PropValue::Key(key) => Some(key.instance),
        _ => None,
    }
}

pub(super) fn array(file: &PropertyFile, hash: u32) -> Result<&[PropValue], String> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Array(values)) => Ok(values),
        None => Ok(&[]),
        _ => Err(format!(
            "BEHAVIOR_TEMPLATE_FIELD: {hash:08X} is not an array"
        )),
    }
}

fn contains(file: &PropertyFile, hashes: &[u32], bundle: u32) -> bool {
    hashes.iter().any(|hash| {
        array(file, *hash)
            .is_ok_and(|values| values.iter().any(|value| instance(value) == Some(bundle)))
    })
}

fn read(bytes: &[u8]) -> Result<PropertyFile, String> {
    PropertyFile::parse(bytes).map_err(|error| format!("BEHAVIOR_TEMPLATE_RESOURCE: {error}"))
}

fn resource(resources: &Resources, group: u32, instance: u32) -> Result<PropertyFile, String> {
    let id = ResourceId {
        type_id: PROPERTY_TYPE,
        group,
        instance,
    };
    read(
        resources
            .get(&id)
            .ok_or_else(|| format!("BEHAVIOR_TEMPLATE_RESOURCE: {id}"))?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::{behavior, menu, plan::plan_from_schema, stages::BuildContext};
    use serde_json::json;

    #[test]
    fn composes_residence_and_land_on_the_generated_unit() {
        let schema = json!({
            "$schema":"openscp.asset", "schema_version":2, "id":"copan-mixed",
            "kind":"building-asset",
            "reference":{"package":"source.package", "lot":"00B1B104:40E1C000:4DC31F9B"},
            "scene":{"document":"asset.lot.json"},
            "semantics":{"behaviors":[
                {"id":"park","type":"park","params":{"landValue":500}},
                {"id":"residence","type":"residence","params":{"residenceProfile":RESIDENCE_PROFILE,"population":550}},
                {"id":"land","type":"landvalue","params":{"landValueProfile":LAND_PROFILE,"value":270}}
            ]},
            "outputs":{"package":"copan-mixed.package","mode":"full"}
        }).to_string();
        let mut ctx = BuildContext::new(".".into(), plan_from_schema(&schema).unwrap());
        let instance = 0x1234_5678;
        ctx.menu_instance = Some(instance);
        let id = |group| ResourceId {
            type_id: PROPERTY_TYPE,
            group,
            instance,
        };
        let unit_id = id(UNIT_GROUP);
        let lot_id = id(lot_group());
        let menu_id = id(menu::MENU_GROUP);
        let mut unit = PropertyFile::default();
        set_array_with_flags(
            &mut unit,
            EXCLUDED,
            PropType::Key,
            vec![key(0xAB9A_0A3E), key(0x3064_5625)],
            KEY_ARRAY_FLAGS,
        );
        let empty = encode(&PropertyFile::default()).unwrap();
        ctx.resources.insert(unit_id, encode(&unit).unwrap());
        ctx.resources.insert(lot_id, empty.clone());
        ctx.resources.insert(menu_id, empty.clone());
        behavior::apply(&mut ctx).unwrap();

        let program = ctx.plan.behavior_program.as_ref().unwrap();
        assert!(
            program
                .templates
                .verify(ctx.resources.all(), instance)
                .unwrap_err()
                .starts_with("RESIDENCE_PATH")
        );
        let mut lot = PropertyFile::default();
        set_array_with_flags(
            &mut lot,
            PATH_ENTRIES,
            PropType::Key,
            vec![PropValue::Key(Key {
                type_id: PROPERTY_TYPE,
                group: PATH_GROUP,
                instance: 0x418A_18B1,
            })],
            KEY_ARRAY_FLAGS,
        );
        ctx.resources.insert(lot_id, encode(&lot).unwrap());
        program
            .templates
            .verify(ctx.resources.all(), instance)
            .unwrap();

        let unit = read(ctx.resources.get(&unit_id).unwrap()).unwrap();
        assert!(contains(&unit, &ENABLE, 0xAB9A_0A3E));
        assert!(!contains(&unit, &ENABLE, 0x8A60_B969));
        assert!(contains(&unit, &[EXCLUDED], 0x3064_5625));
        assert_eq!(
            unit.get(0x0A79_17CB).unwrap().scalar(),
            Some(&PropValue::Float(550.0))
        );
        assert_eq!(
            unit.get(0x0E33_502E).unwrap().scalar(),
            Some(&PropValue::Float(400.0))
        );
        assert_eq!(
            unit.get(0x0E58_5AE0).unwrap().scalar(),
            Some(&PropValue::Float(270.0))
        );
        assert_eq!(
            unit.get(0x0E58_5AE1).unwrap().scalar(),
            Some(&PropValue::Int32(2))
        );
        assert_eq!(
            unit.get(0x0E58_5B00).unwrap().scalar(),
            Some(&PropValue::Float(500.0))
        );
        assert_eq!(ctx.resources.get(&menu_id), Some(&empty));
    }
}
