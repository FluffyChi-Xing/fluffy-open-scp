//! Visitor recipe for the workbench's native tourism profile. The template
//! inputs are the flattened Tokyo Tower Unit (084DB48F), using the leaf-first
//! record selection in SC5 Asset Packager's behavior_profiles.py:62–95,106–123.
//! W2 lodging is an opt-in composition with hotel template 971A4B13. See
//! docs/re/hope-tower-labor-engine-alignment-20261111.md §§13,15.

use super::{behavior_nodes::ResolvedBehavior, stages::BuildContext};
use dbpf::ResourceId;
use sc_properties::{Kind, PropertyFile, Value as PropertyValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const PROFILE: &str = "native-tourism-v1";
const TOURIST_BUNDLES: [u32; 4] = [0xE153_69A2, 0x75D2_2FD2, 0x75D2_2FD1, 0x75D2_2FD0];
const HOTEL_W2: u32 = 0x729A_6626;
const ENABLE_ROWS: [u32; 4] = [0x0A02_09A2, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5];
const EXCLUDE_ROW: u32 = 0x0A02_09B2;
const PATH_ENTRIES: u32 = 0x0CC8_FE7A;
const PATH_ROAD_CONNECT: u32 = 0x0D9A_A2BB;
const MAX_VALUE: f64 = 100_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct NativeTourism {
    node: String,
    attraction: f64,
    quotas: [f64; 3],
    shoppers: f64,
    incomes: [f64; 3],
    lodging: bool,
}

pub(super) fn select(nodes: &[Value]) -> Result<Option<NativeTourism>, String> {
    let mut tourism = nodes.iter().filter(|node| node["type"] == "tourism");
    let Some(first) = tourism.find(|node| !node["params"]["tourismProfile"].is_null()) else {
        return Ok(None);
    };
    let node = first["id"].as_str().unwrap_or("tourism");
    let params = &first["params"];
    if params["tourismProfile"] != PROFILE
        || nodes
            .iter()
            .filter(|node| node["type"] == "tourism")
            .count()
            != 1
    {
        return Err(format!("TOURISM_PROFILE: {node}"));
    }
    let number = |key: &str, default: f64, min: f64| {
        if params[key].is_null() {
            return Ok(default);
        }
        params[key]
            .as_f64()
            .filter(|value| (min..=MAX_VALUE).contains(value))
            .ok_or_else(|| format!("TOURISM_PARAMETER: {node}/{key}"))
    };
    let quotas = [
        number("quotaW1", 100.0, 0.0)?,
        number("quotaW2", 50.0, 0.0)?,
        number("quotaW3", 25.0, 0.0)?,
    ];
    if quotas.iter().any(|quota| quota.fract() != 0.0) || quotas.iter().sum::<f64>() == 0.0 {
        return Err(format!("TOURISM_QUOTA: {node}"));
    }
    let lodging = match &params["lodging"] {
        Value::Bool(value) => *value,
        Value::Null => false,
        _ => return Err(format!("TOURISM_PARAMETER: {node}/lodging")),
    };
    let shoppers = number("shoppers", 175.0, 1.0)?;
    if shoppers.fract() != 0.0 {
        return Err(format!("TOURISM_PARAMETER: {node}/shoppers"));
    }
    Ok(Some(NativeTourism {
        node: node.to_owned(),
        attraction: number("attraction", 400.0, 0.0)?,
        quotas,
        shoppers,
        incomes: [
            number("incomeW1", 10.0, 0.0)?,
            number("incomeW2", 10.0, 0.0)?,
            number("incomeW3", 10.0, 0.0)?,
        ],
        lodging,
    }))
}

impl NativeTourism {
    fn bundles(&self) -> impl Iterator<Item = u32> {
        TOURIST_BUNDLES
            .into_iter()
            .chain(self.lodging.then_some(HOTEL_W2))
    }

    pub(super) fn apply(&self, program: &mut ResolvedBehavior) -> Result<(), String> {
        // 971A4B13 and the visitor bundles consume the same native Float.
        // Preserve the hotel's value for both mappings when lodging is enabled.
        let shared_capacity = if self.lodging { 68.0 } else { 2100.0 };
        for (hash, value) in [
            (0x0E54_2F39, self.attraction),
            (0x0D8C_29F6, self.quotas[0]),
            (0x0D8C_29FE, self.quotas[1]),
            (0x0D8C_2A01, self.quotas[2]),
            (0x0AD6_A725, self.shoppers),
            (0x0E26_60C6, shared_capacity),
            (0x0DA3_EE15, 1000.0),
            (0x0EE2_A50D, self.incomes[0]),
            (0x0EE2_A50E, self.incomes[1]),
            (0x0EE2_A50F, self.incomes[2]),
            (0x0C0F_2290, 36.0),
            (0x0C0F_228C, 6.0),
        ] {
            program.record_float(hash, value, &self.node)?;
            program.remove_fields.retain(|field| *field != hash);
        }
        for bundle in self.bundles() {
            if !program.ensure_bundles.contains(&bundle) {
                program.ensure_bundles.push(bundle);
            }
            if !program.remove_excluded_bundles.contains(&bundle) {
                program.remove_excluded_bundles.push(bundle);
            }
            program
                .remove_bundles
                .retain(|candidate| *candidate != bundle);
        }
        if !self.lodging {
            program.ensure_bundles.retain(|bundle| *bundle != HOTEL_W2);
            if !program.remove_bundles.contains(&HOTEL_W2) {
                program.remove_bundles.push(HOTEL_W2);
            }
        }
        program.notes.push(format!(
            "{PROFILE}: Tokyo Tower 084DB48F, lodging={}",
            self.lodging
        ));
        Ok(())
    }

    pub(super) fn verify(
        &self,
        ctx: &BuildContext,
        unit_id: &ResourceId,
        lot_id: &ResourceId,
    ) -> Result<(), String> {
        if ctx.menu_instance.is_none() {
            return Err("TOURISM_MENU".into());
        }
        let read = |id| {
            let bytes = ctx
                .resources
                .get(id)
                .ok_or_else(|| format!("TOURISM_RESOURCE: {id}"))?;
            PropertyFile::parse(bytes).map_err(|error| format!("TOURISM_RESOURCE: {id}: {error}"))
        };
        let unit = read(unit_id)?;
        for bundle in self.bundles() {
            if !ENABLE_ROWS.iter().any(|row| has_key(&unit, *row, bundle))
                || has_key(&unit, EXCLUDE_ROW, bundle)
            {
                return Err(format!("TOURISM_BUNDLE: {unit_id}/{bundle:08X}"));
            }
        }
        let lot = read(lot_id)?;
        let entries = array(&lot, PATH_ENTRIES);
        let connects = array(&lot, PATH_ROAD_CONNECT);
        if entries.len() != connects.len()
            || !entries.iter().zip(connects).any(|(entry, connect)| {
                matches!(entry, PropertyValue::Key(key) if key.group == 0x40E0_C400)
                    && matches!(connect, PropertyValue::Key(key)
                        if key.instance == super::native_profile::PATH_ROAD_CONNECT_FIRST)
            })
        {
            return Err(format!("TOURISM_ROAD_ACCESS: {lot_id}"));
        }
        Ok(())
    }
}

fn array(file: &PropertyFile, hash: u32) -> &[PropertyValue] {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Array(values)) => values,
        _ => &[],
    }
}

