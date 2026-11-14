use super::menu::MenuRegistrationSpec;
use dbpf::ResourceId;
use sc_properties::{
    Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Value as PropValue,
};
use serde_json::Value;

pub(super) const PROCESSOR_VERSION: &str = "openscp.property.m1/57";

// menu_locations.py:8 supplies the Basic Parks destination. The existing
// workbench park route supplies the Menu2 parent and building tool class.
const BASIC_PARKS: u32 = 0x0704_9250;
const PARK_MENU_PARENT: u32 = 0xF970_A822;
const BUILDING_TOOL_CATEGORY: u32 = 0xC710_B6E9;
const TOOL_ACTION: u32 = 0x08F6_72C7;
const PLOP_ACTION: u32 = 0x1943_CB96;

/// Complete the placement tool for a new menu identity independently of its
/// Unit rules. Returns whether this menu needs the standalone plop contract.
pub(super) fn complete_menu(spec: &mut MenuRegistrationSpec, nodes: &[Value]) -> bool {
    if !nodes.iter().any(|node| {
        matches!(
            node["type"].as_str(),
            Some("residence" | "office" | "hotel")
        )
    }) {
        return false;
    }

    if spec.ui_categories().is_none() {
        spec.category_instance = Some(BASIC_PARKS);
    }
    if spec.menu_parent_instance.is_none() {
        let parks = spec
            .ui_categories()
            .is_some_and(|ids| ids.contains(&BASIC_PARKS));
        spec.menu_parent_instance = Some(if parks {
            PARK_MENU_PARENT
        } else {
            super::menu::PLOP_MENU_PARENT
        });
    }
    spec.building_category_instance
        .get_or_insert(BUILDING_TOOL_CATEGORY);
    true
}

pub(super) fn bind_tool(generated: &mut super::menu::MenuResources) -> Result<(), String> {
    let instance = generated.instance;
    let (_, bytes) = generated
        .entries
        .iter_mut()
        .find(|(id, _)| *id == menu_id(instance))
        .ok_or_else(|| format!("menu registration: missing Menu2 {instance:08X}"))?;
    let mut menu = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
    // Match the scalar Key record in the Packager's complete-building Menu2.
    let action = Property {
        hash: TOOL_ACTION,
        prop_type: PropType::Key,
        kind: Kind::Scalar(PropValue::Key(Key {
            type_id: 0,
            group: 0,
            instance: PLOP_ACTION,
        })),
        encoding: PropertyEncoding {
            flags: 0x8000,
            array_item_size: None,
        },
    };
    if menu.get(TOOL_ACTION) != Some(&action) {
        if let Some(property) = menu.values.iter_mut().find(|prop| prop.hash == TOOL_ACTION) {
            *property = action;
        } else {
            menu.values.push(action);
        }
        *bytes = super::property_edit::encode(&menu)?;
    }
    Ok(())
}

pub(super) fn verify_written(
    resources: &super::connector::Resources,
    instance: u32,
    spec: &MenuRegistrationSpec,
) -> Result<(), String> {
    let id = menu_id(instance);
    let bytes = resources
        .get(&id)
        .ok_or_else(|| format!("written-contract: missing Menu2 {id}"))?;
    let menu = PropertyFile::parse(bytes)
        .map_err(|error| format!("written-contract: Menu2 {id}: {error}"))?;
    for (hash, expected) in [
        (super::menu::HASH_PARENT, spec.menu_parent_instance),
        (
            super::menu::HASH_BUILDING_CATEGORY,
            spec.building_category_instance,
        ),
        (TOOL_ACTION, Some(PLOP_ACTION)),
    ] {
        if let Some(expected) = expected
            && key_instance(&menu, hash) != Some(expected)
        {
            return Err(format!(
                "written-contract: Menu2 {id} {hash:08X} expected {expected:08X}"
            ));
        }
    }
    Ok(())
}

fn menu_id(instance: u32) -> ResourceId {
    ResourceId {
        type_id: super::menu::PROPERTY_TYPE,
        group: super::menu::MENU_GROUP,
        instance,
    }
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    match file.get(hash)?.scalar()? {
        PropValue::Key(key) => Some(key.instance),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_menu_carries_plop_action_through_readback() {
        let spec = MenuRegistrationSpec::from_values(&std::collections::BTreeMap::from([(
            "title".into(),
            serde_json::json!("Hotel"),
        )]))
        .unwrap();
        let mut generated = super::super::menu::MenuResources {
            instance: 0x1234_5678,
            entries: vec![(
                menu_id(0x1234_5678),
                super::super::property_edit::encode(&PropertyFile::default()).unwrap(),
            )],
        };
        let before = generated.entries.iter().cloned().collect();
        assert!(verify_written(&before, generated.instance, &spec).is_err());
        bind_tool(&mut generated).unwrap();
        let after = generated.entries.into_iter().collect();
        verify_written(&after, generated.instance, &spec).unwrap();
    }
}
