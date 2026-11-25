//! Ground residence records from the base-game D2 residential templates
//! 48E0C000:2BD0E3B2 / 513E2625 / CACB43D4, with the Copan ground access
//! wiring. See docs/re/residential-wealth-recipes-20261113.md.

use super::behavior_residence_d3 as d3;
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::set_array_with_flags;
use sc_properties::{PropType, PropertyFile, Value};
use serde::Serialize;

const COMMON: &[u32] = &[
    0x5C74_D18B,
    0xA5B9_D272,
    d3::GROWTH_STATE,
    0x9DEF_44FB,
    0x3748_6456,
    0x44F5_CF6C,
    0xF90C_8A94,
    0x1418_571B,
    0xE447_343A,
    0x323A_4CF2,
];
const GROUND: &[u32] = &[0x5D5E_F1FF, 0xA3F5_54BD, 0xE238_1694, 0xCC2B_572F];
const W1: &[u32] = &[
    0xD4C3_0C02,
    0x7378_2B22,
    0x568C_CCA3,
    0xCC2A_40B6,
    0xE648_41C9,
    0x7C37_AF27,
    0x4E7D_5743,
    0xBAC9_6038,
    0x3B72_84AB,
    0xEE66_978F,
    0xB995_3318,
    0x999F_8485,
    0x927C_3F78,
];
const W2: &[u32] = &[
    0xA8CB_9DE1,
    0x7378_2B21,
    0x568C_CCA0,
    0xCC2A_40B5,
    0xE648_41CA,
    0x290A_7678,
    0x4E7D_5740,
    0xBAC9_603B,
    0x3B72_84A8,
    0x1AA6_1FDC,
    0x0C69_05BC,
    0xD2F2_FDEB,
    0xA797_4BCE,
    0xA47A_1D3B,
];
const W3: &[u32] = &[
    0xAC8B_8A48,
    0x7378_2B20,
    0x568C_CCA1,
    0xCC2A_40B4,
    0xE648_41CB,
    0x0DD3_A3B9,
    0x4E7D_5741,
    0xBAC9_603A,
    0x3B72_84A9,
    0x1AA6_1FDC,
    0xFA6B_27FD,
    0x0089_B3DA,
    0x8D9A_F497,
    0x7E77_A292,
];
const ALTERNATE: &[u32] = &[
    0xAB9A_0A3E,
    0x933F_0FBF,
    0x6142_47C7,
    0xEE66_978E,
    0x999F_8484,
    0xE238_1695,
    0x5D5E_F1FE,
    0x5CB4_02D9,
    0x78A7_30CD,
    0x78A7_30CE,
    0x78A7_30CF,
    0xEBFC_1753,
    0xEBFC_1750,
    0xEBFC_1751,
    0x0C69_05BD,
    0xFA6B_27FC,
];
const INITIAL_FLAGS: u32 = 0x0A88_D179;
const VACANT: Value = Value::Int32(22);

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(super) enum Wealth {
    W1,
    W2,
    W3,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct GroundResidence {
    wealth: Wealth,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    high_density: bool,
    pub population: u32,
    workers: u32,
    shoppers: u32,
    children: u32,
    happiness_initial: u32,
    happiness_capacity: u32,
}

impl GroundResidence {
    pub(super) fn select(params: &serde_json::Value, id: &str) -> Result<Option<Self>, String> {
        let profile = params["residenceProfile"].as_str();
        let high_density = profile == Some(d3::PROFILE);
        let (wealth, base) = match profile {
            Some(d3::PROFILE) => (Wealth::W1, d3::POPULATION),
            Some("ground-residence-w1-v1") => (Wealth::W1, [60, 40, 15]),
            Some("ground-residence-w2-v1") => (Wealth::W2, [30, 20, 10]),
            Some("ground-residence-w3-v1") => (Wealth::W3, [10, 7, 4]),
            _ => {
                if ["residentWorkers", "residentShoppers", "residentChildren", "happinessInitial", "happinessCapacity"]
                    .into_iter()
                    .any(|field| !params[field].is_null())
                {
                    return Err(format!("RESIDENCE_GROUND_PROFILE: {id}"));
                }
                return Ok(None);
            }
        };
        let count = |field: &str, default: u32, min, max| {
            let value = &params[field];
            if value.is_null() {
                return Ok(default);
            }
            value
                .as_f64()
                .filter(|value| {
                    value.fract() == 0.0 && (f64::from(min)..=f64::from(max)).contains(value)
                })
                .map(|value| value as u32)
                .ok_or_else(|| format!("RESIDENCE_GROUND_CAPACITY: {id}/{field}"))
        };
        let population = count("population", base[0], 1, 100_000)?;
        let scaled = |stock: u32| (stock * population + base[0] / 2) / base[0];
        let shoppers = count("residentShoppers", population - scaled(base[1]), 0, population)?;
        let workers = count("residentWorkers", population - shoppers, 0, population)?;
        let shoppers = count("residentShoppers", population - workers, 0, population)?;
        if workers + shoppers != population {
            return Err(format!("RESIDENCE_GROUND_ROLES: {id}"));
        }
        let happiness_capacity = count("happinessCapacity", population * 11, 1, 1_100_000)?;
        Ok(Some(Self {
            wealth,
            high_density,
            population,
            workers,
            shoppers,
            children: count("residentChildren", scaled(base[2]), 0, population)?,
            happiness_initial: count("happinessInitial", population.min(happiness_capacity), 0, happiness_capacity)?,
            happiness_capacity,
        }))
    }

    fn variant(&self) -> &'static [u32] {
        if self.high_density {
            return d3::BUNDLES;
        }
        match self.wealth {
            Wealth::W1 => W1,
            Wealth::W2 => W2,
            Wealth::W3 => W3,
        }
    }

    pub(super) fn bundles(&self) -> impl Iterator<Item = u32> {
        let ground = if self.high_density { d3::GROUND } else { GROUND };
        COMMON.iter().copied().map(|id| {
            // Every workbench residence is ploppable. The zoning construction
            // initializer is replaced by Copan's plop lifecycle at every density.
            if id == d3::GROWTH_STATE { d3::PLOP_STATE } else { id }
        }).chain(self.variant().iter().chain(ground).copied())
    }

    pub(super) fn excluded(&self) -> impl Iterator<Item = u32> {
        ALTERNATE
            .iter()
            .chain(W1)
            .chain(W2)
            .chain(W3)
            .chain(std::iter::once(&d3::GROWTH_STATE))
            .chain(if self.high_density { d3::ALTERNATE } else { &[] })
            .copied()
            .filter(|id| !(self.variant().contains(id) || *id == d3::PLOP_STATE))
    }

    pub(super) fn fields(&self) -> Vec<(u32, PropType, Value)> {
        let (unit_kind, density_key, wealth, happiness, commute, delay, crime, hour, service) =
            match self.wealth {
                Wealth::W1 => (
                    0x31D3_307B,
                    0x192F_CF57,
                    1.0,
                    450.0,
                    4.0,
                    80.0,
                    0.28,
                    242.0,
                    6.0,
                ),
                Wealth::W2 => (
                    0x02CA_56B2,
                    0x3732_3D04,
                    2.0,
                    270.0,
                    3.0,
                    192.0,
                    0.2,
                    241.0,
                    3.0,
                ),
                Wealth::W3 => (
                    0x10BA_89C9,
                    0x2534_5F45,
                    3.0,
                    78.0,
                    2.0,
                    224.0,
                    0.12,
                    241.0,
                    2.0,
                ),
            };
        let mut fields = [
            (0x0A79_17CB, self.population as f32),
            (0x0A79_17C2, self.population as f32),
            (0x0A79_17CC, self.children as f32),
            (0x0E33_502E, self.workers as f32),
            (0x0E33_5022, self.shoppers as f32),
            (0x0A79_17C4, delay),
            (0x0C20_0553, 2.0),
            (0x0D08_4715, wealth),
            (0x0DF0_635A, 6.0),
            (0x0E54_6790, self.happiness_initial as f32),
            (0x0E54_6791, self.happiness_capacity as f32),
            (0x0E54_6792, happiness),
            (0x0E83_DE1C, commute),
            (0x0E83_DE27, commute),
            (0x0D7B_1448, 0.07),
            (0x0D7B_1449, 0.18),
            (0x0D7B_144A, 0.30),
            (0x0D7B_144B, 0.75),
            (0x0E0C_1481, 13000.0),
            (0x0C12_C537, 0.2),
            (0x0D63_43C2, crime),
            (0x0D63_5C98, hour),
            (0x0D89_7622, service),
        ]
        .map(|(hash, value)| {
            let value = if self.high_density {
                d3::FIELDS.iter().find(|field| field.0 == hash).map_or(value, |field| field.1)
            } else {
                value
            };
            (hash, PropType::Float, Value::Float(value))
        })
        .to_vec();
        let density_key = if self.high_density { d3::DENSITY } else { density_key };
        fields.extend([
            (0x0D02_D846, PropType::Key, key(unit_kind)),
            (0x0E30_E65A, PropType::Key, key(density_key)),
            (0x0EAC_DB63, PropType::Key, key(0xB892_8907)),
        ]);
        fields
    }

    pub(super) fn patch(&self, unit: &mut PropertyFile) -> Result<(), String> {
        let selected: Vec<_> = self.bundles().collect();
        let excluded: Vec<_> = self.excluded().collect();
        for hash in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(unit, hash)?;
            let mut values = original.to_vec();
            values.retain(|value| {
                !matches!(value, Value::Key(key)
                if selected.contains(&key.instance) || excluded.contains(&key.instance))
            });
            if hash == ENABLE[3] {
                values.extend(selected.iter().copied().map(key));
            }
            if hash == EXCLUDED {
                for bundle in excluded.iter().copied().map(key) {
                    if !values.contains(&bundle) {
                        values.push(bundle);
                    }
                }
            }
            if values != original {
                set_array_with_flags(unit, hash, PropType::Key, values, 0x809C);
            }
        }
        let mut flags = array(unit, INITIAL_FLAGS)?.to_vec();
        flags.retain(|flag| !d3::blocks_move_in(flag));
        if !flags.contains(&VACANT) {
            flags.push(VACANT);
        }
        if flags != array(unit, INITIAL_FLAGS)? {
            super::property_edit::set_array(unit, INITIAL_FLAGS, PropType::Int32, flags);
        }
        self.verify(unit)
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        let flags = array(unit, INITIAL_FLAGS)?;
        if !flags.contains(&VACANT) || flags.iter().any(d3::blocks_move_in) {
            return Err("RESIDENCE_GROUND_VACANT".into());
        }
        for id in self.bundles() {
            let count = ENABLE.into_iter().try_fold(0, |count, hash| {
                Ok::<_, String>(
                    count
                        + array(unit, hash)?
                            .iter()
                            .filter(|value| matches!(value, Value::Key(key) if key.instance == id))
                            .count(),
                )
            })?;
            if count != 1 || contains(unit, &[EXCLUDED], id) {
                return Err(format!("RESIDENCE_GROUND_BUNDLE: {id:08X}"));
            }
        }
        for id in self.excluded() {
            if contains(unit, &ENABLE, id) || !contains(unit, &[EXCLUDED], id) {
                return Err(format!("RESIDENCE_GROUND_ALTERNATE: {id:08X}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_ground_wealth_uses_plop_construction_and_accepts_move_in() {
        for profile in ["ground-residence-w1-v1", "ground-residence-w2-v1", "ground-residence-w3-v1"] {
            let residence = GroundResidence::select(
                &json!({"residenceProfile":profile,"population":600}), "home"
            ).unwrap().unwrap();
            let mut unit = PropertyFile::default();
            super::super::property_edit::set_array(
                &mut unit, INITIAL_FLAGS, PropType::Int32,
                [13, 14, 49, 23].map(Value::Int32).to_vec(),
            );
            residence.patch(&mut unit).unwrap();
            assert_eq!(array(&unit, INITIAL_FLAGS).unwrap(),
                [Value::Int32(49), Value::Int32(23), VACANT]);
            assert!(contains(&unit, &ENABLE, d3::PLOP_STATE));
            assert!(!contains(&unit, &ENABLE, d3::GROWTH_STATE));
            assert!(contains(&unit, &[EXCLUDED], d3::GROWTH_STATE));
            let bytes = super::super::property_edit::encode(&unit).unwrap();
            residence.verify(&PropertyFile::parse(&bytes).unwrap()).unwrap();
        }
    }

    #[test]
    fn high_density_ground_residence_uses_native_intake_tax_and_happiness() {
        let params = json!({"residenceProfile":d3::PROFILE,"population":1100,
            "residentWorkers":800,"residentShoppers":300});
        let residence = GroundResidence::select(&params, "home").unwrap().unwrap();
        let fields: std::collections::BTreeMap<_, _> = residence.fields().into_iter()
            .map(|(hash, _, value)| (hash, value)).collect();
        assert_eq!(fields[&0x0E54_6790], Value::Float(1100.0));
        assert_eq!(fields[&0x0E54_6791], Value::Float(12100.0));
        assert_eq!(fields[&0x0A79_17CC], Value::Float(183.0));
        for id in [0x999F_8484, 0x927C_3F79, 0xCC2B_572E] {
            assert!(residence.bundles().any(|bundle| bundle == id));
            assert!(!residence.excluded().any(|bundle| bundle == id));
        }
        for id in [0x999F_8485, 0x927C_3F78, 0x78A7_30CD, 0xEBFC_1753] {
            assert!(residence.excluded().any(|bundle| bundle == id));
        }
        let mut unit = PropertyFile::default();
        super::super::property_edit::set_array(
            &mut unit,
            INITIAL_FLAGS,
            PropType::Int32,
            [13, 14, 49, 22].map(Value::Int32).to_vec(),
        );
        residence.patch(&mut unit).unwrap();
        assert_eq!(array(&unit, INITIAL_FLAGS).unwrap(), [Value::Int32(49), VACANT]);
        assert!(contains(&unit, &ENABLE, d3::PLOP_STATE));
        assert!(!contains(&unit, &ENABLE, d3::GROWTH_STATE));
        assert!(contains(&unit, &[EXCLUDED], d3::GROWTH_STATE));
    }

    #[test]
    fn sizes_ground_happiness_and_preserves_resident_role_accounting() {
        let mut params = json!({"residenceProfile":"ground-residence-w1-v1","population":600,
            "residentShoppers":180,"residentChildren":75,"happinessCapacity":6800});
        let residence = GroundResidence::select(&params, "home").unwrap().unwrap();
        let fields: std::collections::BTreeMap<_, _> = residence.fields().into_iter()
            .map(|(hash, _, value)| (hash, value)).collect();
        for (hash, value) in [(0x0E33_502E, 420.0), (0x0E33_5022, 180.0), (0x0A79_17CC, 75.0),
            (0x0E54_6790, 600.0), (0x0E54_6791, 6800.0), (0x0E54_6792, 450.0)] {
            assert_eq!(fields[&hash], Value::Float(value));
        }
        params["residentWorkers"] = json!(419);
        assert!(GroundResidence::select(&params, "home").is_err());
        params["residentWorkers"] = json!(420);
        params["happinessCapacity"] = json!(false);
        assert!(GroundResidence::select(&params, "home").is_err());
    }
}
