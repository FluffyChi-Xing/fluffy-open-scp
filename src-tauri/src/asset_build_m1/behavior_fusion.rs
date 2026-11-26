//! Standalone EP1 fusion plant, based on original MegaTower floor AF98CA04.
use super::{
    behavior_templates::{ENABLE, EXCLUDED, array, contains, key},
    connector::Resources,
    property_edit::{encode, set_array_with_flags, set_scalar},
};
use dbpf::{Package, ResourceId};
use sc_properties::{Key, PropType, PropertyFile, Value};
use serde::Serialize;
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) const VERSION: &str = "openscp.property.m1/fusion-power-1";
const PROPERTY: u32 = 0x00B1_B104;
const CITY: u32 = 0x40E0_C700;
const EP1: u32 = 0x61EF_C700;
const PRODUCER: ResourceId = bundle(EP1, 0x6D26_CFD5);
const TRANSMIT: ResourceId = bundle(CITY, 0x4106_B159);
const MARKER: ResourceId = bundle(EP1, 0xE678_CDF8);
const CONSUMER: ResourceId = bundle(EP1, 0x26A2_E32F);
const ORDINARY_POWER: u32 = 0x4893_9D96;
const SKILLED: u32 = 0x5386_FBCF;
const PRODUCTION: u32 = 0x0AFB_9881;
const CHARGE: u32 = 0x0C09_DAA0;
const NETWORK_FIELDS: [(u32, f32); 4] = [
    (0x0FD1_6C0B, 3750.0),
    (0x0FD1_6C0F, 60.0),
    (0x0FD1_6C12, 3750.0),
    (0x0FD1_6C15, 1250.0),
];

const fn bundle(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: PROPERTY,
        group,
        instance,
    }
}

