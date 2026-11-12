//! Native Lot decal rows. A decal key selects an atlas entry. The other
//! three columns carry placement and projection parameters at the same index.

use super::scene::{SceneDocument, SceneUnit, array_property, transform_value};
use crate::decal_library::{raster_resource, read_atlas};
use dbpf::{Package, ResourceId};
use sc_properties::lot_unit::{
    DECAL_CATEGORIES, DECAL_DEPTH_BASE, DECAL_ID_BASE, DECAL_MATERIAL_BASE, DECAL_TRANSFORM_BASE,
};
use sc_properties::{Key, Kind, PropType, Property, PropertyFile, Value};
use serde_json::Value as Json;
use std::collections::{BTreeSet, HashMap};

/// Older workbench documents keep the native ID as a formatted Property field.
pub(super) fn legacy_id(unit: &Json, category: usize) -> Option<Key> {
    if category >= DECAL_CATEGORIES {
        return None;
    }
    let hash = DECAL_ID_BASE + category as u32;
    let field_key = format!("0x{hash:08X}");
    let stored = unit["fields"][field_key].as_str().or_else(|| {
        unit["fieldRows"].as_array()?.iter().find_map(|field| {
            (field["hash"].as_u64() == Some(u64::from(hash)))
                .then(|| field["value"].as_str())
                .flatten()
        })
    })?;
    let parts: Vec<_> = stored.split_whitespace().collect();
    let ["T", type_id, "-", "G", group, "-", "I", instance] = parts.as_slice() else {
        return None;
    };
    let parse = |value| u32::from_str_radix(value, 16).ok();
    Some(Key {
        type_id: parse(type_id)?,
        group: parse(group)?,
        instance: parse(instance)?,
    })
}

pub(super) fn columns(document: &SceneDocument) -> Result<Vec<Property>, String> {
    let mut rows: [[Vec<Value>; 4]; DECAL_CATEGORIES] = Default::default();
    for unit in &document.units {
        let SceneUnit::Decal {
            category,
            id,
            source: _,
            transform,
            scale,
            depth,
            material_data,
        } = unit
        else {
            continue;
        };
        let row = rows
            .get_mut(*category)
            .ok_or_else(|| format!("DECAL_CATEGORY: {category}"))?;
        let position = row[0].len();
        let missing =
            |field| format!("DECAL_FIELD: category {category}, index {position}, {field}");
        let key = id
            .filter(|key| key.instance != 0)
            .ok_or_else(|| missing("decalId"))?;
        if scale.is_some_and(|value| !value.is_finite()) {
            return Err(missing("scale"));
        }
        let transform = transform_value(*scale, transform)
            .filter(|value| matches!(value, Value::Transform(t) if t.matrix.len() == 12 && t.matrix.iter().all(|v| v.is_finite())))
            .ok_or_else(|| missing("transform"))?;
        let depth = depth
            .filter(|v| v.is_finite())
            .ok_or_else(|| missing("depth"))?;
        let material = material_data
            .filter(|v| v.iter().all(|v| v.is_finite()))
            .ok_or_else(|| missing("materialData"))?;
        for (column, value) in row.iter_mut().zip([
            Value::Key(key),
            transform,
            Value::Float(depth),
            Value::Vector3(material),
        ]) {
            column.push(value);
        }
    }
    let mut output = Vec::new();
    for (category, row) in rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| !row[0].is_empty())
    {
        for ((base, kind), values) in [
            (DECAL_ID_BASE, PropType::Key),
            (DECAL_TRANSFORM_BASE, PropType::Transform),
            (DECAL_DEPTH_BASE, PropType::Float),
            (DECAL_MATERIAL_BASE, PropType::Vector3),
        ]
        .into_iter()
        .zip(row)
        {
            let mut column = array_property(base + category as u32, kind, values);
            column.encoding.flags = 0x809C;
            output.push(column);
        }
    }
    Ok(output)
}

