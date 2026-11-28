//! EP1 ControlNet facility, using the operating rule providers of Unit 61EFC000:74AFA83E.
use super::{
    behavior_templates::{ENABLE, EXCLUDED, array, contains, key},
    property_edit::{encode, set_array_with_flags, set_scalar},
};
use dbpf::{Package, ResourceId};
use sc_properties::{Key, PropType, PropertyFile, Value};
use serde::Serialize;
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) const VERSION: &str = "openscp.property.m1/controlnet-station-1";
const PROPERTY: u32 = 0x00B1_B104;
const GROUP: u32 = 0x61EF_C700;
const BUNDLES: [(u32, &str); 6] = [
    (
        0xE678_CDF8,
        "cf02facb375982e9ce0d7bc81bdc4a40895a79a40a4ceceb23c3e939d178c772",
    ),
    (
        0x8936_DD32,
        "c59d13dcdfb415949c69ab5f16b39c253be804fa62fc97557a8f7187b4f4a8b7",
    ),
    (
        0x27DF_B0D6,
        "1101b5d17616b988333fd7d436882557f8ded76f301e750db3125780ec6efa83",
    ),
    (
        0x1CBF_8227,
        "c4725929763c23c3c3ba8e23604853d126badaaa5ce68c21b3ee1b341d9b3e70",
    ),
    (
        0x19B0_72F3,
        "32ef07f7c4a1f0246563f0b1d3d5c088bc1b79947d388077330128ade9d2c9ab",
    ),
    (
        0xA0E0_3A4E,
        "9a7b435089e0fb4387ba2383edf1b8325bbfc3bd46e0d6f095c95f03713e71d1",
    ),
];
const EXCLUDE: [u32; 7] = [
    0x1B94_6073,
    0x3064_5625,
    0xD07C_5A04,
    0x1E16_EDA9,
    0x6827_D125,
    0x6827_D126,
    0x6827_D127,
];
const MODE: u32 = 0x0DB6_53A1;
const FLAGS: u32 = 0x0A88_D179;
const SUPPLY: ResourceId = ResourceId {
    type_id: PROPERTY,
    group: 0x61EF_C500,
    instance: 0x26A2_E32F,
};
const AEB: ResourceId = ResourceId {
    type_id: 0x0806_8AEB,
    group: 0x4080_0201,
    instance: 0x05AF_D67D,
};
const AEB_SHA256: &str = "227badfbb567af3213fdb6b035f26f32d8632eabbec2b4fff46b184f38661df0";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct ControlNet {
    jobs: [u16; 3],
    weights: [u16; 3],
}

pub(super) fn requested(nodes: &[Json]) -> bool {
    nodes.iter().any(|node| node["type"] == "controlnet")
}

pub(super) fn select(nodes: &[Json]) -> Result<Option<ControlNet>, String> {
    let Some(node) = nodes.iter().find(|node| node["type"] == "controlnet") else {
        return Ok(None);
    };
    if nodes.iter().any(|node| {
        !node["unit"].is_null()
            || !matches!(
                node["type"].as_str(),
                Some("controlnet" | "tree" | "landvalue" | "utilities-core")
            )
    }) || nodes
        .iter()
        .filter(|node| node["type"] == "controlnet")
        .count()
        != 1
    {
        return Err("CONTROLNET_COMPOSITION".into());
    }
    let params = &node["params"];
    let read = |names: [&str; 3], defaults: [u16; 3], maximum: u64| -> Result<[u16; 3], String> {
        let mut values = defaults;
        for (index, name) in names.into_iter().enumerate() {
            if !params[name].is_null() {
                values[index] = params[name]
                    .as_u64()
                    .filter(|&value| value <= maximum)
                    .ok_or_else(|| format!("CONTROLNET_PARAMETER: {name}"))?
                    as u16;
            }
        }
        Ok(values)
    };
    let jobs = read(["jobsW1", "jobsW2", "jobsW3"], [25, 50, 100], 10_000)?;
    let weights = read(["weightW1", "weightW2", "weightW3"], [1, 2, 4], 100)?;
    if !jobs
        .into_iter()
        .zip(weights)
        .any(|(jobs, weight)| jobs > 0 && weight > 0)
    {
        return Err("CONTROLNET_PARAMETER: production".into());
    }
    Ok(Some(ControlNet { jobs, weights }))
}

fn bundle(id: u32) -> ResourceId {
    ResourceId {
        type_id: PROPERTY,
        group: GROUP,
        instance: id,
    }
}