fn qualified(id: ResourceId) -> Value {
    Value::Key(Key {
        type_id: id.type_id,
        group: id.group,
        instance: id.instance,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FusionPower {
    pub(super) requires_control_net: bool,
    charge_capacity: u32,
    generation_amount: u32,
}

pub(crate) fn requested(nodes: &[Json]) -> bool {
    nodes.iter().any(|node| node["type"] == "fusion-power")
}

pub(super) fn select(nodes: &[Json]) -> Result<Option<FusionPower>, String> {
    let Some(node) = nodes.iter().find(|node| node["type"] == "fusion-power") else {
        return Ok(None);
    };
    if super::behavior_units::scoped(nodes)
        || nodes
            .iter()
            .filter(|node| node["type"] != "utilities-core")
            .count()
            != 1
    {
        return Err("FUSION_COMPOSITION".into());
    }
    let params = &node["params"];
    let quantity = |field: &str, default| match &params[field] {
        Json::Null => Ok(default),
        value => value
            .as_u64()
            .filter(|n| (1..=10_000_000).contains(n))
            .map(|n| n as u32)
            .ok_or_else(|| format!("FUSION_PARAMETER: {field}")),
    };
    let requires_control_net = match &params["requiresControlNet"] {
        Json::Null => true,
        Json::Bool(enabled) => *enabled,
        _ => return Err("FUSION_PARAMETER: requiresControlNet".into()),
    };
    Ok(Some(FusionPower {
        requires_control_net,
        charge_capacity: quantity("chargeCapacity", 400_000)?,
        generation_amount: quantity("generationAmount", 13_334)?,
    }))
}

pub(crate) fn verify_installation(
    game_data: &Path,
    requires_control_net: bool,
) -> Result<(), String> {
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
        .filter_map(|p| version(p, "SimCity-Scripts_"))
        .max()
        .ok_or("FUSION_EP1: base simulation version")?;
    let mut candidates = paths
        .iter()
        .filter(|p| version(p, "SimCityDLCEP1-Scripts_") == Some(current));
    let path = candidates
        .next()
        .filter(|_| candidates.next().is_none())
        .ok_or_else(|| format!("FUSION_EP1: simulation version {current}"))?;
    let ep1 = Package::open(path).map_err(|error| format!("FUSION_EP1: {error}"))?;
    let mut providers = vec![
        (
            ResourceId {
                type_id: 0x0806_8AEB,
                group: 0x4080_0201,
                instance: 0x05AF_D67D,
            },
            "227badfbb567af3213fdb6b035f26f32d8632eabbec2b4fff46b184f38661df0",
        ),
        (
            PRODUCER,
            "ebed08cd9230199823d701f3660393c351ec56d9fefd1428a14f8e0b9feb7d26",
        ),
        (
            MARKER,
            "cf02facb375982e9ce0d7bc81bdc4a40895a79a40a4ceceb23c3e939d178c772",
        ),
    ];
    if requires_control_net {
        providers.extend([
            (
                CONSUMER,
                "2df884e97e8e1d636546d0d146a833b18739ac27b1c7ede744cc9a8471cb9b30",
            ),
            (
                bundle(0x61EF_C500, 0x26A2_E32F),
                "c11e547f418ff472b1d92b8a16cde6a6fc0bb2c2460161d660593cc1c2dd4d7c",
            ),
            (
                bundle(0x61EF_C500, 0xEAE6_60BE),
                "b6ddd7a4d55a96c29c68c9f1eacb3fb691cd60317fb2aec99a29fd8e5c202a57",
            ),
        ]);
    }
    for (id, hash) in providers {
        verify_provider(&ep1, id, hash)?;
    }
    let base_path = paths
        .iter()
        .find(|p| version(p, "SimCity-Scripts_") == Some(current))
        .ok_or("FUSION_EP1: base simulation package")?;
    let base = Package::open(base_path).map_err(|error| format!("FUSION_EP1: {error}"))?;
    let provider = if ep1.entry(TRANSMIT).is_some() {
        &ep1
    } else {
        &base
    };
    verify_provider(
        provider,
        TRANSMIT,
        "b04a7f8b32019bfcc0e9eabd54c7ec2b27aca482e2511f9bbc3b87465a1bbe30",
    )
}

pub(crate) fn verify_target(
    artifact: &Path,
    game_data: &Path,
) -> Result<(), crate::activity::CommandError> {
    use crate::activity::CommandError;
    let invalid =
        |error: dbpf::Error| CommandError::new("deploy_invalid_artifact", error.to_string());
    let package = Package::open(artifact).map_err(invalid)?;
    for entry in package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == PROPERTY && entry.id.group == super::menu::UNIT_GROUP)
    {
        let raw = package.read(entry).map_err(invalid)?;
        let Ok(unit) = PropertyFile::parse(&raw) else {
            continue;
        };
        if has_power_provider(&unit) {
            return verify_installation(game_data, provider_count(&unit, CONSUMER) == 1)
                .map_err(|error| CommandError::new("deploy_fusion_ep1", error));
        }
    }
    Ok(())
}

fn verify_provider(package: &Package, id: ResourceId, hash: &str) -> Result<(), String> {
    if package
        .entry(id)
        .and_then(|entry| package.read(entry).ok())
        .is_some_and(|bytes| format!("{:x}", Sha256::digest(bytes)) == hash)
    {
        return Ok(());
    }
    Err(format!("FUSION_EP1: {id}"))
}

pub(super) fn readiness_id(host: u32) -> ResourceId {
    bundle(
        CITY,
        super::behavior_unit_build::instance(host, "fusion-local-readiness-v1"),
    )
}

fn readiness() -> Result<Vec<u8>, String> {
    let mut file = PropertyFile::default();
    set_array_with_flags(
        &mut file,
        0x07E0_AD23,
        PropType::Key,
        vec![key(SKILLED)],
        0x809C,
    );
    for hash in [0x07E0_AD24, 0x0863_5E12] {
        set_array_with_flags(
            &mut file,
            hash,
            PropType::Int32,
            vec![Value::Int32(1)],
            0x809C,
        );
    }
    encode(&file)
}

pub(super) fn has_power_provider(unit: &PropertyFile) -> bool {
    [PRODUCER, TRANSMIT, MARKER]
        .into_iter()
        .all(|id| provider_count(unit, id) == 1 && !contains(unit, &[EXCLUDED], id.instance))
        && contains(unit, &[EXCLUDED], ORDINARY_POWER)
        && !contains(unit, &ENABLE, ORDINARY_POWER)
}

fn provider_count(unit: &PropertyFile, id: ResourceId) -> usize {
    ENABLE
        .iter()
        .filter_map(|&row| array(unit, row).ok())
        .flatten()
        .filter(|value| **value == qualified(id))
        .count()
}

impl FusionPower {
    fn fields(&self) -> impl Iterator<Item = (u32, PropType, Value)> {
        [
            (PRODUCTION, self.generation_amount as f32),
            (CHARGE, self.charge_capacity as f32),
            (0x0A79_17C8, 0.0),
            (0x0AFB_9882, 100_000_000.0),
            (0x0AFB_9883, 2.0),
            (0x0B88_904E, 0.0),
            (0x0C09_DAA6, 0.0),
            (0x0C09_DAB2, 100_000_000.0),
        ]
        .into_iter()
        .chain(
            NETWORK_FIELDS
                .into_iter()
                .filter(|_| self.requires_control_net),
        )
        .map(|(hash, value)| (hash, PropType::Float, Value::Float(value)))
        .chain([
            (0x0C09_DAB6, PropType::Key, key(0x617D_5B25)),
            (0x0DB6_53A1, PropType::Key, key(0xFAE8_5276)),
        ])
    }

    pub(super) fn patch(
        &self,
        id: ResourceId,
        resources: &mut super::package::BuildResources,
    ) -> Result<(), String> {
        let bytes = resources.get(&id).ok_or("FUSION_RESOURCE")?;
        let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        let local = readiness_id(id.instance);
        for row in ENABLE.into_iter().chain([EXCLUDED]) {
            let original = array(&unit, row)?;
            let mut values = original.to_vec();
            values.retain(|value| {
                !matches!(value, Value::Key(key) if [
                ORDINARY_POWER, PRODUCER.instance, TRANSMIT.instance, MARKER.instance,
                CONSUMER.instance, local.instance,
            ].contains(&key.instance))
            });
            if row == ENABLE[3] {
                values.extend(
                    [
                        MARKER,
                        PRODUCER,
                        TRANSMIT,
                        if self.requires_control_net {
                            CONSUMER
                        } else {
                            local
                        },
                    ]
                    .map(qualified),
                );
            }
            if row == EXCLUDED {
                values.push(key(ORDINARY_POWER));
                if !self.requires_control_net {
                    values.push(key(CONSUMER.instance));
                }
            }
            if values != original {
                set_array_with_flags(&mut unit, row, PropType::Key, values, 0x809C);
            }
        }
        if !self.requires_control_net {
            unit.values
                .retain(|prop| !NETWORK_FIELDS.iter().any(|(hash, _)| *hash == prop.hash));
            if resources.get(&local).is_some() {
                return Err(format!("FUSION_RESOURCE: {local}"));
            }
            resources.insert(local, readiness()?);
        }
        for (hash, kind, value) in self.fields() {
            set_scalar(&mut unit, hash, kind, value);
        }
        resources.insert(id, encode(&unit)?);
        Ok(())
    }

    pub(super) fn verify(&self, resources: &Resources, id: ResourceId) -> Result<(), String> {
        let raw = resources.get(&id).ok_or("FUSION_RESOURCE")?;
        let unit = PropertyFile::parse(raw).map_err(|error| error.to_string())?;
        let local = readiness_id(id.instance);
        if !has_power_provider(&unit) {
            return Err("FUSION_BUNDLE: power".into());
        }
        let network = if self.requires_control_net {
            provider_count(&unit, CONSUMER) == 1
                && !contains(&unit, &[EXCLUDED], CONSUMER.instance)
                && provider_count(&unit, local) == 0
                && !resources.contains_key(&local)
        } else {
            !contains(&unit, &ENABLE, CONSUMER.instance)
                && contains(&unit, &[EXCLUDED], CONSUMER.instance)
                && provider_count(&unit, local) == 1
                && !contains(&unit, &[EXCLUDED], local.instance)
                && resources.get(&local) == Some(&readiness()?)
                && NETWORK_FIELDS
                    .iter()
                    .all(|(hash, _)| unit.get(*hash).is_none())
        };
        if !network {
            return Err("FUSION_BUNDLE: ControlNet".into());
        }
        for (hash, kind, value) in self.fields() {
            if !unit
                .get(hash)
                .is_some_and(|prop| prop.prop_type == kind && prop.scalar() == Some(&value))
            {
                return Err(format!("FUSION_FIELD: {hash:08X}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::package::BuildResources;
    use serde_json::json;

    #[test]
    fn standalone_modes_bind_exactly_one_readiness_source() {
        let id = bundle(super::super::menu::UNIT_GROUP, 0xF01D_1234);
        for enabled in [true, false] {
            let recipe = select(&[json!({
                "type": "fusion-power",
                "params": {
                    "requiresControlNet": enabled,
                    "chargeCapacity": 432100,
                    "generationAmount": 15555
                }
            })])
            .unwrap()
            .unwrap();
            let mut resources = BuildResources::empty();
            resources.insert(id, encode(&PropertyFile::default()).unwrap());
            recipe.patch(id, &mut resources).unwrap();
            recipe.verify(&resources, id).unwrap();
            let unit = PropertyFile::parse(resources.get(&id).unwrap()).unwrap();
            assert_eq!(provider_count(&unit, CONSUMER), usize::from(enabled));
            if !enabled {
                resources.remove(&readiness_id(id.instance));
                assert!(
                    recipe
                        .verify(&resources, id)
                        .unwrap_err()
                        .contains("FUSION_BUNDLE")
                );
            }
        }
    }

    #[test]
    fn rejects_other_simulation_scopes_and_invalid_parameters() {
        let fusion = json!({"type":"fusion-power"});
        assert!(
            select(&[fusion.clone()])
                .unwrap()
                .unwrap()
                .requires_control_net
        );
        for incompatible in [
            json!({"type":"hotel"}),
            json!({"type":"utilities-core","unit":"component"}),
            fusion.clone(),
        ] {
            assert_eq!(
                select(&[fusion.clone(), incompatible]).unwrap_err(),
                "FUSION_COMPOSITION"
            );
        }
        assert_eq!(
            select(&[json!({"type":"fusion-power","params":{"chargeCapacity":0}})]).unwrap_err(),
            "FUSION_PARAMETER: chargeCapacity"
        );
    }
}
