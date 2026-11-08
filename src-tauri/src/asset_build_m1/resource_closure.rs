//! Pure resource-closure discovery and materialization.
//!
//! The builder works on a full reference snapshot while writing only a delta
//! package.  This module is the boundary between those two views:
//!
//! 1. discover the external resources a Property/RW4 actually references;
//! 2. resolve them with SimCity's raster/Key conventions;
//! 3. copy the required bytes into the delta map;
//! 4. fail closed when a required reference cannot be resolved.
//!
//! No file I/O and no staging/DBPF knowledge lives here.

use dbpf::ResourceId;
use rw4::{Rw4File, SectionType};
use sc_properties::{Kind, LOT_MASK_HASH, PropertyFile, Value};
use std::collections::{HashMap, HashSet};

use super::materials::{raster_type, rw4_model_type};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const ICON_TYPE: u32 = 0x2F7D_0004;
const MARQUEE_TYPE: u32 = 0x3F86_62EA;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub label: String,
    /// Ordered candidates. The first existing one is the canonical match;
    /// all existing candidates are copied so the game can resolve either the
    /// RW4-wrapper or raster form of the same instance.
    pub candidates: Vec<ResourceId>,
    pub required: bool,
}

impl Dependency {
    fn new(label: impl Into<String>, candidates: Vec<ResourceId>, required: bool) -> Self {
        let mut seen = HashSet::new();
        let candidates = candidates
            .into_iter()
            .filter(|id| seen.insert((id.type_id, id.group, id.instance)))
            .collect();
        Self {
            label: label.into(),
            candidates,
            required,
        }
    }
}

/// Discover all references present in the delta slice.
///
/// `require_lot_mask` is true for a new menu identity: its Lot Property must
/// own a mask raster, even when the Key carries the incomplete `0:0:<id>`
/// form used by SCP files.
pub fn discover(
    resources: &HashMap<ResourceId, Vec<u8>>,
    require_lot_mask: bool,
) -> Result<Vec<Dependency>, String> {
    let mut deps = Vec::new();
    let mut seen = HashSet::new();
    for (id, bytes) in resources {
        if id.type_id == PROPERTY_TYPE {
            for dep in property_dependencies(bytes, require_lot_mask)? {
                if seen.insert(dep.label.clone()) {
                    deps.push(dep);
                }
            }
        } else if id.type_id == rw4_model_type() {
            for dep in model_material_dependencies(bytes)? {
                if seen.insert(dep.label.clone()) {
                    deps.push(dep);
                }
            }
        }
    }
    Ok(deps)
}