pub(crate) fn verify_installation(game_data: &Path) -> Result<(), String> {
    let paths = crate::game_assets::package_paths(game_data);
    let version = |path: &Path, prefix: &str| -> Option<u32> {
        path.file_stem()?
            .to_str()?
            .strip_prefix(prefix)?
            .parse()
            .ok()
    };
    let current = paths
        .iter()
        .filter_map(|path| version(path, "SimCity-Scripts_"))
        .max()
        .ok_or("CONTROLNET_EP1: base simulation version")?;
    let mut candidates = paths
        .iter()
        .filter(|path| version(path, "SimCityDLCEP1-Scripts_") == Some(current));
    let Some(path) = candidates.next().filter(|_| candidates.next().is_none()) else {
        return Err(format!("CONTROLNET_EP1: simulation version {current}"));
    };
    let package = Package::open(path).map_err(|error| format!("CONTROLNET_EP1: {error}"))?;
    let matches = |id: ResourceId, expected: &str| {
        let Some(entry) = package.entry(id) else {
            return false;
        };
        package
            .read(entry)
            .is_ok_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == expected)
    };
    for (id, sha) in BUNDLES {
        if !matches(bundle(id), sha) {
            return Err(format!("CONTROLNET_EP1: {}", bundle(id)));
        }
    }
    if !matches(
        SUPPLY,
        "c11e547f418ff472b1d92b8a16cde6a6fc0bb2c2460161d660593cc1c2dd4d7c",
    ) {
        return Err(format!("CONTROLNET_EP1: {SUPPLY}"));
    }
    if !matches(AEB, AEB_SHA256) {
        return Err(format!("CONTROLNET_EP1: {AEB}"));
    }
    Ok(())
}

fn bundle_key(instance: u32) -> Value {
    Value::Key(Key {
        type_id: PROPERTY,
        group: GROUP,
        instance,
    })
}

pub(crate) fn verify_target(
    artifact: &Path,
    game_data: &Path,
) -> Result<(), crate::activity::CommandError> {
    use crate::activity::CommandError;
    let invalid = |error: String| CommandError::new("deploy_invalid_artifact", error);
    let package = Package::open(artifact).map_err(|error| invalid(error.to_string()))?;
    for entry in package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == PROPERTY && entry.id.group == super::menu::UNIT_GROUP)
    {
        let raw = package
            .read(entry)
            .map_err(|error| invalid(error.to_string()))?;
        let Ok(unit) = PropertyFile::parse(&raw) else {
            continue;
        };
        if ENABLE
            .into_iter()
            .flat_map(|row| {
                unit.get(row)
                    .and_then(|prop| prop.array())
                    .unwrap_or_default()
            })
            .any(|value| value == &bundle_key(0x8936_DD32))
        {
            return verify_installation(game_data)
                .map_err(|error| CommandError::new("deploy_controlnet_ep1", error));
        }
    }
    Ok(())
}

impl ControlNet {
    fn fields(&self) -> impl Iterator<Item = (u32, PropType, Value)> {
        let jobs = self.jobs.map(f32::from);
        let weights = self.weights.map(f32::from);
        let total: f32 = jobs.iter().sum();
        let potential: f32 = jobs
            .into_iter()
            .zip(weights)
            .map(|(jobs, weight)| jobs * weight * 10.0)
            .sum();
        [
            (0x0A79_17C3, total),
            (0x0A79_17CB, total),
            (0x0A79_17C8, 1.0),
            (0x0A79_17CD, 1.0),
            (0x0FD1_6BE4, 6.0),
            (0x0CFD_EA11, 4.0),
            (0xAA5F_E358, potential),
        ]
        .into_iter()
        .chain(
            [0x0A79_17C5, 0x0A79_17C6, 0x0A79_17C7]
                .into_iter()
                .zip(jobs),
        )
        .chain(
            [0x0E3F_65A9, 0x0E3F_65B3, 0x0E3F_65B4]
                .into_iter()
                .zip(jobs),
        )
        .chain(
            [0x0FF5_64C9, 0x0FF5_64CB, 0x0FF5_64CD]
                .into_iter()
                .zip(weights),
        )
        .chain(
            [0x0FEE_63D4, 0x0FEE_63EC, 0x0FEE_63F2]
                .into_iter()
                .zip(weights),
        )
        .map(|(hash, value)| (hash, PropType::Float, Value::Float(value)))
        .chain([
            (MODE, PropType::Key, key(0xFAE8_5276)),
            (0x0E3F_606A, PropType::Key, key(0x657D_0D3B)),
            (0x0E3F_6070, PropType::Key, key(0x657D_0D38)),
            (0x0E3F_6073, PropType::Key, key(0x657D_0D39)),
        ])
    }

