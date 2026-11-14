//! Ground W2 retail from 40E0C000:2DDD403E and its effective parent chain.
use super::behavior_commercial_wealth::RetailWealth;
use super::behavior_nodes::LaborSpec;
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use sc_properties::{Key, Kind, PropType, PropertyFile, Value};
use serde::Serialize;

const BUNDLES: [u32; 7] = [
    0x2B5A_2BAF,
    0x3620_87BD,
    0x9448_0760,
    0x3A82_63E2,
    0xC3EC_8B4D,
    0x9957_A6A5,
    0xD07C_5A04,
];
const ALTERNATE: [u32; 8] = [
    0xBE47_F5A2,
    0x3A82_63E3,
    0x3A82_63E0,
    // The generic ground donor's park visitors also claim shopper space.
    0x6EA8_E701,
    0x265F_B8DD,
    0xC067_2F57,
    0x5FFA_86B9,
    0xEA02_54BF,
];
const CATEGORY_FIELD: u32 = 0x0E30_E65A;
const CATEGORY: u32 = 0x7136_D91E;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct Commercial {
    #[serde(skip_serializing_if = "Option::is_none")]
    wealth: Option<RetailWealth>,
    jobs: [i32; 3],
    minimum_workers: i32,
    shoppers: i32,
    goods: i32,
}

pub(super) fn select(nodes: &[serde_json::Value]) -> Result<Option<Commercial>, String> {
    let mut leaves = nodes.iter().filter(|node| node["type"] == "commercial");
    let Some(leaf) = leaves.next() else {
        return Ok(None);
    };
    if leaves.next().is_some()
        || nodes.iter().any(|node| {
            matches!(
                node["type"].as_str(),
                Some("education" | "tourism" | "residence" | "office" | "park")
            )
        })
    {
        return Err("COMMERCIAL_CONFLICT".into());
    }
    let params = &leaf["params"];
    let wealth = RetailWealth::select(params)?;
    let profile = wealth.map(RetailWealth::profile);
    let count = |name: &str, default, min| {
        if params[name].is_null() {
            return Ok(default);
        }
        params[name]
            .as_i64()
            .filter(|value| (min..=100_000).contains(value))
            .map(|value| value as i32)
            .ok_or_else(|| format!("COMMERCIAL_PARAMETER: {name}"))
    };
    let [w1, w2, w3] = profile.as_ref().map_or([3, 2, 1], |profile| profile.jobs);
    let jobs = [
        count("w1", w1, 0)?,
        count("w2", w2, 0)?,
        count("w3", w3, 0)?,
    ];
    let minimum_workers = count("minWorkers", 3, 1)?;
    let total = jobs.iter().sum::<i32>();
    if total > 100_000 || minimum_workers > total {
        return Err("COMMERCIAL_PARAMETER: minWorkers/totalJobs".into());
    }
    Ok(Some(Commercial {
        wealth,
        jobs,
        minimum_workers,
        shoppers: count(
            "shoppers",
            profile.as_ref().map_or(9, |profile| profile.shoppers),
            1,
        )?,
        goods: count(
            "goods",
            profile.as_ref().map_or(9, |profile| profile.goods),
            1,
        )?,
    }))
}

impl Commercial {
    pub(super) fn verify_lot(
        &self,
        resources: &super::connector::Resources,
        id: &dbpf::ResourceId,
    ) -> Result<(), String> {
        if self.wealth.is_none() {
            return Ok(());
        }
        let bytes = resources.get(id).ok_or("COMMERCIAL_RESOURCE: Lot")?;
        let lot = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        if !super::behavior_tourism::has_visitor_access(
            array(&lot, 0x0CC8_FE7A)?,
            array(&lot, 0x0D9A_A2BB)?,
        ) {
            return Err(format!("COMMERCIAL_ROAD_ACCESS: {id}"));
        }
        Ok(())
    }

    pub(super) fn bundles(&self) -> Vec<u32> {
        self.wealth
            .map_or_else(|| BUNDLES.to_vec(), |tier| tier.bundles().collect())
    }

    fn excluded(&self) -> Vec<u32> {
        self.wealth
            .map_or_else(|| ALTERNATE.to_vec(), |tier| tier.excluded().collect())
    }

    fn category(&self) -> u32 {
        self.wealth.map_or(CATEGORY, |tier| tier.profile().category)
    }

