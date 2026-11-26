//! Ground hotel, from the effective medium-density templates AF6F44EC, 971A4B13 and 7014C68A.
use super::behavior_nodes::LaborSpec;
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use sc_properties::{PropType, PropertyFile, Value};
use serde::Serialize;

pub(super) const INTAKE: u32 = 0x729A_6626;
pub(super) const PROCESSOR_VERSION: &str = "openscp.property.m1/plop-hotel-1";
// The daily RCI profit cycle can write Abandoned after the profit check fails.
const RCI_PROFIT_CYCLE: u32 = 0x6827_D126;
const BUNDLES: [u32; 17] = [
    0x5C74_D18B,
    0xF90C_8A94,
    0x3748_6456,
    0x44F5_CF6C,
    0x0D19_711A,
    0x234B_6693,
    0x2B5A_2BAF,
    0x3620_87BD,
    0x9448_0760,
    0xE62E_CB66,
    0xC3EC_8B4D,
    0x9957_A6A5,
    0x7424_049C,
    0xD07C_5A04,
    0xF236_BB93,
    INTAKE,
    0x8748_B5A5,
];
const EXCLUDE: [u32; 26] = [
    0xBC76_13A3,
    0x3064_5625,
    0xBE47_F5A2,
    0x7424_049F,
    0x5D5E_F1FE,
    0x5D5E_F1FF,
    0xE153_69A2,
    0x75D2_2FD2,
    0x75D2_2FD1,
    0x75D2_2FD0,
    0x352F_4263,
    0x3A82_63E0,
    0x3A82_63E1,
    0x3A82_63E2,
    0x3A82_63E3,
    0x6827_D125,
    RCI_PROFIT_CYCLE,
    0x6827_D127,
    0x6EA8_E701,
    0x265F_B8DD,
    0x265F_B8DE,
    0x265F_B8DF,
    0xC067_2F54,
    0xC067_2F55,
    0xC067_2F57,
    0xC067_2F56,
];
const FLAGS: u32 = 0x0A88_D179;
const ROOMS: u32 = 0x0E26_60C6;
const INTAKES: [u32; 3] = [0x729A_6625, INTAKE, 0x729A_6627];
const CAPACITIES: [u16; 3] = [76, 68, 60];
const HOTEL_LEDGER: [u32; 3] = [0xC3EC_8B4E, 0xC3EC_8B4D, 0xC3EC_8B4C];
const HOTEL_DEMAND: [u32; 3] = [0xF25B_DF56, 0x9957_A6A5, 0xCA24_5C9C];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct Hotel {
    rooms: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    wealth: Option<u8>,
}

pub(super) fn select(nodes: &[serde_json::Value]) -> Result<Option<Hotel>, String> {
    let Some(node) = nodes.iter().find(|node| node["type"] == "hotel") else {
        return Ok(None);
    };
    let wealth = match &node["params"]["wealth"] {
        serde_json::Value::Null => None,
        value => {
            let value = value.as_u64().filter(|value| (1..=3).contains(value)).ok_or("HOTEL_WEALTH")?;
            (value != 2).then_some(value as u8)
        }
    };
    let rooms = &node["params"]["rooms"];
    let rooms = if rooms.is_null() {
        Some(u64::from(CAPACITIES[usize::from(wealth.unwrap_or(2) - 1)]))
    } else {
        rooms.as_u64()
    };
    rooms
        .filter(|rooms| (1..=1000).contains(rooms))
        .map(|rooms| {
            Some(Hotel {
                rooms: rooms as u16,
                wealth,
            })
        })
        .ok_or_else(|| "HOTEL_ROOMS".into())
}

impl Hotel {
    pub(super) fn needs_tiered_panel(&self) -> bool {
        self.wealth.is_some()
    }

    fn tier(&self) -> usize {
        usize::from(self.wealth.unwrap_or(2) - 1)
    }

    pub(super) fn labor(&self, previous: Option<LaborSpec>) -> LaborSpec {
        previous.unwrap_or(LaborSpec {
            w1: 20,
            w2: 10,
            w3: 4,
            min_workers: 12,
            slots: Some([10, 5, 0]),
            schedule: Some([6, 6, 7, 7, 6, 6, 7, 6]),
        })
    }

    pub(super) fn bundles(&self) -> impl Iterator<Item = u32> {
        let tier = self.tier();
        BUNDLES.into_iter().map(move |id| match id {
            INTAKE => INTAKES[tier],
            0xC3EC_8B4D => HOTEL_LEDGER[tier],
            0x9957_A6A5 => HOTEL_DEMAND[tier],
            _ => id,
        })
    }

    fn fields(&self) -> impl Iterator<Item = (u32, PropType, Value)> {
        let tier = self.tier();
        let native_rooms = f32::from(CAPACITIES[tier]);
        [
            (ROOMS, f32::from(self.rooms)),
            (0x0A79_17CB, native_rooms),
            (0x0C20_0553, 2.0),
            (0x0D08_4715, [1.0, 2.0, 3.0][tier]),
            (0x0A7B_B621, 16.0),
            (0x0B4F_8ECC, 40.0),
            (0x0CBD_3000, 1.0),
            (0x0BE3_AD6A, 0.0),
            (0x0D76_E8E7, [12.0, 8.0, 4.0][tier]),
            (0x0A79_17C4, [104.0, 120.0, 256.0][tier]),
            (0x0AB9_85AA, native_rooms / 2.0),
            (0x0AB9_85AB, 1.0),
            (0x0E54_68F3, native_rooms * 8.0),
            (0x0E54_67E7, native_rooms * 8.0),
            (0x0C0F_228C, 6.0),
            (0x0C0F_2290, 144.0),
            (0x0C0F_228F, 1000.0),
            (0x0C0F_2291, 250.0),
            (0x0C0F_2296, 750.0),
            (0x0C0F_2298, 125.0),
        ]
        .into_iter()
        .map(|(hash, value)| (hash, PropType::Float, Value::Float(value)))
        .chain([
            (0x0E30_E65A, PropType::Key, key([0x7F2E_A046, 0x7136_D91D, 0x8333_23DC][tier])),
            (0x0D02_D846, PropType::Key, key([0xCAF1_511E, 0x2E64_8431, 0x03DD_CA70][tier])),
        ])
    }