    pub(super) fn patch(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        for row in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(&unit, row)?;
            let mut values = original.to_vec();
            values.retain(|value| !matches!(value, Value::Key(key)
                if EXCLUDE.contains(&key.instance) || BUNDLES.iter().any(|(id, _)| *id == key.instance)));
            if row == ENABLE[3] {
                values.extend(BUNDLES.map(|(id, _)| bundle_key(id)));
            }
            if row == EXCLUDED {
                values.extend(EXCLUDE.map(key));
            }
            if values != original {
                set_array_with_flags(&mut unit, row, PropType::Key, values, 0x809C);
            }
        }
        for (hash, prop_type, value) in self.fields() {
            set_scalar(&mut unit, hash, prop_type, value);
        }
        set_array_with_flags(
            &mut unit,
            FLAGS,
            PropType::Int32,
            vec![Value::Int32(14)],
            0x809C,
        );
        self.verify(&unit)?;
        encode(&unit)
    }

    pub(super) fn verify(&self, unit: &PropertyFile) -> Result<(), String> {
        for (id, _) in BUNDLES {
            let count = ENABLE.iter().try_fold(0, |count, &row| {
                array(unit, row).map(|values| {
                    count
                        + values
                            .iter()
                            .filter(|value| matches!(value, Value::Key(key) if key.instance == id))
                            .count()
                })
            })?;
            if count != 1
                || contains(unit, &[EXCLUDED], id)
                || !array(unit, ENABLE[3])?.contains(&bundle_key(id))
            {
                return Err(format!("CONTROLNET_BUNDLE: {id:08X}"));
            }
        }
        for id in EXCLUDE {
            if contains(unit, &ENABLE, id) || !contains(unit, &[EXCLUDED], id) {
                return Err(format!("CONTROLNET_BUNDLE: {id:08X}"));
            }
        }
        for (hash, kind, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.prop_type == kind && prop.scalar() == Some(&value))
            {
                return Err(format!("CONTROLNET_FIELD: {hash:08X}"));
            }
        }
        if array(unit, FLAGS)? != [Value::Int32(14)] {
            return Err("CONTROLNET_FIELD: 0A88D179".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn station_patches_only_providers_and_keeps_authored_utility_bundle() {
        let node = json!({"type":"controlnet","params":{"jobsW3":5,"weightW3":3}});
        let recipe = select(std::slice::from_ref(&node)).unwrap().unwrap();
        assert!(select(&[node.clone(), json!({"type":"office"})]).is_err());
        assert!(select(&[node, json!({"type":"tree"})]).is_ok());
        let mut unit = PropertyFile::default();
        set_array_with_flags(
            &mut unit,
            ENABLE[2],
            PropType::Key,
            vec![key(0x3064_5625), key(0x825B_4CA4)],
            0x809C,
        );
        let bytes = recipe.patch(&encode(&unit).unwrap()).unwrap();
        let unit = PropertyFile::parse(&bytes).unwrap();
        recipe.verify(&unit).unwrap();
        assert!(contains(&unit, &ENABLE, 0x825B_4CA4));
        assert!(!contains(&unit, &ENABLE, 0x3064_5625));
        assert_eq!(
            unit.get(0xAA5F_E358).unwrap().scalar(),
            Some(&Value::Float(1400.0))
        );
        assert_eq!(recipe.patch(&bytes).unwrap(), bytes);
    }

    #[test]
    fn deployment_requires_ep1_only_for_a_package_with_the_station_provider() {
        let dir = crate::test_scratch::scratch_base()
            .join(format!("controlnet-deploy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let artifact = dir.join("asset.package");
        let game = dir.join("SimCityData");
        let unit = ResourceId {
            type_id: PROPERTY,
            group: super::super::menu::UNIT_GROUP,
            instance: 1,
        };
        let write = |bytes| {
            let package =
                dbpf::write_uncompressed_overlay(&[dbpf::OverlayEntry::new(unit, bytes)]).unwrap();
            std::fs::write(&artifact, package).unwrap();
        };
        let bare = encode(&PropertyFile::default()).unwrap();
        write(bare.clone());
        verify_target(&artifact, &game).unwrap();
        let station = select(&[json!({"type":"controlnet"})]).unwrap().unwrap();
        write(station.patch(&bare).unwrap());
        let error = verify_target(&artifact, &game).unwrap_err();
        assert_eq!(error.code, "deploy_controlnet_ep1");
        assert!(error.message.contains("CONTROLNET_EP1"));
        std::fs::remove_file(&artifact).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