    pub(super) fn labor(&self, previous: Option<LaborSpec>) -> LaborSpec {
        LaborSpec {
            w1: self.jobs[0],
            w2: self.jobs[1],
            w3: self.jobs[2],
            min_workers: self.minimum_workers,
            schedule: previous.and_then(|spec| spec.schedule).or_else(|| {
                Some(
                    self.wealth
                        .map_or([2, 2, 3, 3, 2, 2, 3, 2], |tier| tier.profile().shifts),
                )
            }),
            slots: previous
                .and_then(|spec| spec.slots)
                .or_else(|| Some(self.wealth.map_or([2, 1, 0], |tier| tier.profile().slots))),
        }
    }

    fn fields(&self) -> impl Iterator<Item = (u32, f32)> {
        let profile = self.wealth.map(RetailWealth::profile);
        let (population, labor_tuning, profit, vacancy_tuning, freight_cycle) = profile
            .as_ref()
            .map_or((12.0, 6.0, 96.0, 60.0, 216.0), |profile| {
                (
                    profile.population,
                    profile.labor_tuning,
                    profile.profit,
                    profile.vacancy_tuning,
                    108.0,
                )
            });
        [
            (0x0A79_17C3, self.jobs.iter().sum::<i32>() as f32),
            (0x0A79_17C5, self.jobs[0] as f32),
            (0x0A79_17C6, self.jobs[1] as f32),
            (0x0A79_17C7, self.jobs[2] as f32),
            (0x0A79_17C8, self.minimum_workers as f32),
            (0x0AD6_A725, self.shoppers as f32),
            (0x0DA3_EE15, self.goods as f32),
            (0x0A79_17CB, population),
            (0x0AB9_85AA, labor_tuning),
            (0x0AB9_85AB, 1.0),
            (0x0E54_68F3, profit),
            (0x0C0F_228C, 3.0),
            (0x0C0F_2290, freight_cycle),
            (0x0C0F_228F, 500.0),
            (0x0C0F_2291, 125.0),
            (0x0C0F_2296, 375.0),
            (0x0C0F_2298, 63.0),
            (0x0A79_17C4, vacancy_tuning),
        ]
        .into_iter()
        .chain(profile.map(|profile| (0x0E54_67E7, profile.profit)))
    }

    pub(super) fn patch(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        let bundles = self.bundles();
        let alternate = self.excluded();
        for hash in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(&unit, hash)?;
            let mut values = original.to_vec();
            values.retain(|value| {
                !matches!(value, Value::Key(key)
                if bundles.contains(&key.instance) || alternate.contains(&key.instance))
            });
            if hash == ENABLE[3] {
                values.extend(bundles.iter().copied().map(native_bundle));
            }
            if hash == EXCLUDED {
                values.extend(alternate.iter().copied().map(key));
            }
            if values != original {
                set_array_with_flags(&mut unit, hash, PropType::Key, values, 0x809C);
            }
        }
        for (hash, value) in self.fields() {
            set_scalar(&mut unit, hash, PropType::Float, Value::Float(value));
        }
        set_scalar(
            &mut unit,
            CATEGORY_FIELD,
            PropType::Key,
            key(self.category()),
        );
        if let Some(wealth) = self.wealth {
            set_scalar(
                &mut unit,
                0x0D02_D846,
                PropType::Key,
                key(wealth.profile().unit_kind),
            );
        }
        for property in unit.values.iter_mut().filter(|prop| {
            prop.hash == CATEGORY_FIELD
                || (self.wealth.is_some() && prop.hash == 0x0D02_D846)
                || self.fields().any(|(hash, _)| hash == prop.hash)
        }) {
            property.encoding.flags = 0x8000;
            property.encoding.array_item_size = None;
        }
        let mut flags = array(&unit, 0x0A88_D179)?.to_vec();
        if !flags.contains(&Value::Int32(23)) {
            flags.push(Value::Int32(23));
            set_array(&mut unit, 0x0A88_D179, PropType::Int32, flags);
        }
        self.verify(&unit)?;
        encode(&unit)
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        for id in self.bundles() {
            if !ENABLE
                .iter()
                .any(|&hash| array(unit, hash).is_ok_and(|row| row.contains(&native_bundle(id))))
                || contains(unit, &[EXCLUDED], id)
            {
                return Err(format!("COMMERCIAL_BUNDLE: {id:08X}"));
            }
        }
        for id in self.excluded() {
            if contains(unit, &ENABLE, id) || !contains(unit, &[EXCLUDED], id) {
                return Err(format!("COMMERCIAL_BUNDLE: {id:08X}"));
            }
        }
        for (hash, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.kind == Kind::Scalar(Value::Float(value)))
            {
                return Err(format!("COMMERCIAL_FIELD: {hash:08X}"));
            }
        }
        if !unit
            .get(CATEGORY_FIELD)
            .is_some_and(|prop| prop.kind == Kind::Scalar(key(self.category())))
            || !array(unit, 0x0A88_D179)?.contains(&Value::Int32(23))
        {
            return Err("COMMERCIAL_FIELD: retail category or operation flag".into());
        }
        if let Some(wealth) = self.wealth {
            let catalog = super::behavior_conflicts::catalog()?;
            for id in self.bundles() {
                let count =
                    ENABLE.into_iter().try_fold(0, |count, hash| {
                        Ok::<_, String>(count + array(unit, hash)?.iter().filter(|value|
                        matches!(value, Value::Key(key) if key.instance == id)).count())
                    })?;
                if count != 1 || !catalog.has_bundle(id) {
                    return Err(format!("COMMERCIAL_BUNDLE: {id:08X}"));
                }
            }
            if unit.get(0x0D02_D846).and_then(|property| property.scalar())
                != Some(&key(wealth.profile().unit_kind))
            {
                return Err("COMMERCIAL_FIELD: 0D02D846".into());
            }
        }
        Ok(())
    }
}

