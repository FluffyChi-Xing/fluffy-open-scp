//! Production and labor references from the medium-tech office floor
//! 61EFC000:6EF67CC5. The ground host keeps its own entrance and identity.
use super::behavior_nodes::LaborSpec;
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use sc_properties::{Key, Kind, PropType, PropertyFile, Value};
use serde::Serialize;

const JOBS: [u32; 3] = [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7];
const MIN_WORKERS: u32 = 0x0A79_17C8;
const TOTAL_JOBS: u32 = 0x0A79_17C3;
const BASE_LABOR: u32 = 0xD07C_5A04;
const BUNDLES: [u32; 8] = [
    0x1E16_EDA9,
    0xBE47_F5A2,
    0x2B5A_2BAF,
    0x2285_8A20,
    0xDD6E_83EC,
    0x6A32_6F41,
    0x1BAB_0227,
    0xF236_BB93,
];
const PRODUCTION: [(u32, f32); 3] = [(0x0FC6_E602, 2.0), (0x0FC6_E604, 12.0), (0x0FC6_E605, 1.0)];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct Office {
    jobs: [i32; 3],
    minimum_workers: i32,
}

pub(super) fn select(nodes: &[serde_json::Value]) -> Result<Option<Office>, String> {
    let mut offices = nodes.iter().filter(|node| node["type"] == "office");
    let Some(node) = offices.next() else {
        return Ok(None);
    };
    if offices.next().is_some()
        || nodes.iter().any(|node| {
            matches!(
                node["type"].as_str(),
                Some("education" | "residence" | "commercial")
            )
        })
    {
        return Err("OFFICE_CONFLICT".into());
    }
    let params = &node["params"];
    let count = |name: &str, default| {
        let value = &params[name];
        if value.is_null() {
            return Ok(default);
        }
        value
            .as_i64()
            .filter(|value| (0..=100_000).contains(value))
            .map(|value| value as i32)
            .ok_or_else(|| format!("OFFICE_PARAMETER: {name}"))
    };
    let jobs = [count("w1", 400)?, count("w2", 200)?, count("w3", 100)?];
    let minimum_workers = count("minWorkers", 1)?;
    let total = jobs.iter().sum::<i32>();
    if !(1..=100_000).contains(&total) || !(1..=total).contains(&minimum_workers) {
        return Err("OFFICE_PARAMETER: minWorkers/totalJobs".into());
    }
    Ok(Some(Office {
        jobs,
        minimum_workers,
    }))
}

impl Office {
    pub(super) fn labor(&self, previous: Option<LaborSpec>) -> LaborSpec {
        LaborSpec {
            w1: self.jobs[0],
            w2: self.jobs[1],
            w3: self.jobs[2],
            min_workers: self.minimum_workers,
            schedule: previous.and_then(|spec| spec.schedule),
            slots: previous
                .and_then(|spec| spec.slots)
                .or(Some(self.jobs.map(|jobs| jobs / 2))),
        }
    }

    fn fields(&self) -> impl Iterator<Item = (u32, f32)> {
        JOBS.into_iter()
            .zip(self.jobs.map(|jobs| jobs as f32))
            .chain([
                (TOTAL_JOBS, self.jobs.iter().sum::<i32>() as f32),
                (MIN_WORKERS, self.minimum_workers as f32),
                (0x0A79_17CB, self.jobs.iter().sum::<i32>() as f32),
                (0x0C0F_228C, 9.0),
                (0x0C0F_2290, 54.0),
            ])
            .chain(PRODUCTION)
    }

    pub(super) fn patch(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        for hash in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(&unit, hash)?;
            let mut values = original.to_vec();
            values.retain(|value| {
                !matches!(value, Value::Key(key) if matches!(key.instance, BASE_LABOR | 0x7761_2FD4) || BUNDLES.contains(&key.instance))
            });
            if hash == ENABLE[3] {
                values.extend(BUNDLES.map(office_bundle));
            }
            if hash == EXCLUDED {
                values.extend([key(BASE_LABOR), key(0x7761_2FD4)]);
            }
            if values != original {
                set_array_with_flags(&mut unit, hash, PropType::Key, values, 0x809C);
            }
        }
        for (hash, value) in self.fields() {
            set_scalar(&mut unit, hash, PropType::Float, Value::Float(value));
            if let Some(property) = unit.values.iter_mut().find(|prop| prop.hash == hash) {
                property.encoding.flags = 0x8000;
                property.encoding.array_item_size = None;
            }
        }
        write_key(&mut unit, 0x0C0F_2297, 0xDE83_B355);
        write_key(&mut unit, 0x0D02_D846, 0x2E64_8431);
        if !array(&unit, 0x0A88_D179)?.contains(&Value::Int32(23)) {
            let mut flags = array(&unit, 0x0A88_D179)?.to_vec();
            flags.push(Value::Int32(23));
            set_array(&mut unit, 0x0A88_D179, PropType::Int32, flags);
        }
        self.verify(&unit)?;
        encode(&unit)
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        for bundle in BUNDLES {
            if !ENABLE.iter().any(|&hash| {
                array(unit, hash).is_ok_and(|row| row.contains(&office_bundle(bundle)))
            }) || contains(unit, &[EXCLUDED], bundle)
            {
                return Err(format!("OFFICE_BUNDLE: {bundle:08X}"));
            }
        }
        for (hash, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.kind == Kind::Scalar(Value::Float(value)))
            {
                return Err(format!("OFFICE_FIELD: {hash:08X}"));
            }
        }
        for (hash, id) in [(0x0C0F_2297, 0xDE83_B355), (0x0D02_D846, 0x2E64_8431)] {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.kind == Kind::Scalar(key(id)))
            {
                return Err(format!("OFFICE_FIELD: {hash:08X}"));
            }
        }
        if contains(unit, &ENABLE, 0x7761_2FD4)
            || !contains(unit, &[EXCLUDED], 0x7761_2FD4)
            || contains(unit, &ENABLE, BASE_LABOR)
            || !contains(unit, &[EXCLUDED], BASE_LABOR)
            || !array(unit, 0x0A88_D179)?.contains(&Value::Int32(23))
        {
            return Err("OFFICE_LABOR".into());
        }
        Ok(())
    }
}

fn office_bundle(instance: u32) -> Value {
    let group = match instance {
        0x1E16_EDA9 | 0x2285_8A20 | 0xDD6E_83EC | 0x6A32_6F41 | 0x1BAB_0227 => 0x61EF_C700,
        _ => 0x40E0_C700,
    };
    Value::Key(Key {
        type_id: super::behavior::PROPERTY_TYPE,
        group,
        instance,
    })
}

fn write_key(unit: &mut PropertyFile, hash: u32, instance: u32) {
    set_scalar(unit, hash, PropType::Key, key(instance));
    if let Some(prop) = unit.values.iter_mut().find(|prop| prop.hash == hash) {
        prop.encoding.flags = 0x8000;
        prop.encoding.array_item_size = None;
    }
}
