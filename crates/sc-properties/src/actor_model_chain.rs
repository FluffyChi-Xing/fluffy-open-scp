//! Exact-TGI actor model traversal, independent of the application runtime.
use crate::{actor_graphics, vignette::Reference};
use dbpf::{Package, ResourceId};
use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelKey {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}
const PROPERTY: u32 = 0x00B1B104;
const RW4: u32 = 0x2F4E681B;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub tgi: ModelKey,
    pub via: String,
    pub status: String,
    pub parts: Option<actor_graphics::Parts>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub package_id: u64,
    pub tgi: ModelKey,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub models: Vec<Model>,
    pub steps: Vec<Step>,
}
fn tgi(r: Reference) -> ModelKey {
    ModelKey {
        type_id: if r.type_id == 0 { PROPERTY } else { r.type_id },
        group: r.group_id,
        instance: r.instance_id,
    }
}
/// Bounded traversal; package identity and property group are preserved at every hop.
pub fn trace(packages: &[(u64, Arc<Package>)], roots: Vec<ModelKey>) -> Resolution {
    let mut queue: VecDeque<_> = roots
        .into_iter()
        .map(|k| (k, "reference".to_owned()))
        .collect();
    let mut seen = HashSet::new();
    let mut result = Resolution {
        models: vec![],
        steps: vec![],
    };
    while let Some((mut key, via)) = queue.pop_front() {
        if key.type_id == 0 {
            key.type_id = PROPERTY;
        }
        let id = ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        };
        if !seen.insert(id) {
            continue;
        }
        if seen.len() > 128 {
            result.steps.push(Step {
                tgi: key,
                via,
                status: "limit".into(),
                parts: None,
            });
            break;
        }
        let mut step = Step {
            tgi: key,
            via,
            status: "missing".into(),
            parts: None,
        };
        if let Some((pid, pkg, entry)) = packages
            .iter()
            .find_map(|(pid, pkg)| pkg.entry(id).map(|entry| (*pid, pkg, entry)))
        {
            if id.type_id == RW4 {
                step.status = "model".into();
                result.models.push(Model {
                    package_id: pid,
                    tgi: step.tgi.clone(),
                });
            } else if id.type_id == PROPERTY {
                step.status = "property".into();
                if entry.decompressed_size > 16 * 1024 * 1024 {
                    step.status = "limit".into();
                } else if let Ok(file) = pkg
                    .read(entry)
                    .ok()
                    .and_then(|data| crate::PropertyFile::parse(&data).ok())
                    .ok_or(())
                {
                    let parts = actor_graphics::parts(&file);
                    if parts.present() {
                        step.status = "composite".into();
                        step.parts = Some(parts);
                    }
                    let mut refs = Vec::new();
                    for (hash, via) in [
                        (0x0D897169, "vehicle-models"),
                        (0x00F9EFBB, "lod1"),
                        (0x0D8C29C3, "graphics-wrapper"),
                    ] {
                        refs.extend(
                            actor_graphics::keys(&file, hash)
                                .into_iter()
                                .map(|k| (tgi(k), via.to_owned())),
                        );
                    }
                    if refs.is_empty() {
                        refs.extend(
                            actor_graphics::keys(&file, 0x00B2CCCB)
                                .into_iter()
                                .map(|k| (tgi(k), "parent".to_owned())),
                        );
                    }
                    queue.extend(refs);
                } else {
                    step.status = "invalid".into();
                }
            } else {
                step.status = "unsupported".into();
            }
        }
        result.steps.push(step);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Kind, PropType, Property, PropertyFile, Value};
    fn prop(hash: u32, key: ResourceId) -> Property {
        Property {
            hash,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(Key {
                type_id: key.type_id,
                group: key.group,
                instance: key.instance,
            })),
            encoding: Default::default(),
        }
    }
    fn id(t: u32, g: u32, i: u32) -> ResourceId {
        ResourceId {
            type_id: t,
            group: g,
            instance: i,
        }
    }
    fn dto(k: ResourceId) -> ModelKey {
        ModelKey {
            type_id: k.type_id,
            group: k.group,
            instance: k.instance,
        }
    }
    #[test]
    fn follows_parent_and_lod_without_crossing_groups() {
        let wrapper = id(PROPERTY, 9, 1);
        let parent = id(PROPERTY, 9, 2);
        let model = id(RW4, 0, 3);
        let property = |p| {
            PropertyFile {
                claimed_count: 1,
                values: vec![p],
            }
            .encode_canonical()
            .unwrap()
        };
        let entries = vec![
            dbpf::OverlayEntry::new(wrapper, property(prop(0x00B2CCCB, parent))),
            dbpf::OverlayEntry::new(parent, property(prop(0x00F9EFBB, model))),
            dbpf::OverlayEntry::new(model, vec![]),
        ];
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("actor-chain-{}.package", std::process::id()));
        std::fs::write(&path, dbpf::write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let packages = vec![(7, Arc::new(Package::open(&path).unwrap()))];
        let found = trace(&packages, vec![dto(id(0, 9, 1))]);
        assert_eq!(found.models.len(), 1);
        assert_eq!(found.models[0].package_id, 7);
        assert_eq!(found.steps[1].via, "parent");
        assert_eq!(found.steps[2].via, "lod1");
        let missing = trace(&packages, vec![dto(id(0, 10, 1))]);
        assert!(missing.models.is_empty());
        assert_eq!(missing.steps[0].status, "missing");
        drop(packages);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires local installed EP1 assets"]
    fn real_ep1_actor_wrapper_and_template() {
        let root = std::path::Path::new(
            "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData",
        );
        let packages: Vec<_> = ["SimCity_Game.package", "SimCity_Graphics.package"]
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u64, Arc::new(Package::open(root.join(n)).unwrap())))
            .collect();
        let found = trace(
            &packages,
            vec![
                dto(id(0, 0x40E02D00, 0x66BE4E6E)),
                dto(id(PROPERTY, 0x40E02D00, 0xACCCD1D7)),
            ],
        );
        assert!(found.models.iter().any(|m| m.tgi.instance == 0x66BE4E6E));
        let model = &found.models[0];
        let package = packages
            .iter()
            .find(|(id, _)| *id == model.package_id)
            .unwrap();
        let data = package
            .1
            .read(
                package
                    .1
                    .entry(id(model.tgi.type_id, model.tgi.group, model.tgi.instance))
                    .unwrap(),
            )
            .unwrap();
        let rw4 = rw4::Rw4File::parse(&data).unwrap();
        assert!(
            rw4.sections()
                .iter()
                .filter(|s| s.type_code == 0x20009)
                .any(|s| rw4.decode_mesh(&data, s.number).is_ok())
        );
        let template = found
            .steps
            .iter()
            .find(|s| s.tgi.instance == 0xACCCD1D7)
            .unwrap();
        assert_eq!(template.status, "composite");
        assert_eq!(template.parts.as_ref().unwrap().outfits[0], 64);
        println!(
            "resolved models={}, trace steps={}",
            found.models.len(),
            found.steps.len()
        );
    }
}