pub(super) fn native_columns(
    document: &SceneDocument,
    degrees: u32,
) -> Result<Vec<Property>, String> {
    let mut columns = columns(document)?;
    let transforms = DECAL_TRANSFORM_BASE..DECAL_TRANSFORM_BASE + DECAL_CATEGORIES as u32;
    for column in columns
        .iter_mut()
        .filter(|column| transforms.contains(&column.hash))
    {
        if let Kind::Array(values) = &mut column.kind {
            for value in values {
                if let Value::Transform(transform) = value {
                    let matrix = super::native_profile::rotate_matrix(&transform.matrix, degrees)?;
                    transform.matrix =
                        super::native_profile::apply_model_translation(matrix, document).to_vec();
                }
            }
        }
    }
    Ok(columns)
}

pub(super) fn validate_sources(document: &SceneDocument) -> Result<Vec<String>, String> {
    let mut packages = HashMap::new();
    let mut dictionaries = HashMap::new();
    let mut selected_atlases = HashMap::new();
    let mut dependencies = BTreeSet::new();
    for unit in &document.units {
        let SceneUnit::Decal {
            id,
            source: Some(source),
            ..
        } = unit
        else {
            continue;
        };
        let error = |detail: &str| format!("DECAL_SOURCE: {} ({detail})", source.package_path);
        let key = id.ok_or_else(|| error("decalId missing"))?;
        let atlas_id: ResourceId = source.atlas.clone().into();
        if selected_atlases
            .insert(key.instance, atlas_id)
            .is_some_and(|previous| previous != atlas_id)
        {
            return Err(error("decal ID selected from different atlases"));
        }
        let atlas_key = (&source.package_path, atlas_id);
        let dictionary = match dictionaries.entry(atlas_key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let package = open_source(&mut packages, &source.package_path)?;
                entry.insert(read_atlas(package, &source.atlas).map_err(|cause| error(&cause))?)
            }
        };
        let raster = dictionary
            .entries
            .iter()
            .find_map(|entry| (entry.id == Some(key)).then_some(entry.raster).flatten())
            .ok_or_else(|| error("decal ID or raster key missing in the selected atlas"))?;
        let raster_path = source
            .raster_package_path
            .as_deref()
            .unwrap_or(&source.package_path);
        let raster_package = open_source(&mut packages, raster_path)?;
        if raster_resource(raster_package, raster).is_none() {
            return Err(format!(
                "DECAL_SOURCE: {raster_path} (raster {:08X} missing)",
                raster.instance
            ));
        }
        dependencies.insert(source.package_path.clone());
        dependencies.insert(raster_path.to_owned());
    }
    Ok(dependencies.into_iter().collect())
}

fn open_source<'a>(
    packages: &'a mut HashMap<String, Package>,
    path: &str,
) -> Result<&'a Package, String> {
    Ok(match packages.entry(path.to_owned()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
            Package::open(path).map_err(|cause| format!("DECAL_SOURCE: {path} ({cause})"))?,
        ),
    })
}