    pub(super) fn patch(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        for hash in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(&unit, hash)?;
            let mut values = original.to_vec();
            values.retain(|value| {
                !matches!(value, Value::Key(key)
                if BUNDLES.contains(&key.instance) || EXCLUDE.contains(&key.instance)
                || INTAKES.contains(&key.instance) || HOTEL_LEDGER.contains(&key.instance)
                || HOTEL_DEMAND.contains(&key.instance))
            });
            if hash == ENABLE[3] {
                values.extend(self.bundles().map(key));
            } else if hash == EXCLUDED {
                values.extend(EXCLUDE.into_iter().map(key));
            }
            if values != original {
                set_array_with_flags(&mut unit, hash, PropType::Key, values, 0x809C);
            }
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
        let mut flags = array(&unit, FLAGS)?.to_vec();
        if !flags.contains(&Value::Int32(23)) {
            flags.push(Value::Int32(23));
            set_array(&mut unit, FLAGS, PropType::Int32, flags);
        }
        self.verify(&unit)?;
        encode(&unit)
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        let catalog = super::behavior_conflicts::catalog()?;
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
            if count != 1 || contains(unit, &[EXCLUDED], id) || !catalog.has_bundle(id) {
                return Err(format!("HOTEL_BUNDLE: {id:08X}"));
            }
        }
        for id in EXCLUDE {
            if contains(unit, &ENABLE, id) || !contains(unit, &[EXCLUDED], id) {
                return Err(format!("HOTEL_BUNDLE: {id:08X}"));
            }
        }
        for id in INTAKES.into_iter().filter(|&id| id != INTAKES[self.tier()]) {
            if contains(unit, &ENABLE, id) {
                return Err(format!("HOTEL_BUNDLE: {id:08X}"));
            }
        }
        for (hash, kind, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.prop_type == kind && prop.scalar() == Some(&value))
            {
                return Err(format!("HOTEL_FIELD: {hash:08X}"));
            }
        }
        if !array(unit, FLAGS)?.contains(&Value::Int32(23)) {
            return Err("HOTEL_FIELD: 0A88D179".into());
        }
        Ok(())
    }

    pub(super) fn verify_lot(
        &self,
        resources: &super::connector::Resources,
        id: &dbpf::ResourceId,
    ) -> Result<(), String> {
        let bytes = resources.get(id).ok_or("HOTEL_ROAD_ACCESS")?;
        let lot = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        if !super::behavior_tourism::has_visitor_access(
            array(&lot, 0x0CC8_FE7A)?,
            array(&lot, 0x0D9A_A2BB)?,
        ) {
            return Err("HOTEL_ROAD_ACCESS".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wealth_selects_one_native_intake_and_keeps_legacy_default() {
        for (wealth, expected) in [(None, 68), (Some(1), 76), (Some(2), 68), (Some(3), 60)] {
            let recipe = select(&[json!({"type":"hotel","params":{"wealth":wealth}})])
                .unwrap().unwrap();
            let bytes = recipe.patch(&encode(&PropertyFile::default()).unwrap()).unwrap();
            let unit = PropertyFile::parse(&bytes).unwrap();
            for (index, intake) in INTAKES.into_iter().enumerate() {
                assert_eq!(contains(&unit, &ENABLE, intake), index + 1 == wealth.unwrap_or(2));
            }
            assert_eq!(recipe.rooms, expected);
            assert_eq!(recipe.needs_tiered_panel(), matches!(wealth, Some(1 | 3)));
        }
        assert_eq!(select(&[json!({"type":"hotel","params":{"wealth":4}})]).unwrap_err(), "HOTEL_WEALTH");
    }

    #[test]
    fn standalone_hotel_replaces_competing_rules_and_roundtrips_rooms() {
        let program = super::super::behavior_nodes::compile(
            None,
            &[json!({
                "type":"hotel", "id":"rooms", "params":{"rooms":84},
            })],
        )
        .unwrap();
        let recipe = program.hotel.unwrap();
        let mut unit = PropertyFile::default();
        set_array(
            &mut unit,
            ENABLE[0],
            PropType::Key,
            vec![key(INTAKE), key(0x75D2_2FD1), key(0x3A82_63E2), key(RCI_PROFIT_CYCLE)],
        );
        let bytes = recipe.patch(&encode(&unit).unwrap()).unwrap();
        let unit = PropertyFile::parse(&bytes).unwrap();
        assert_eq!(unit.get(ROOMS).unwrap().scalar(), Some(&Value::Float(84.0)));
        assert!(contains(&unit, &[EXCLUDED], 0x75D2_2FD1));
        assert!(contains(&unit, &[EXCLUDED], 0x3A82_63E2));
        assert!(contains(&unit, &[EXCLUDED], RCI_PROFIT_CYCLE));
        assert!(!contains(&unit, &ENABLE, RCI_PROFIT_CYCLE));
        assert!(contains(&unit, &ENABLE, INTAKE));
        assert_eq!(recipe.patch(&bytes).unwrap(), bytes);
        assert!(select(&[json!({"type":"hotel","params":{"rooms":1001}})]).is_err());
    }
}