fn native_bundle(instance: u32) -> Value {
    Value::Key(Key {
        type_id: super::behavior::PROPERTY_TYPE,
        group: 0x40E0_C700,
        instance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn commercial_wealth_uses_one_customer_and_profit_provider() {
        for (tier, shopper, cycle, shops, goods) in [
            ("w1", 0x3A82_63E1, 0x6827_D125, 10.0, 21.0),
            ("w2", 0x3A82_63E2, 0x6827_D126, 8.0, 18.0),
            ("w3", 0x3A82_63E3, 0x6827_D127, 7.0, 15.0),
        ] {
            let profile = format!("ground-commercial-{tier}-v1");
            let program = super::super::behavior_nodes::compile(
                None,
                &[json!({
                    "id": "retail", "type": "commercial", "params": {"commercialProfile": profile},
                })],
            )
            .unwrap();
            let recipe = program.commercial.unwrap();
            let mut donor = PropertyFile::default();
            set_array(
                &mut donor,
                ENABLE[0],
                PropType::Key,
                vec![key(0x3A82_63E2), key(0x6EA8_E701), key(0xBE47_F5A2)],
            );
            let bytes = recipe.patch(&encode(&donor).unwrap()).unwrap();
            let unit = PropertyFile::parse(&bytes).unwrap();
            for id in [shopper, cycle, 0x7424_049F, 0x9448_0760] {
                assert!(
                    array(&unit, ENABLE[3])
                        .unwrap()
                        .contains(&native_bundle(id))
                );
            }
            for id in [0x6EA8_E701, 0xBE47_F5A2] {
                assert!(contains(&unit, &[EXCLUDED], id));
            }
            assert_eq!(
                unit.get(0x0AD6_A725).unwrap().scalar(),
                Some(&Value::Float(shops))
            );
            assert_eq!(
                unit.get(0x0DA3_EE15).unwrap().scalar(),
                Some(&Value::Float(goods))
            );
            assert_eq!(recipe.patch(&bytes).unwrap(), bytes);
            let lot_id = dbpf::ResourceId {
                type_id: super::super::behavior::PROPERTY_TYPE,
                group: super::super::menu::lot_group(),
                instance: 1,
            };
            let mut resources = super::super::connector::Resources::new();
            assert!(recipe.verify_lot(&resources, &lot_id).is_err());
            let mut lot = PropertyFile::default();
            set_array(
                &mut lot,
                0x0CC8_FE7A,
                PropType::Key,
                vec![Value::Key(Key {
                    type_id: super::super::behavior::PROPERTY_TYPE,
                    group: 0x40E0_C400,
                    instance: 0x418A_18B1,
                })],
            );
            set_array(&mut lot, 0x0D9A_A2BB, PropType::Key, vec![key(0xC186_DA37)]);
            resources.insert(lot_id, encode(&lot).unwrap());
            recipe.verify_lot(&resources, &lot_id).unwrap();
        }
    }
}