pub(super) fn verify(lot: &PropertyFile, document: &SceneDocument) -> Result<(), String> {
    let expected = columns(document)?;
    for category in 0..DECAL_CATEGORIES as u32 {
        for base in [
            DECAL_ID_BASE,
            DECAL_TRANSFORM_BASE,
            DECAL_DEPTH_BASE,
            DECAL_MATERIAL_BASE,
        ] {
            let hash = base + category;
            let wanted = expected.iter().find(|column| column.hash == hash);
            match (wanted, lot.get(hash)) {
                (None, None) => {}
                (Some(wanted), Some(written)) if wanted.prop_type == written.prop_type => {
                    let (Kind::Array(left), Kind::Array(right)) = (&wanted.kind, &written.kind)
                    else {
                        return Err(format!("DECAL_READBACK: {hash:08X} is not an array"));
                    };
                    // A geometry stage may translate transforms. The ID, depth
                    // and material values must still round-trip in row order.
                    if left.len() != right.len() || (base != DECAL_TRANSFORM_BASE && left != right)
                    {
                        return Err(format!("DECAL_READBACK: {hash:08X} row mismatch"));
                    }
                    if base == DECAL_TRANSFORM_BASE
                        && left.iter().zip(right).any(|(before, after)| {
                            !matches!((before, after), (Value::Transform(expected), Value::Transform(actual))
                                if actual.unknown == expected.unknown
                                    && actual.matrix.len() == 12
                                    && actual.matrix.iter().all(|value| value.is_finite()))
                        })
                    {
                        return Err(format!("DECAL_READBACK: {hash:08X} invalid transform or scale"));
                    }
                }
                _ => return Err(format!("DECAL_READBACK: {hash:08X} missing or wrong type")),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::scene::{SceneWrites, apply_units, parse_document};
    use serde_json::json;

    fn row() -> Json {
        json!({
            "kind": "decal",
            "category": 1,
            "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,2,3]},
            "scale": 1.5,
            "depth": 1.0,
            "materialData": [0.5, 0, 0],
            "fieldRows": [{
                "hash": DECAL_ID_BASE + 1,
                "typeName": "Key",
                "value": "T 00000000 - G 00000000 - I D2DD48C5"
            }]
        })
    }

    #[test]
    fn decal_id_and_projection_columns_round_trip_and_can_be_deleted() {
        let document = parse_document(&json!({"units": [row()]})).unwrap();
        let writes = SceneWrites {
            decals: true,
            ..Default::default()
        };
        let mut file = PropertyFile::default();
        apply_units(&mut file, &document, &writes).unwrap();
        let mut written = PropertyFile::parse(&file.encode_canonical().unwrap()).unwrap();
        verify(&written, &document).unwrap();
        assert_eq!(
            written.get(DECAL_ID_BASE + 1).unwrap().kind,
            Kind::Array(vec![Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: 0xD2DD_48C5
            }),])
        );
        apply_units(&mut written, &SceneDocument::default(), &writes).unwrap();
        verify(&written, &SceneDocument::default()).unwrap();
        assert!(written.values.is_empty());
    }

    #[test]
    fn decal_write_rejects_an_incomplete_row() {
        let mut input = row();
        input["fieldRows"] = json!([]);
        let document = parse_document(&json!({"units": [input]})).unwrap();
        assert!(columns(&document).unwrap_err().contains("decalId"));
    }

    #[test]
    #[ignore = "requires the local original SimCity packages"]
    fn sourced_scene_decal_links_to_a_readable_package_without_copying_the_atlas() {
        use crate::asset_build_m1::{connector, tests::temp_root};
        let game = std::path::PathBuf::from(std::env::var("OPENSCP_DECAL_GAME_DATA").unwrap());
        let mut input = row();
        input["fieldRows"] = json!([]);
        input["decalId"] = json!({"typeId": 0, "group": 0, "instance": 0xD2DD_48C5u32});
        input["decalSource"] = json!({
            "packagePath": game.join("SimCity_Game.package"),
            "atlas": {"typeId": 0x00B1_B104, "group": 0xC67F_B185u32, "instance": 0xEEFD_390Cu32},
            "rasterPackagePath": game.join("SimCity_Graphics.package"),
        });
        let document = parse_document(&json!({"units": [input.clone()]})).unwrap();
        assert_eq!(validate_sources(&document).unwrap().len(), 2);
        let mut lot = PropertyFile::default();
        let writes = SceneWrites {
            decals: true,
            ..Default::default()
        };
        apply_units(&mut lot, &document, &writes).unwrap();
        let lot_id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0x7FFF_0011,
        };
        let root = temp_root();
        let output = root.join("decal-readback.package");
        let resources = HashMap::from([(lot_id, lot.encode_canonical().unwrap())]);
        std::fs::write(&output, connector::link(&resources).unwrap()).unwrap();
        let reread = connector::read_source(&output).unwrap();
        assert_eq!(reread.len(), 1);
        verify(&PropertyFile::parse(&reread[&lot_id]).unwrap(), &document).unwrap();
        let _ = std::fs::remove_dir_all(root);

        input["decalSource"]["rasterPackagePath"] = json!(game.join("SimCity_Game.package"));
        let broken = parse_document(&json!({"units": [input]})).unwrap();
        assert!(
            validate_sources(&broken)
                .unwrap_err()
                .starts_with("DECAL_SOURCE")
        );
    }
}