fn property_dependencies(bytes: &[u8], require_lot_mask: bool) -> Result<Vec<Dependency>, String> {
    let file = match PropertyFile::parse(bytes) {
        Ok(file) => file,
        Err(_) => return Ok(Vec::new()),
    };
    let mut deps = Vec::new();
    for property in &file.values {
        let keys: Vec<_> = match &property.kind {
            Kind::Scalar(Value::Key(key)) => vec![*key],
            Kind::Array(values) => values
                .iter()
                .filter_map(|value| match value {
                    Value::Key(key) => Some(*key),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        for key in keys {
            let exact = ResourceId {
                type_id: key.type_id,
                group: key.group,
                instance: key.instance,
            };
            if property.hash == LOT_MASK_HASH {
                // SCP LotMask Keys are frequently type/group 0; the actual
                // DBPF entry is a raster. Try exact, typed, then group-0.
                let mut candidates = vec![
                    ResourceId {
                        type_id: raster_type(),
                        group: key.group,
                        instance: key.instance,
                    },
                    ResourceId {
                        type_id: raster_type(),
                        group: 0,
                        instance: key.instance,
                    },
                ];
                if key.type_id != 0 {
                    candidates.insert(0, exact);
                }
                deps.push(Dependency::new(
                    format!("lot-mask:0x{:08X}", key.instance),
                    candidates,
                    require_lot_mask,
                ));
                continue;
            }
            // Structural Property keys (parent/unit/lot links) resolve from
            // the asset graph itself. Catalog PathEntry ids are script-side
            // built-ins and need no local B1 resource (the vanilla university
            // references 418A18B1 without shipping one). Custom C400 entries
            // remain mandatory: dropping one makes the runtime skip the path.
            if key.type_id == PROPERTY_TYPE {
                if (key.group & 0xFFFF) == 0xC400 {
                    let builtin = super::path_catalog::PATH_TYPES
                        .iter()
                        .any(|spec| spec.entry == key.instance);
                    deps.push(Dependency::new(
                        format!("path-entry:0x{:08X}", key.instance),
                        vec![exact],
                        !builtin,
                    ));
                }
                continue;
            }
            // For graphic Key properties (Lot textures, icons, marquee) a
            // local copy is optional: base packages may already provide it.
            if key.type_id == rw4_model_type()
                || key.type_id == raster_type()
                || key.type_id == ICON_TYPE
                || key.type_id == MARQUEE_TYPE
            {
                deps.push(Dependency::new(
                    format!("property-key:0x{:08X}", key.instance),
                    vec![exact],
                    false,
                ));
            }
        }
    }
    Ok(deps)
}

fn model_material_dependencies(bytes: &[u8]) -> Result<Vec<Dependency>, String> {
    let file = match Rw4File::parse(bytes) {
        Ok(file) => file,
        Err(_) => return Ok(Vec::new()),
    };
    if file.sections_of_type(SectionType::MESH).next().is_none() {
        return Ok(Vec::new());
    }
    let mut deps = Vec::new();
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let material = file
            .decode_material(bytes, section.number)
            .map_err(|e| format!("material #{}: {e}", section.number))?;
        let rw4::MaterialSection::Decoded(material) = material else {
            return Err(format!(
                "material #{} has an unknown layout; cannot prove closure",
                section.number
            ));
        };
        for slot in material.texture_slots() {
            if slot.texture_instance == 0 {
                continue;
            }
            let slot_number = slot.slot_byte();
            let mut candidates = Vec::new();
            if slot_number == 0 {
                // Slot0 is the f32 parameter table and must be an RW4
                // palette, never a bare raster.
                candidates.push(ResourceId {
                    type_id: rw4_model_type(),
                    group: 0,
                    instance: slot.texture_instance,
                });
            } else {
                candidates.push(ResourceId {
                    type_id: raster_type(),
                    group: 0,
                    instance: slot.texture_instance,
                });
                candidates.push(ResourceId {
                    type_id: rw4_model_type(),
                    group: 0,
                    instance: slot.texture_instance,
                });
            }
            deps.push(Dependency::new(
                format!(
                    "material:slot{}:0x{:08X}",
                    slot_number, slot.texture_instance
                ),
                candidates,
                true,
            ));
        }
    }
    Ok(deps)
}

/// Copy every resolvable dependency into `written`; reject missing required
/// references. Existing delta entries are left untouched.
pub fn complete(
    written: &mut HashMap<ResourceId, Vec<u8>>,
    available: &HashMap<ResourceId, Vec<u8>>,
    deps: &[Dependency],
) -> Result<(), String> {
    let mut missing = Vec::new();
    for dep in deps {
        if dep.candidates.iter().any(|id| written.contains_key(id)) {
            continue;
        }
        let mut copied = false;
        for id in &dep.candidates {
            if let Some(bytes) = available.get(id) {
                written.insert(*id, bytes.clone());
                copied = true;
            }
        }
        if !copied && dep.required {
            missing.push(dep.label.clone());
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        missing.sort();
        Err(format!(
            "resource closure missing {} required reference(s): {}",
            missing.len(),
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{Key, PropType, Property, PropertyEncoding};

    fn property(hash: u32, key: Key) -> Property {
        Property {
            hash,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(key)),
            encoding: PropertyEncoding::default(),
        }
    }

    fn encoded(properties: Vec<Property>) -> Vec<u8> {
        PropertyFile {
            values: properties,
            claimed_count: 0,
        }
        .encode_canonical()
        .unwrap()
    }

    #[test]
    fn incomplete_lot_mask_key_resolves_to_raster() {
        let bytes = encoded(vec![property(
            LOT_MASK_HASH,
            Key {
                instance: 0x4E23_544D,
                type_id: 0,
                group: 0,
            },
        )]);
        let deps = property_dependencies(&bytes, true).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(
            deps[0].candidates,
            vec![ResourceId {
                type_id: raster_type(),
                group: 0,
                instance: 0x4E23_544D
            }]
        );
    }

    #[test]
    fn complete_copies_available_dependency_and_rejects_missing_required() {
        let mask = ResourceId {
            type_id: raster_type(),
            group: 0,
            instance: 0x4E23_544D,
        };
        let bytes = encoded(vec![property(
            LOT_MASK_HASH,
            Key {
                instance: mask.instance,
                type_id: 0,
                group: 0,
            },
        )]);
        let deps = property_dependencies(&bytes, true).unwrap();
        let mut available = HashMap::new();
        available.insert(mask, vec![1, 2, 3]);
        let mut written = HashMap::new();
        complete(&mut written, &available, &deps).unwrap();
        assert_eq!(written.get(&mask), Some(&vec![1, 2, 3]));

        let mut empty = HashMap::new();
        assert!(complete(&mut empty, &HashMap::new(), &deps).is_err());
    }

    #[test]
    fn path_entry_property_is_required_dependency() {
        let entry = ResourceId {
            type_id: PROPERTY_TYPE,
            group: 0x40E0_C400,
            instance: 0x0310_6C2E,
        };
        let bytes = encoded(vec![property(
            0x0CC8_FE7A,
            Key {
                type_id: entry.type_id,
                group: entry.group,
                instance: entry.instance,
            },
        )]);
        let deps = property_dependencies(&bytes, true).unwrap();
        assert!(deps.iter().any(|dep| {
            dep.label == "path-entry:0x03106C2E" && dep.required && dep.candidates == vec![entry]
        }));
    }
}
