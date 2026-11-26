//! Casino extension of the authored tourism Unit. Native reference:
//! 40E0C000:3CE0F852 and the base AEB 40800200:622B9CD7.
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::{set_array, set_array_with_flags, set_scalar};
use sc_properties::{PropType, PropertyFile, Value};
use serde::Serialize;
use serde_json::Value as Json;

pub(super) const VERSION: &str = "openscp.property.m1/tourism-casino-1";
const PROVIDERS: [u32; 4] = [0x3064_5625, 0x352F_4263, 0x3B8E_EF5B, 0xCA72_6CB4];
const REPLACED: [u32; 5] = [0x3A70_FAA2, 0xC140_44C4, 0x6827_D125, 0x6827_D126, 0x6827_D127];
const MODE: u32 = 0x0DB6_53A1;
const START_FLAGS: u32 = 0x0A88_D179;
const CAPACITY: [u32; 3] = [0x0EF2_9855, 0x0EF2_9856, 0x0EF2_9857];
const INCOME: [u32; 3] = [0x0EE2_A50D, 0x0EE2_A50E, 0x0EE2_A50F];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct Casino {
    capacity: [u16; 3],
    income: [u16; 3],
}

pub(super) fn requested(nodes: &[Json]) -> bool {
    nodes.iter().any(|node| node["type"] == "casino")
}

pub(super) fn select(nodes: &[Json], author_tourism: bool) -> Result<Option<Casino>, String> {
    let Some(casino) = nodes.iter().find(|node| node["type"] == "casino") else {
        return Ok(None);
    };
    if !author_tourism
        || nodes.iter().any(|node| !node["unit"].is_null()
            || !matches!(node["type"].as_str(), Some("casino" | "tourism" | "utilities-core")))
        || nodes.iter().filter(|node| node["type"] == "tourism").count() != 1
        || !nodes.iter().any(|node| node["type"] == "tourism"
            && node["params"]["tourismProfile"] == "native-tourism-v1")
    {
        return Err("CASINO_TOURISM_REQUIRED".into());
    }
    let params = &casino["params"];
    let read = |names: [&str; 3], defaults: [u16; 3]| -> Result<[u16; 3], String> {
        let mut values = defaults;
        for (index, name) in names.into_iter().enumerate() {
            if !params[name].is_null() {
                values[index] = params[name].as_u64().filter(|&v| v <= 10000)
                    .ok_or_else(|| format!("CASINO_PARAMETER: {name}"))? as u16;
            }
        }
        Ok(values)
    };
    let capacity = read(["capacityW1", "capacityW2", "capacityW3"], [0, 100, 300])?;
    if capacity == [0; 3] {
        return Err("CASINO_PARAMETER: capacity".into());
    }
    Ok(Some(Casino {
        capacity,
        income: read(["incomeW1", "incomeW2", "incomeW3"], [14, 28, 56])?,
    }))
}

impl Casino {
    pub(super) fn apply(&self, unit: &mut PropertyFile) -> Result<(), String> {
        let owns = |value: &Value, ids: &[u32]|
            matches!(value, Value::Key(k) if ids.contains(&k.instance));
        for row in ENABLE.into_iter().chain([EXCLUDED]) {
            let mut values = array(unit, row)?.to_vec();
            values.retain(|value| !owns(value, &REPLACED) && !owns(value, &PROVIDERS));
            if row == ENABLE[3] {
                values.extend(PROVIDERS.map(key));
            } else if row == EXCLUDED {
                values.extend(REPLACED.map(key));
            }
            set_array_with_flags(unit, row, PropType::Key, values, 0x809C);
        }
        set_scalar(unit, MODE, PropType::Key, key(0xC853_6F16));
        set_array(unit, START_FLAGS, PropType::Int32, vec![Value::Int32(14)]);
        for (field, value) in self.fields() {
            set_scalar(unit, field, PropType::Float, Value::Float(value));
        }
        self.verify(unit)
    }

    fn fields(&self) -> impl Iterator<Item = (u32, f32)> + '_ {
        CAPACITY.into_iter().zip(self.capacity.map(f32::from))
            .chain(INCOME.into_iter().zip(self.income.map(f32::from)))
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        let catalog = super::behavior_conflicts::catalog()?;
        for id in PROVIDERS {
            let count = ENABLE.iter().try_fold(0, |count, &row| {
                array(unit, row).map(|values| count + values.iter()
                    .filter(|value| matches!(value, Value::Key(key) if key.instance == id)).count())
            })?;
            if !catalog.has_bundle(id) || count != 1
                || contains(unit, &[EXCLUDED], id)
            {
                return Err(format!("CASINO_BUNDLE: {id:08X}"));
            }
        }
        for id in REPLACED {
            if contains(unit, &ENABLE, id) || !contains(unit, &[EXCLUDED], id) {
                return Err(format!("CASINO_BUNDLE: {id:08X}"));
            }
        }
        for (field, value) in self.fields() {
            if unit.get(field).and_then(|p| p.scalar()) != Some(&Value::Float(value)) {
                return Err(format!("CASINO_FIELD: {field:08X}"));
            }
        }
        if unit.get(MODE).and_then(|p| p.scalar()) != Some(&key(0xC853_6F16))
            || array(unit, START_FLAGS)? != [Value::Int32(14)]
        {
            return Err("CASINO_FIELD: native operating state".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requires_native_tourism_and_writes_the_casino_cycle_once() {
        let casino = json!({"id":"casino","type":"casino","params":{"capacityW2":84}});
        let tourism = json!({"id":"tourism","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}});
        assert!(select(std::slice::from_ref(&casino), true).is_err());
        assert!(select(&[casino.clone(), tourism.clone()], false).is_err());
        let recipe = select(&[casino, tourism], true).unwrap().unwrap();
        let mut unit = PropertyFile::default();
        set_array(&mut unit, ENABLE[2], PropType::Key, vec![key(PROVIDERS[0]), key(REPLACED[1])]);
        recipe.apply(&mut unit).unwrap();
        recipe.apply(&mut unit).unwrap();
        assert_eq!(unit.get(CAPACITY[1]).unwrap().scalar(), Some(&Value::Float(84.0)));
        assert!(contains(&unit, &[EXCLUDED], 0x6827_D126));
        assert!(!contains(&unit, &ENABLE, 0xC140_44C4));
    }
}
