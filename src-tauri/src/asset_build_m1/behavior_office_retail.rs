//! Ground mall payroll and storefront with the medium-tech office production
//! bundle. Both businesses consume the same workers and settle on one Unit.
use super::{
    behavior_nodes::{self, LaborSpec, ResolvedBehavior},
    behavior_office::PRODUCTION,
    behavior_templates::{ENABLE, EXCLUDED, array, contains},
    property_edit::{encode, set_array_with_flags, set_scalar},
};
use sc_properties::{Key, PropType, PropertyFile, Value};

const PRODUCTION_BUNDLE: u32 = 0x6A32_6F41;
const RETAIL_PAYROLL: u32 = 0xD07C_5A04;
const OFFICE_PAYROLL: u32 = 0x1E16_EDA9;
const DAILY_INCOME: u32 = 0xDD6E_83EC;

pub(super) fn compile(
    nodes: &[serde_json::Value],
    retail_index: usize,
    labor: Option<LaborSpec>,
) -> Result<ResolvedBehavior, String> {
    let retail = &nodes[retail_index];
    let profile = retail["params"]["commercialProfile"]
        .as_str()
        .unwrap_or_default();
    if !matches!(
        profile,
        super::behavior_commercial_mixed::PROFILE | super::behavior_commercial_mall::PROFILE
    ) {
        return Err(
            "OFFICE_CONFLICT: office production requires the ordinary ground mall income recipe"
                .into(),
        );
    }
    let mut program = behavior_nodes::compile_with_profile(labor, nodes, false)?;
    program.office_and_retail = true;
    program
        .notes
        .push("Ground mall payroll and trade with native medium-tech office production".into());
    Ok(program)
}

fn production_key() -> Value {
    Value::Key(Key {
        type_id: super::behavior::PROPERTY_TYPE,
        group: 0x61EF_C700,
        instance: PRODUCTION_BUNDLE,
    })
}

pub(super) fn patch(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if let Some(data) = std::env::var_os("OPENSCP_GAME_DATA") {
        let id = dbpf::ResourceId {
            type_id: super::behavior::PROPERTY_TYPE,
            group: 0x61EF_C700,
            instance: PRODUCTION_BUNDLE,
        };
        let available = crate::game_assets::package_paths(std::path::Path::new(&data))
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("SimCityDLCEP1-Scripts_"))
            })
            .any(|path| dbpf::Package::open(path).is_ok_and(|package| package.entry(id).is_some()));
        if !available {
            return Err(format!(
                "OFFICE_BUNDLE: installed native production {id} is missing"
            ));
        }
    }
    let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
    for hash in ENABLE.into_iter().chain([EXCLUDED]) {
        let mut values = array(&unit, hash)?.to_vec();
        values
            .retain(|value| !matches!(value, Value::Key(key) if key.instance == PRODUCTION_BUNDLE));
        if hash == ENABLE[3] {
            values.push(production_key());
        }
        if values != array(&unit, hash)? {
            set_array_with_flags(&mut unit, hash, PropType::Key, values, 0x809C);
        }
    }
    for (hash, value) in PRODUCTION {
        set_scalar(&mut unit, hash, PropType::Float, Value::Float(value));
    }
    encode(&unit)
}

pub(super) fn verify(unit: &PropertyFile) -> Result<(), String> {
    if !array(unit, ENABLE[3])?.contains(&production_key())
        || contains(unit, &[EXCLUDED], PRODUCTION_BUNDLE)
        || contains(unit, &ENABLE, OFFICE_PAYROLL)
        || [RETAIL_PAYROLL, DAILY_INCOME]
            .iter()
            .any(|&bundle| !contains(unit, &ENABLE, bundle) || contains(unit, &[EXCLUDED], bundle))
    {
        return Err(
            "OFFICE_CONFLICT: shared retail payroll or native office production is incomplete"
                .into(),
        );
    }
    for (hash, value) in PRODUCTION {
        if unit.get(hash).and_then(|property| property.scalar()) != Some(&Value::Float(value)) {
            return Err(format!("OFFICE_FIELD: {hash:08X}"));
        }
    }
    Ok(())
}
