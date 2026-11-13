//! Ordinary-vehicle entrances on authored ground lots.
//! Sink rows come from the native Unit bundles. The paired Lot columns are
//! read by cTransportTraffic::AddSinkPositionsFromDataPaths.

use super::behavior_templates::{ENABLE, EXCLUDED, array, key};
use super::property_edit::{encode, set_array_with_flags, set_scalar};
use super::scene::SceneDocument;
use sc_properties::{PropType, PropertyFile, Value};

const PATH: u32 = 0x0310_6C2E;
const PATH_ENTRIES: u32 = 0x0CC8_FE7A;
const SINK_IDS: u32 = 0x0CFF_E03D;
const SINK_PATHS: u32 = 0x0CFE_C186;
const DIRECTIONS: u32 = 0x0D1A_7636;
const OUTBOUND_ONLY: u32 = 26822469;
const INITIAL_FLAGS: u32 = 0x0A88_D179;
const CAR_RESTRICTION: Value = Value::Int32(49);
const PARKING_PERCENT_FILL: u32 = 0x0C12_EF60;

fn has_parking(lot: &PropertyFile) -> Result<bool, String> {
    Ok(array(lot, sc_properties::lot_unit::PROP_ID_BASE)?
        .iter()
        .filter_map(instance)
        .any(|id| (0x0AB4_DFE1..=0x0AB4_DFE3).contains(&id)))
}

// These are the car handler rows of the native bundles, limited to arrivals.
// Ground commute rows require control bit 49 to be clear.
const COMMUTE: &[(u32, &[u32])] = &[
    (
        0x3064_5625,
        &[
            0xDB14_4779,
            0x26F1_4EA8,
            0x4AB3_780B,
            0x1BB6_F93B,
            0x863E_354E,
            0xC253_84A9,
        ],
    ),
    (
        0xD07C_5A04,
        &[
            0xADAF_4C17,
            0x9090_F61A,
            0x8133_3365,
            0x1BB6_F93B,
            0x863E_354E,
            0xC253_84A9,
        ],
    ),
    (0x1E16_EDA9, &[0xADAF_4C17, 0x9090_F61A, 0x8133_3365]),
    (0x3A70_FAA2, &[0x1BB6_F93B, 0x863E_354E, 0xC253_84A9]),
];
const OTHER_ARRIVALS: &[(u32, &[u32])] = &[
    (
        0xAB9A_0A3E,
        &[
            0x487E_1CC6,
            0x9B6F_B379,
            0xF880_AFAB,
            0xE0BC_C3B7,
            0x5770_FEDD,
            0x5B18_518D,
        ],
    ),
    (0x75D2_2FD0, &[0x75D2_2FD0]),
    (0x75D2_2FD1, &[0x75D2_2FD1]),
    (0x75D2_2FD2, &[0x75D2_2FD2]),
    (0x6C04_2366, &[0xD10F_7B4E, 0x6688_3F3B, 0x8365_DE58]),
    (0x3A82_63E2, &[0x8A97_E8AF, 0xAD48_46DB]),
];

pub(super) fn authored(document: Option<&SceneDocument>) -> bool {
    document.is_some_and(|scene| {
        scene
            .paths
            .iter()
            .any(|path| path.kind.as_deref() == Some("vehicle"))
    })
}

fn receivers(unit: &PropertyFile) -> Result<(Vec<Value>, bool), String> {
    let mut active = Vec::new();
    for hash in ENABLE {
        active.extend(array(unit, hash)?.iter().filter_map(instance));
    }
    let excluded = array(unit, EXCLUDED)?;
    active.retain(|id| {
        !excluded
            .iter()
            .filter_map(instance)
            .any(|excluded| excluded == *id)
    });
    let mut sinks = Vec::new();
    let collect = |sinks: &mut Vec<Value>, rows: &[(u32, &[u32])]| {
        for &(bundle, ids) in rows {
            if active.contains(&bundle) {
                for &id in ids {
                    let value = key(id);
                    if !sinks.contains(&value) {
                        sinks.push(value);
                    }
                }
            }
        }
    };
    collect(&mut sinks, OTHER_ARRIVALS);
    let open_commute = sinks.is_empty() || active.contains(&0x1E16_EDA9);
    if open_commute {
        collect(&mut sinks, COMMUTE);
    }
    if sinks.is_empty() {
        return Err(
            "VEHICLE_PATH_RECEIVER: no supported ordinary-vehicle arrival in the active recipe"
                .into(),
        );
    }
    Ok((sinks, open_commute))
}

fn instance(value: &Value) -> Option<u32> {
    match value {
        Value::Key(key) => Some(key.instance),
        _ => None,
    }
}