fn has_key(file: &PropertyFile, hash: u32, instance: u32) -> bool {
    array(file, hash)
        .iter()
        .any(|value| matches!(value, PropertyValue::Key(key) if key.instance == instance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::{
        behavior,
        behavior_nodes::compile,
        menu, native_profile,
        plan::plan_from_schema,
        property_edit::{encode, set_array},
    };
    use sc_properties::{Key, PropType};
    use serde_json::json;

    #[test]
    fn defaults_and_lodging_bind_native_visitor_bundles_without_changing_park_identity() {
        for lodging in [false, true] {
            let program = compile(
                None,
                &[
                    json!({"id":"park", "type":"park", "params":{"attraction":500}}),
                    json!({"id":"visits", "type":"tourism", "params":{
                        "tourismProfile":PROFILE, "lodging":lodging
                    }}),
                ],
            )
            .unwrap();
            assert!(program.parent_template.is_none());
            assert_eq!(program.set_float_fields[&0x0E54_2F39], 400.0);
            assert_eq!(program.set_float_fields[&0x0D8C_29FE], 50.0);
            assert_eq!(program.ensure_bundles.contains(&HOTEL_W2), lodging);
            assert_eq!(
                program.set_float_fields[&0x0E26_60C6],
                if lodging { 68.0 } else { 2100.0 }
            );
            for bundle in TOURIST_BUNDLES {
                assert!(program.ensure_bundles.contains(&bundle));
                assert!(program.remove_excluded_bundles.contains(&bundle));
            }
        }
    }

    #[test]
    fn rejects_zero_total_quota_and_honors_explicit_overrides() {
        let tourism = |params| json!({"id":"visits", "type":"tourism", "params":params});
        assert!(compile(
            None,
            &[tourism(json!({
                "tourismProfile":PROFILE, "quotaW1":0, "quotaW2":0, "quotaW3":0
            }))]
        )
        .unwrap_err()
        .contains("TOURISM_QUOTA"));
        let program = compile(
            None,
            &[tourism(json!({
                "tourismProfile":PROFILE, "attraction":275, "quotaW2":19, "shoppers":92
            }))],
        )
        .unwrap();
        assert_eq!(program.set_float_fields[&0x0E54_2F39], 275.0);
        assert_eq!(program.set_float_fields[&0x0D8C_29FE], 19.0);
        assert_eq!(program.set_float_fields[&0x0AD6_A725], 92.0);
    }

    #[test]
    fn behavior_bind_writes_lodging_to_the_new_unit_and_requires_road_access() {
        let raw = json!({
            "$schema":"openscp.asset", "schema_version":2, "id":"native-tourism",
            "kind":"building-asset",
            "reference":{"package":"source.package", "lot":"00B1B104:40E1C000:8D007745"},
            "scene":{"document":"asset.lot.json"},
            "semantics":{"behaviors":[{"id":"visits","type":"tourism","params":{
                "tourismProfile":PROFILE,"lodging":true,"quotaW2":23
            }}]},
            "outputs":{"package":"native-tourism.package","mode":"full"}
        })
        .to_string();
        let mut ctx = BuildContext::new(".".into(), plan_from_schema(&raw).unwrap());
        let instance = 0x1234_5678;
        ctx.menu_instance = Some(instance);
        let id = |group| ResourceId {
            type_id: menu::PROPERTY_TYPE,
            group,
            instance,
        };
        let unit = id(menu::UNIT_GROUP);
        let lot = id(menu::lot_group());
        let menu = id(menu::MENU_GROUP);
        let key = |group, instance| {
            PropertyValue::Key(Key {
                type_id: 0,
                group,
                instance,
            })
        };
        let empty = encode(&PropertyFile::default()).unwrap();
        ctx.resources.insert(unit, empty.clone());
        ctx.resources.insert(lot, empty.clone());
        ctx.resources.insert(menu, empty.clone());
        assert!(behavior::apply(&mut ctx)
            .unwrap_err()
            .contains("TOURISM_ROAD_ACCESS"));

        let mut entrance = PropertyFile::default();
        set_array(
            &mut entrance,
            PATH_ENTRIES,
            PropType::Key,
            vec![key(0x40E0_C400, 0x0310_6C2E)],
        );
        set_array(
            &mut entrance,
            PATH_ROAD_CONNECT,
            PropType::Key,
            vec![key(0, native_profile::PATH_ROAD_CONNECT_FIRST)],
        );
        entrance.claimed_count = entrance.values.len() as u32;
        ctx.resources.insert(lot, encode(&entrance).unwrap());
        behavior::apply(&mut ctx).unwrap();

        let written = PropertyFile::parse(ctx.resources.get(&unit).unwrap()).unwrap();
        for bundle in TOURIST_BUNDLES.into_iter().chain([HOTEL_W2]) {
            assert!(has_key(&written, ENABLE_ROWS[3], bundle));
            assert!(!has_key(&written, EXCLUDE_ROW, bundle));
        }
        assert!(matches!(
            written.get(0x0D8C_29FE).unwrap().kind,
            Kind::Scalar(PropertyValue::Float(23.0))
        ));
        assert_eq!(ctx.resources.get(&menu).unwrap(), &empty);
    }
}
