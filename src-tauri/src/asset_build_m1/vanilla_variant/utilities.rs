//! Consumer parameters on the variant Unit. Retail bundles resolve the
//! capacity and amount bindings against the effective Unit properties.
use super::*;

const CAPACITY_KEYS: u32 = 0x0A6F_E749;
const CAPACITY_FIELDS: u32 = 0x0A6F_E74A;
const EXACT_INTEGER_MAX: f64 = 16_777_216.0;

struct Consumer {
    amount: (u32, &'static str),
    capacity: (u32, &'static str),
    interval: (u32, &'static str),
    companion: u32,
}

const CONSUMERS: [Consumer; 2] = [
    Consumer {
        amount: (0x0B88_904E, "powerAmount"),
        capacity: (0x0C09_DAA0, "powerCapacity"),
        interval: (0x0AFB_9882, "powerInterval"),
        companion: 0x0C09_DAB2,
    },
    Consumer {
        amount: (0x0C09_DA65, "waterAmount"),
        capacity: (0x0C09_DA5A, "waterCapacity"),
        interval: (0x0C09_DA6A, "waterInterval"),
        companion: 0x0C09_DA6E,
    },
];

fn effective(chain: &[PropertyFile], hash: u32) -> Option<&Property> {
    chain.iter().find_map(|file| file.get(hash))
}

fn array(chain: &[PropertyFile], hash: u32) -> &[Value] {
    effective(chain, hash)
        .and_then(Property::array)
        .unwrap_or_default()
}

fn number(property: &Property) -> Option<f64> {
    match property.scalar()? {
        Value::Float(value) if value.is_finite() => Some(f64::from(*value)),
        Value::Int32(value) => Some(f64::from(*value)),
        Value::UInt32(value) => Some(f64::from(*value)),
        _ => None,
    }
}

fn active_capacity_fields(
    packages: &[Package],
    chain: &[PropertyFile],
) -> std::collections::HashSet<u32> {
    let excluded = array(chain, 0x0A02_09B2);
    (0x0A02_09A2..=0x0A02_09A5)
        .flat_map(|hash| array(chain, hash))
        .filter_map(|value| {
            match value {
            Value::Key(key) if key.instance != 0 && !excluded.iter().any(
                |value| matches!(value, Value::Key(excluded) if excluded.instance == key.instance)
            ) => Some(property_id(
                if key.group == 0 { 0x40E0_C700 } else { key.group },
                key.instance,
            )),
            _ => None,
        }
        })
        .filter_map(|id| {
            let bundle = read_property(packages, id).ok()?;
            rci::inheritance(packages, &bundle).ok()
        })
        .flat_map(|bundle| {
            array(&bundle, CAPACITY_KEYS)
                .iter()
                .zip(array(&bundle, CAPACITY_FIELDS))
                .filter_map(|(key, field)| match (key, field) {
                    (Value::Key(key), Value::UInt32(hash)) if key.instance != 0 => Some(*hash),
                    (Value::Key(key), Value::Int32(hash)) if key.instance != 0 => {
                        Some(*hash as u32)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(super) fn register(
    packages: &[Package],
    chain: &[PropertyFile],
) -> serde_json::Map<String, Json> {
    let bindings = active_capacity_fields(packages, chain);
    let mut fields = serde_json::Map::new();
    for consumer in &CONSUMERS {
        let bound =
            bindings.contains(&consumer.capacity.0) && bindings.contains(&consumer.amount.0);
        for (hash, name) in [consumer.amount, consumer.capacity, consumer.interval] {
            if let Some(original) = effective(chain, hash).and_then(number) {
                fields.insert(
                    name.into(),
                    json!({
                        "hash": format!("{hash:08X}"), "original": original, "value": original,
                        "consumerBound": bound,
                    }),
                );
            }
        }
    }
    fields
}

fn typed(property: &Property, value: f64) -> Result<Value, String> {
    match property.prop_type {
        PropType::Float => Ok(Value::Float(value as f32)),
        PropType::Int32 => Ok(Value::Int32(value as i32)),
        PropType::UInt32 => Ok(Value::UInt32(value as u32)),
        _ => Err("unexpected consumer property type".into()),
    }
}

pub(super) fn apply(
    fields: &Json,
    chain: &[PropertyFile],
    unit: &mut PropertyFile,
) -> Result<(), String> {
    for consumer in &CONSUMERS {
        let mut resulting = [None; 3];
        let mut writes = Vec::new();
        for (index, (hash, name)) in [consumer.amount, consumer.capacity, consumer.interval]
            .into_iter()
            .enumerate()
        {
            let property = effective(chain, hash);
            let original = property.and_then(number);
            resulting[index] = original;
            let Some(field) = fields.get(name) else {
                continue;
            };
            let property = property
                .filter(|_| original.is_some())
                .ok_or_else(|| format!("missing source field {name}"))?;
            let value = field["value"]
                .as_f64()
                .ok_or_else(|| format!("invalid value for {name}"))?;
            if Some(value) == original {
                continue;
            }
            let minimum = if index == 2 { 1.0 } else { 0.0 };
            if !(minimum..=EXACT_INTEGER_MAX).contains(&value) || value.fract() != 0.0 {
                return Err(format!(
                    "consumer field {name} must be a whole number from {minimum} to {EXACT_INTEGER_MAX}"
                ));
            }
            resulting[index] = Some(value);
            writes.push((hash, property.prop_type, typed(property, value)?));
        }
        if writes.is_empty() {
            continue;
        }
        if let [Some(amount), Some(capacity), _] = resulting
            && capacity < amount
        {
            return Err(format!(
                "{} must be at least {}",
                consumer.capacity.1, consumer.amount.1
            ));
        }
        if let Some(capacity) = resulting[1]
            && writes
                .iter()
                .any(|(hash, _, _)| *hash == consumer.capacity.0)
            && let Some(companion) = effective(chain, consumer.companion)
            && number(companion).is_some()
            && number(companion) == effective(chain, consumer.capacity.0).and_then(number)
        {
            scalar(
                unit,
                consumer.companion,
                companion.prop_type,
                typed(companion, capacity)?,
            );
        }
        for (hash, kind, value) in writes {
            scalar(unit, hash, kind, value);
        }
    }
    Ok(())
}