pub(super) fn bind(unit_bytes: &[u8], lot_bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut unit = PropertyFile::parse(unit_bytes).map_err(|error| error.to_string())?;
    let mut lot = PropertyFile::parse(lot_bytes).map_err(|error| error.to_string())?;
    let (sinks, open_commute) = receivers(&unit)?;
    if open_commute {
        let mut flags = array(&unit, INITIAL_FLAGS)?.to_vec();
        if flags.contains(&CAR_RESTRICTION) {
            flags.retain(|flag| flag != &CAR_RESTRICTION);
            if flags.is_empty() {
                unit.values
                    .retain(|property| property.hash != INITIAL_FLAGS);
            } else {
                set_array_with_flags(&mut unit, INITIAL_FLAGS, PropType::Int32, flags, 0x809C);
            }
        }
        if has_parking(&lot)? {
            // Ground workplaces have uncapped CarW bins. Follow the native
            // ground lots and prop_contract.py by drawing the resource count.
            set_scalar(
                &mut lot,
                PARKING_PERCENT_FILL,
                PropType::Bool,
                Value::Bool(false),
            );
        }
    }

    let mut ids = array(&lot, SINK_IDS)?.to_vec();
    let mut paths = array(&lot, SINK_PATHS)?.to_vec();
    let mut directions = array(&lot, DIRECTIONS)?.to_vec();
    if ids.len() != paths.len() || (!directions.is_empty() && directions.len() != ids.len()) {
        return Err(
            "VEHICLE_PATH_BINDING: existing sink, path and direction columns differ in length"
                .into(),
        );
    }
    for sink in sinks {
        if let Some(index) = ids
            .iter()
            .position(|value| instance(value) == instance(&sink))
        {
            paths[index] = key(PATH);
            if !directions.is_empty() {
                directions[index] = key(0);
            }
        } else {
            ids.push(sink);
            paths.push(key(PATH));
            if !directions.is_empty() {
                directions.push(key(0));
            }
        }
    }
    set_array_with_flags(&mut lot, SINK_IDS, PropType::Key, ids, 0x809C);
    set_array_with_flags(&mut lot, SINK_PATHS, PropType::Key, paths, 0x809C);
    if !directions.is_empty() {
        set_array_with_flags(&mut lot, DIRECTIONS, PropType::Key, directions, 0x809C);
    }
    Ok((encode(&unit)?, encode(&lot)?))
}

pub(super) fn verify(unit: &[u8], lot: &[u8]) -> Result<(), String> {
    let unit = PropertyFile::parse(unit).map_err(|error| error.to_string())?;
    let lot = PropertyFile::parse(lot).map_err(|error| error.to_string())?;
    let (sinks, open_commute) = receivers(&unit)?;
    let ids = array(&lot, SINK_IDS)?;
    let paths = array(&lot, SINK_PATHS)?;
    let directions = array(&lot, DIRECTIONS)?;
    if (open_commute && array(&unit, INITIAL_FLAGS)?.contains(&CAR_RESTRICTION))
        || (open_commute
            && has_parking(&lot)?
            && lot.get(PARKING_PERCENT_FILL).and_then(|prop| prop.scalar())
                != Some(&Value::Bool(false)))
        || !array(&lot, PATH_ENTRIES)?
            .iter()
            .any(|value| instance(value) == Some(PATH))
        || ids.len() != paths.len()
        || (!directions.is_empty() && directions.len() != ids.len())
        || sinks.iter().any(|sink| {
            !ids.iter().zip(paths).enumerate().any(|(row, (id, path))| {
                instance(id) == instance(sink)
                    && instance(path) == Some(PATH)
                    && directions.get(row).and_then(instance) != Some(OUTBOUND_ONLY)
            })
        })
    {
        return Err("VEHICLE_PATH_BINDING: final Lot or Unit is missing the ordinary-vehicle arrival wiring".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_entrance_requires_active_receivers_and_clears_only_flag_49() {
        let mut unit = PropertyFile::default();
        set_array_with_flags(
            &mut unit,
            ENABLE[3],
            PropType::Key,
            vec![key(0x1E16_EDA9)],
            0x809C,
        );
        set_array_with_flags(
            &mut unit,
            INITIAL_FLAGS,
            PropType::Int32,
            vec![Value::Int32(14), CAR_RESTRICTION, Value::Int32(23)],
            0x809C,
        );
        let mut lot = PropertyFile::default();
        set_array_with_flags(
            &mut lot,
            PATH_ENTRIES,
            PropType::Key,
            vec![key(PATH)],
            0x809C,
        );
        let source = encode(&unit).unwrap();
        set_array_with_flags(
            &mut lot,
            sc_properties::lot_unit::PROP_ID_BASE,
            PropType::Key,
            vec![key(0x0AB4_DFE1)],
            0x809C,
        );
        set_scalar(
            &mut lot,
            PARKING_PERCENT_FILL,
            PropType::Bool,
            Value::Bool(true),
        );
        let lot = encode(&lot).unwrap();
        assert!(verify(&source, &lot).is_err());
        let (bound_unit, bound_lot) = bind(&source, &lot).unwrap();
        verify(&bound_unit, &bound_lot).unwrap();
        let mut bad_lot = PropertyFile::parse(&bound_lot).unwrap();
        set_scalar(
            &mut bad_lot,
            PARKING_PERCENT_FILL,
            PropType::Bool,
            Value::Bool(true),
        );
        assert!(verify(&bound_unit, &encode(&bad_lot).unwrap()).is_err());
        assert_eq!(
            array(&PropertyFile::parse(&bound_unit).unwrap(), INITIAL_FLAGS).unwrap(),
            &[Value::Int32(14), Value::Int32(23)]
        );
        set_array_with_flags(
            &mut unit,
            EXCLUDED,
            PropType::Key,
            vec![key(0x1E16_EDA9)],
            0x809C,
        );
        assert!(
            bind(&encode(&unit).unwrap(), &lot)
                .unwrap_err()
                .contains("VEHICLE_PATH_RECEIVER")
        );
    }
}
