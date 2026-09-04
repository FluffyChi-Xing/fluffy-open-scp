//! Cross-package texture/material resource resolution.
use dbpf::{Package, ResourceId};
use rw4::MaterialSection;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TextureIndex<'a> {
    packages: Vec<&'a Package>,
    by_instance: HashMap<u32, (usize, ResourceId)>,
    conflicts: Vec<TextureConflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureConflict {
    pub instance: u32,
    pub selected: ResourceId,
    pub shadowed: ResourceId,
}

impl<'a> TextureIndex<'a> {
    fn insert_resource(&mut self, package_index: usize, id: ResourceId) {
        if let Some((_, selected)) = self.by_instance.get(&id.instance).copied() {
            self.conflicts.push(TextureConflict {
                instance: id.instance,
                selected,
                shadowed: id,
            });
        } else {
            self.by_instance.insert(id.instance, (package_index, id));
        }
    }

    pub fn new(packages: Vec<&'a Package>) -> Self {
        let mut index = Self {
            packages,
            by_instance: HashMap::new(),
            conflicts: Vec::new(),
        };
        let resources: Vec<_> = index
            .packages
            .iter()
            .enumerate()
            .flat_map(|(package_index, package)| {
                package.entries().iter().filter_map(move |entry| {
                    matches!(entry.id.type_id, 0x2F4E681B | 0x2F4E681C)
                        .then_some((package_index, entry.id))
                })
            })
            .collect();
        for (package_index, id) in resources {
            index.insert_resource(package_index, id);
        }
        index
    }

    pub fn resource(&self, instance: u32) -> Option<(&'a Package, ResourceId)> {
        let (package, id) = *self.by_instance.get(&instance)?;
        Some((self.packages[package], id))
    }

    pub fn conflicts(&self) -> &[TextureConflict] {
        &self.conflicts
    }

    /// Resolve a decoded RW4 material's texture slots against sibling packages.
    pub fn resolve_material(&self, material: &MaterialSection) -> Vec<ResolvedTexture<'a>> {
        let MaterialSection::Decoded(material) = material else {
            return Vec::new();
        };
        material
            .texture_slots()
            .filter_map(|slot| {
                self.resource(slot.texture_instance)
                    .map(|(package, id)| ResolvedTexture {
                        slot: slot.slot_byte(),
                        package,
                        id,
                    })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedTexture<'a> {
    pub slot: u8,
    pub package: &'a Package,
    pub id: ResourceId,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_material_is_safe() {
        let index = TextureIndex::new(Vec::new());
        assert!(
            index
                .resolve_material(&MaterialSection::Raw(Vec::new()))
                .is_empty()
        );
    }

    #[test]
    fn first_package_wins_and_duplicate_is_diagnosed() {
        let mut index = TextureIndex::new(Vec::new());
        let selected = ResourceId {
            type_id: 0x2F4E681B,
            group: 1,
            instance: 7,
        };
        let shadowed = ResourceId {
            type_id: 0x2F4E681C,
            group: 2,
            instance: 7,
        };
        index.insert_resource(0, selected);
        index.insert_resource(1, shadowed);
        assert_eq!(index.by_instance.get(&7), Some(&(0, selected)));
        assert_eq!(
            index.conflicts(),
            &[TextureConflict {
                instance: 7,
                selected,
                shadowed
            }]
        );
    }
}
