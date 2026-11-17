use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value, flatten_parent_inheritance};

use super::{property, property_id};

const LOT_MATERIAL: u32 = 0x0B76_2710;
const PATH_FILTERS: u32 = 0x0D64_98E8;
const ZONABLE: u32 = 0xA2F9_BEEB;

pub(super) fn encode(data: &[u8], scripts: &Package, enabled: bool) -> Result<Vec<u8>, String> {
    let file = PropertyFile::parse(data).map_err(|error| error.to_string())?;
    let mut missing_parent = None;
    // The live game resolved the custom layout but left its parent pointer null.
    let mut file = flatten_parent_inheritance(file, |key| {
        let group = if key.group == 0 {
            super::SIM
        } else {
            key.group
        };
        match property(scripts, property_id(group, key.instance)) {
            Ok(parent) => Some(parent),
            Err(error) => {
                missing_parent = Some(error);
                None
            }
        }
    });
    if let Some(error) = missing_parent {
        return Err(error);
    }
    configure(&mut file, enabled)?;
    file.encode_canonical().map_err(|error| error.to_string())
}

fn configure(file: &mut PropertyFile, enabled: bool) -> Result<(), String> {
    let is_zonable = |value: &Value| matches!(value, Value::Key(key) if key.instance == ZONABLE);
    if enabled {
        // cZoningGame::GetLotsFromPath exits if cNewLotData cannot read the material.
        if !matches!(file.get(LOT_MATERIAL).and_then(|field| field.scalar()),
            Some(Value::Key(key)) if key.instance != 0)
            || !file
                .get(PATH_FILTERS)
                .and_then(|field| field.array())
                .is_some_and(|filters| filters.iter().any(is_zonable))
        {
            return Err("road_frontage_donor_missing".into());
        }
    } else {
        file.values.retain(|field| field.hash != LOT_MATERIAL);
        if let Some(field) = file
            .values
            .iter_mut()
            .find(|field| field.hash == PATH_FILTERS)
        {
            let Kind::Array(filters) = &mut field.kind else {
                return Err("road_frontage_donor_missing".into());
            };
            filters.retain(|value| !is_zonable(value));
        }
    }
    Ok(())
}
