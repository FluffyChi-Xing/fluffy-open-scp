//! Shipped local-template baselines. Never opens player save directories.
use crate::{Key, Kind, PropertyFile, Value, region_3d, region_state};
use serde::Serialize;
use std::{collections::HashSet, io::Read};

const MAX_STATE: u64 = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct TemplateBinding {
    pub region: u32,
    pub metadata: Key,
    pub sites: Vec<(String, Key)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticLayers {
    pub sources: Vec<region_state::StateSource>,
    #[serde(skip_serializing)]
    pub regional_units: Vec<region_state::StateUnit>,
    pub template: String,
    pub cities: Vec<CityEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CityEntry {
    pub site: String,
    pub environment: String,
    pub version: u32,
}

fn resource(package: &dbpf::Package, key: &Key) -> Result<Vec<u8>, String> {
    let entry = package
        .entries()
        .iter()
        .find(|e| {
            e.id.type_id == key.type_id && e.id.group == key.group && e.id.instance == key.instance
        })
        .ok_or_else(|| {
            format!(
                "missing static resource {:08X}:{:08X}:{:08X}",
                key.type_id, key.group, key.instance
            )
        })?;
    package.read(entry).map_err(|e| e.to_string())
}

fn binding(props: &PropertyFile) -> Result<(Key, Vec<(String, Key)>), String> {
    let Some(Kind::Array(keys)) = props.get(0x46066280).map(|p| &p.kind) else {
        return Err("missing bootstrap key array".into());
    };
    let Some(Kind::Array(uids)) = props.get(0x46066281).map(|p| &p.kind) else {
        return Err("missing bootstrap UID array".into());
    };
    let Some(Kind::Scalar(Value::Key(metadata))) = props.get(0xF240A8CA).map(|p| &p.kind) else {
        return Err("missing template metadata key".into());
    };
    if keys.len() != uids.len() || keys.is_empty() {
        return Err("bootstrap key/UID count mismatch".into());
    }
    let mut seen = HashSet::new();
    let sites = keys
        .iter()
        .zip(uids)
        .map(|(key, uid)| {
            let (Value::Key(key), Value::Int32(uid)) = (key, uid) else {
                return Err("invalid bootstrap binding types".to_string());
            };
            if key.type_id != 0x08068AED || *uid < 0 || !seen.insert(*uid) {
                return Err("invalid or duplicate bootstrap site".into());
            }
            Ok((uid.to_string(), *key))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !seen.contains(&0) {
        return Err("bootstrap has no regional site 0".into());
    }
    Ok((*metadata, sites))
}

/// Read only the bounded gzip header for catalog identity, not 145 full states.
fn environment(packed: &[u8]) -> Result<u32, String> {
    let mut decoder = flate2::read::GzDecoder::new(packed);
    let mut header = [0u8; 16];
    decoder.read_exact(&mut header).map_err(|e| e.to_string())?;
    let word = |i| u32::from_be_bytes(header[i..i + 4].try_into().unwrap());
    if !matches!(word(0), 17 | 18 | 20) {
        return Err("unsupported bootstrap version".into());
    }
    let addons = word(12);
    if addons > 256 {
        return Err("invalid bootstrap addon count".into());
    }
    let mut tail = vec![0; addons as usize * 8 + 4];
    decoder.read_exact(&mut tail).map_err(|e| e.to_string())?;
    Ok(u32::from_be_bytes(
        tail[tail.len() - 4..].try_into().unwrap(),
    ))
}

pub fn catalog(ep1: &dbpf::Package) -> Result<Vec<TemplateBinding>, String> {
    let mut out = Vec::new();
    let mut regions = HashSet::new();
    for entry in ep1
        .entries()
        .iter()
        .filter(|e| e.id.group == 0x95F4AEF0 && e.id.type_id == 0x00B1B104)
    {
        let bytes = ep1.read(entry).map_err(|e| e.to_string())?;
        let props = PropertyFile::parse(&bytes).map_err(|e| e.to_string())?;
        if props.get(0x46066280).is_none() {
            continue;
        }
        let (metadata, sites) = binding(&props)?;
        let zero = sites.iter().find(|(uid, _)| uid == "0").unwrap();
        let region = environment(&resource(ep1, &zero.1)?)?;
        if !regions.insert(region) {
            return Err(format!("duplicate static region {region:08X}"));
        }
        out.push(TemplateBinding {
            region,
            metadata,
            sites,
        });
    }
    Ok(out)
}

fn state(ep1: &dbpf::Package, key: &Key) -> Result<region_state::RegionState, String> {
    let packed = resource(ep1, key)?;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&packed[..])
        .take(MAX_STATE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_STATE {
        return Err("bootstrap exceeds decompression limit".into());
    }
    region_state::read_state(&bytes)
}

/// Bind by UID AND city environment. Only site 0 owns the regional network.
pub fn load(
    terrain: &dbpf::Package,
    ep1: &dbpf::Package,
    group: u32,
) -> Result<StaticLayers, String> {
    let binding = catalog(ep1)?
        .into_iter()
        .find(|b| b.region == group)
        .ok_or_else(|| format!("no static template for {group:08X}"))?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&resource(ep1, &binding.metadata)?).map_err(|e| e.to_string())?;
    let template = metadata["name"]
        .as_str()
        .ok_or("template name missing")?
        .to_string();
    let (uids, positions, environments) =
        region_3d::plot_table(terrain, group).ok_or("city plot table missing")?;
    if uids.len() + 1 != binding.sites.len()
        || positions.len() != uids.len()
        || environments.len() != uids.len()
    {
        return Err("static city/plot count mismatch".into());
    }
    let mut sources = Vec::new();
    let mut regional_units = Vec::new();
    let mut cities = Vec::new();
    for (site, key) in binding.sites {
        let state = state(ep1, &key).map_err(|e| format!("{group:08X}/{site}: {e}"))?;
        if site == "0" {
            if state.environment != group {
                return Err("regional bootstrap identity mismatch".into());
            }
            regional_units = state.units;
            sources.push(region_state::StateSource {
                site,
                origin: [0.; 2],
                bounds: None,
                maps: vec![],
                curves: state.curves,
            });
        } else {
            let i = uids
                .iter()
                .position(|u| *u == site)
                .ok_or("unknown static city UID")?;
            if state.environment != environments[i] {
                return Err(format!("city {site} environment mismatch"));
            }
            if state.maps.iter().any(|m| m.size != 128) {
                return Err("unsupported static city dimensions".into());
            }
            let (x, y) = positions[i];
            cities.push(CityEntry {
                site: site.clone(),
                environment: format!("{:08X}", state.environment),
                version: state.version,
            });
            sources.push(region_state::StateSource {
                site,
                origin: [x, y],
                bounds: Some([x - 1024., y - 1024., x + 1024., y + 1024.]),
                maps: state
                    .maps
                    .into_iter()
                    .filter(|m| {
                        matches!(
                            m.id,
                            0xD779C976
                                | 0x76D10FF2
                                | 0xD74AF22B
                                | 0x0F92DC56
                                | 0x1D52BE3E
                                | 0x28F896A4
                        )
                    })
                    .collect(),
                curves: vec![],
            });
        }
    }
    Ok(StaticLayers {
        sources,
        regional_units,
        template,
        cities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn props(uids: Vec<i32>, key_count: usize) -> PropertyFile {
        let key = Key {
            type_id: 0x08068AED,
            group: 1,
            instance: 2,
        };
        let values = [
            (0x46066280, Kind::Array(vec![Value::Key(key); key_count])),
            (
                0x46066281,
                Kind::Array(uids.into_iter().map(Value::Int32).collect()),
            ),
            (
                0xF240A8CA,
                Kind::Scalar(Value::Key(Key {
                    type_id: 0x0A98EAF0,
                    ..key
                })),
            ),
        ]
        .into_iter()
        .map(|(hash, kind)| crate::Property {
            hash,
            kind,
            prop_type: crate::PropType::Key,
            encoding: Default::default(),
        })
        .collect();
        PropertyFile {
            values,
            claimed_count: 3,
        }
    }
    #[test]
    fn rejects_ambiguous_or_partial_site_bindings() {
        assert_eq!(
            binding(&props(vec![0, 1026, 1027], 3)).unwrap().1[2].0,
            "1027"
        );
        assert!(
            binding(&props(vec![0, 1026], 3))
                .unwrap_err()
                .contains("count mismatch")
        );
        assert!(
            binding(&props(vec![0, 1026, 1026], 3))
                .unwrap_err()
                .contains("duplicate")
        );
        assert!(
            binding(&props(vec![1026], 1))
                .unwrap_err()
                .contains("site 0")
        );
        assert!(binding(&props(vec![0, -1], 2)).is_err());
        assert!(binding(&PropertyFile::default()).is_err());
    }
    #[test]
    fn bounded_header_accepts_addons_and_all_retail_versions() {
        for version in [17, 18, 20] {
            let words = [version, 1, 1, 2, 99, 88, 77, 66, 0xB12DE348];
            let bytes: Vec<_> = words.into_iter().flat_map(u32::to_be_bytes).collect();
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            gz.write_all(&bytes).unwrap();
            assert_eq!(environment(&gz.finish().unwrap()).unwrap(), 0xB12DE348);
        }
        assert!(environment(&[0; 20]).is_err());
    }
}
