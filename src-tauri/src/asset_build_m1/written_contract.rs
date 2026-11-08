//! Final DBPF contract validation.
//!
//! This module runs after the package has been linked and read back from disk.
//! Intermediate resource pools are not accepted as evidence: menu rows, scene
//! columns, LOD models and all six material slots must survive in the actual
//! deliverable. Any violation aborts publishing.

use std::collections::{HashMap, HashSet};

use dbpf::ResourceId;
use rw4::{MaterialSection, Rw4File, SectionType};
use sc_properties::{
    Key, Kind, PropertyFile, Value, LOD_MODEL_HASHES, LOT_MASK_HASH, LOT_SIZE_HASH,
};

use super::connector::Resources;
use super::package::LodModelIds;
use super::scene::{SceneDocument, SceneLotMaskKind, SceneUnit, SceneWrites};
use sha2::{Digest as _, Sha256};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const MENU_GROUP: u32 = 0x0987_8A01;
const ICON_TYPE: u32 = 0x2F7D_0004;
const MARQUEE_TYPE: u32 = 0x3F86_62EA;
const TOOL_ICON_HASH: u32 = 0x0977_AA8F;
const MARQUEE_HASH: u32 = 0x0DDE_EE56;
const PATH_ENTRIES: u32 = 0x0CC8_FE7A;
const PATH_SCOPES: u32 = 0x0D9A_4E54;
const PATH_CONNECTS: u32 = 0x0D9A_A2BB;

pub(crate) struct WrittenContractSpec<'a> {
    pub menu_instance: Option<u32>,
    pub reference_lot: Option<ResourceId>,
    pub document: Option<&'a SceneDocument>,
    pub writes: &'a SceneWrites,
    pub model_ids: Option<LodModelIds>,
    pub author_building40: bool,
    pub donor: Option<(&'a Resources, u32)>,
    pub ground_source: Option<(&'a Resources, ResourceId)>,
}

pub(crate) struct WrittenContractReport {
    pub lowered_units: usize,
    pub detail: serde_json::Value,
}

pub(crate) fn validate(
    resources: &Resources,
    spec: WrittenContractSpec<'_>,
) -> Result<WrittenContractReport, String> {
    let instance = spec
        .menu_instance
        .or(spec.reference_lot.map(|id| id.instance));
    let Some(instance) = instance else {
        return Ok(WrittenContractReport {
            lowered_units: 0,
            detail: serde_json::json!({"status": "skipped", "reason": "no lot identity"}),
        });
    };
    let lot_group = if spec.menu_instance.is_some() {
        super::menu::lot_group()
    } else {
        spec.reference_lot.expect("lot checked above").group
    };
    let menu = super::menu_contract::validate(
        resources,
        instance,
        lot_group,
        spec.donor,
        spec.menu_instance.is_some(),
    )?;
    let lot_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: lot_group,
        instance,
    };
    let lot_bytes = resources
        .get(&lot_id)
        .ok_or_else(|| format!("written-contract: final Asset/Lot {lot_id} is missing"))?;
    let lot = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("written-contract: final Asset/Lot {lot_id}: {error}"))?;

    let presentation = if spec.menu_instance.is_some() {
        Some(validate_presentation(resources, instance)?)
    } else {
        None
    };
    let scene = match spec.document {
        Some(document) => Some(validate_scene(resources, &lot, document, spec.writes)?),
        None => None,
    };
    let (ground_mask, ground_style) = match spec.document {
        Some(document) => validate_ground(resources, &lot, document, spec.ground_source)?,
        None => (None, None),
    };
    let models = match spec.model_ids {
        Some(model_ids) => Some(validate_models(
            resources,
            &lot,
            &model_ids,
            spec.author_building40,
        )?),
        None => None,
    };
    let lowered_units = spec.document.map_or(0, |document| {
        document
            .units
            .iter()
            .filter(|unit| unit_is_written(unit, spec.writes))
            .count()
    });
    Ok(WrittenContractReport {
        lowered_units,
        detail: serde_json::json!({
            "status": "ok",
            "identity": format!("{instance:08X}"),
            "menu": menu.to_json(),
            "presentation": presentation,
            "scene": scene,
            "groundMask": ground_mask,
            "groundStyle": ground_style,
            "loweredUnits": lowered_units,
            "models": models,
            "resourceCount": resources.len(),
        }),
    })
}

fn unit_is_written(unit: &SceneUnit, writes: &SceneWrites) -> bool {
    if writes.authoritative {
        return true;
    }
    match unit {
        SceneUnit::Light { .. } => writes.lights,
        SceneUnit::Effect { .. } => writes.effects,
        SceneUnit::Decal { .. } => writes.decals,
        SceneUnit::Prop { bin, .. } => writes.prop_bin_declared(*bin),
        SceneUnit::PathPoint { .. } => writes.paths,
        SceneUnit::Spawner { .. } => writes.spawners,
    }
}

fn validate_presentation(
    resources: &Resources,
    instance: u32,
) -> Result<serde_json::Value, String> {
    let menu_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: MENU_GROUP,
        instance,
    };
    let menu = PropertyFile::parse(
        resources
            .get(&menu_id)
            .ok_or_else(|| format!("written-contract: Menu2 {menu_id} is missing"))?,
    )
    .map_err(|error| format!("written-contract: Menu2 {menu_id}: {error}"))?;
    let mut report = serde_json::Map::new();
    for (hash, expected_type, label) in [
        (TOOL_ICON_HASH, ICON_TYPE, "icon"),
        (MARQUEE_HASH, MARQUEE_TYPE, "marquee"),
    ] {
        let key = scalar_key(&menu, hash)
            .ok_or_else(|| format!("written-contract: Menu2 {label} key {hash:08X} is missing"))?;
        if key.type_id != expected_type || key.instance != instance {
            return Err(format!(
                "written-contract: Menu2 {label} points to {:08X}:{:08X}:{:08X}, expected type {expected_type:08X} and identity {instance:08X}",
                key.type_id, key.group, key.instance
            ));
        }
        let id = ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        };
        let bytes = resources
            .get(&id)
            .ok_or_else(|| format!("written-contract: Menu2 {label} resource {id} is missing"))?;
        let image = image::load_from_memory(bytes).map_err(|error| {
            format!("written-contract: Menu2 {label} {id} is not a readable image: {error}")
        })?;
        if image.width() == 0 || image.height() == 0 {
            return Err(format!(
                "written-contract: Menu2 {label} {id} has empty dimensions"
            ));
        }
        report.insert(
            label.into(),
            serde_json::json!({
                "tgi": id.to_string(),
                "width": image.width(),
                "height": image.height(),
                "bytes": bytes.len(),
            }),
        );
    }
    Ok(serde_json::Value::Object(report))
}

fn validate_scene(
    resources: &Resources,
    lot: &PropertyFile,
    document: &SceneDocument,
    writes: &SceneWrites,
) -> Result<serde_json::Value, String> {
    super::scene::verify_units(lot, document, writes)
        .map_err(|error| format!("written-contract: {error}"))?;
    if let Some(expected) = document.lot_size {
        let actual = match lot.get(LOT_SIZE_HASH).map(|property| &property.kind) {
            Some(Kind::Scalar(Value::Vector2(size))) => Some(*size),
            Some(Kind::Array(values)) => values.iter().find_map(|value| match value {
                Value::Vector2(size) => Some(*size),
                _ => None,
            }),
            _ => None,
        }
        .ok_or("written-contract: LotSize 0CCB7FC8 is missing or has the wrong type")?;
        if actual != expected {
            return Err(format!(
                "written-contract: LotSize {:?} != authored {:?}",
                actual, expected
            ));
        }
    }
    let paths = if writes.authoritative || writes.paths {
        Some(validate_paths(resources, lot, document)?)
    } else {
        None
    };
    let props = validate_props(lot, document, writes)?;
    Ok(serde_json::json!({
        "sourceUnits": document.units.len(),
        "lotSize": document.lot_size,
        "paths": paths,
        "props": props,
    }))
}

fn validate_ground(
    resources: &Resources,
    lot: &PropertyFile,
    document: &SceneDocument,
    ground_source: Option<(&Resources, ResourceId)>,
) -> Result<(Option<serde_json::Value>, Option<serde_json::Value>), String> {
    let ground_mask = match document.lot_mask {
        Some(SceneLotMaskKind::Source) => {
            let (source_resources, source_lot) = ground_source
                .ok_or("written-contract: lot.mask.kind=source has no source snapshot")?;
            let source_lot_bytes = source_resources.get(&source_lot).ok_or_else(|| {
                format!("written-contract: ground source Lot {source_lot} is missing")
            })?;
            let source_file = PropertyFile::parse(source_lot_bytes).map_err(|error| {
                format!("written-contract: ground source Lot {source_lot}: {error}")
            })?;
            let source_key = scalar_key(&source_file, LOT_MASK_HASH).ok_or_else(|| {
                format!("written-contract: ground source Lot {source_lot} has no LotMask")
            })?;
            let (source_id, source_bytes) =
                super::menu::find_lot_mask(source_resources, source_key)
                    .ok_or("written-contract: ground source LotMask raster is missing")?;
            let final_key = scalar_key(lot, LOT_MASK_HASH)
                .ok_or("written-contract: final LotMask key is missing or has the wrong type")?;
            let (final_id, final_bytes) = super::menu::find_lot_mask(resources, final_key)
                .ok_or("written-contract: final LotMask raster is missing")?;
            let (source_report, source_dimensions, source_rgba) =
                raster_evidence(source_id, source_bytes)?;
            let (final_report, final_dimensions, final_rgba) =
                raster_evidence(final_id, final_bytes)?;
            if source_dimensions != final_dimensions || source_rgba != final_rgba {
                return Err(format!(
                    "written-contract: source LotMask pixel mismatch: source {:?} sha {}, final {:?} sha {}",
                    source_dimensions,
                    format!("{:x}", Sha256::digest(&source_rgba)),
                    final_dimensions,
                    format!("{:x}", Sha256::digest(&final_rgba)),
                ));
            }
            Some(serde_json::json!({
                "kind": "source",
                "sourceLot": source_lot.to_string(),
                "source": source_report,
                "final": final_report,
                "pixelExact": true,
            }))
        }
        Some(SceneLotMaskKind::Synthetic) => {
            let expected = document
                .lot_size
                .ok_or("written-contract: synthetic LotMask requires lot.size")?;
            let final_key = scalar_key(lot, LOT_MASK_HASH)
                .ok_or("written-contract: final LotMask key is missing or has the wrong type")?;
            let (final_id, final_bytes) = super::menu::find_lot_mask(resources, final_key)
                .ok_or("written-contract: final LotMask raster is missing")?;
            let (final_report, dimensions, _) = raster_evidence(final_id, final_bytes)?;
            let expected_dimensions = [
                expected[0].round().max(1.0) as u32,
                expected[1].round().max(1.0) as u32,
            ];
            if [dimensions.0, dimensions.1] != expected_dimensions {
                return Err(format!(
                    "written-contract: synthetic LotMask dimensions {}x{} do not match LotSize {}x{}",
                    dimensions.0, dimensions.1, expected_dimensions[0], expected_dimensions[1]
                ));
            }
            Some(serde_json::json!({
                "kind": "synthetic",
                "final": final_report,
                "expectedDimensions": expected_dimensions,
            }))
        }
        None => None,
    };
    let expected_values = super::scene::ground_values(document);
    let ground_style = if expected_values.is_empty() {
        None
    } else {
        let mut fields = Vec::new();
        for (hash, expected) in expected_values {
            let actual = match lot.get(hash).map(|property| &property.kind) {
                Some(Kind::Scalar(value)) => value,
                other => {
                    return Err(format!(
                        "written-contract: ground style {hash:08X} is missing or not scalar: {other:?}"
                    ));
                }
            };
            if actual != &expected {
                return Err(format!(
                    "written-contract: ground style {hash:08X} is {actual:?}, expected {expected:?}"
                ));
            }
            fields.push(serde_json::json!({
                "hash": format!("{hash:08X}"),
                "value": format!("{actual:?}"),
            }));
        }
        Some(serde_json::json!({"status": "ok", "fields": fields}))
    };
    Ok((ground_mask, ground_style))
}

fn raster_evidence(
    id: ResourceId,
    bytes: &[u8],
) -> Result<(serde_json::Value, (u32, u32), Vec<u8>), String> {
    let raster = rw4::RasterImage::parse(bytes)
        .map_err(|error| format!("written-contract: LotMask {id}: {error}"))?;
    let rgba = raster
        .decode_top_mip_rgba()
        .map_err(|error| format!("written-contract: LotMask {id}: {error}"))?;
    if raster.width == 0 || raster.height == 0 || rgba.is_empty() || rgba.len() % 4 != 0 {
        return Err(format!("written-contract: LotMask {id} has invalid pixels"));
    }
    let mut min = [u8::MAX; 4];
    let mut max = [u8::MIN; 4];
    let mut nonzero = [0usize; 4];
    let mut selected = [0usize; 4];
    let mut channel_values: [HashSet<u8>; 4] = std::array::from_fn(|_| HashSet::new());
    let mut pixels = HashSet::new();
    for pixel in rgba.as_chunks::<4>().0 {
        let pixel = [pixel[0], pixel[1], pixel[2], pixel[3]];
        pixels.insert(pixel);
        for channel in 0..4 {
            min[channel] = min[channel].min(pixel[channel]);
            max[channel] = max[channel].max(pixel[channel]);
            nonzero[channel] += usize::from(pixel[channel] != 0);
            selected[channel] += usize::from(pixel[channel] >= 128);
            channel_values[channel].insert(pixel[channel]);
        }
    }
    let unique_by_channel: Vec<usize> = channel_values.iter().map(HashSet::len).collect();
    let pixel_count = rgba.len() / 4;
    let report = serde_json::json!({
        "tgi": id.to_string(),
        "width": raster.width,
        "height": raster.height,
        "rgbaSha256": format!("{:x}", Sha256::digest(&rgba)),
        "pixelCount": pixel_count,
        "uniquePixels": pixels.len(),
        "min": min,
        "max": max,
        "nonzeroByChannel": nonzero,
        "selectedAt128ByChannel": selected,
        "uniqueValuesByChannel": unique_by_channel,
    });
    Ok((report, (raster.width, raster.height), rgba))
}

fn validate_paths(
    resources: &Resources,
    lot: &PropertyFile,
    document: &SceneDocument,
) -> Result<serde_json::Value, String> {
    use sc_properties::lot_unit::{
        PATH_PAIRS, PATH_POINT_INDICES, PATH_POINT_POINTS, PATH_POINT_TANGENTS,
    };

    let expected_points = document
        .units
        .iter()
        .filter(|unit| matches!(unit, SceneUnit::PathPoint { .. }))
        .count();
    for (hash, label) in [
        (PATH_POINT_POINTS, "points"),
        (PATH_POINT_TANGENTS, "tangents"),
        (PATH_POINT_INDICES, "indices"),
    ] {
        let actual = array(lot, hash).len();
        if actual != expected_points {
            return Err(format!(
                "written-contract: path {label} column {hash:08X} has {actual} rows, expected {expected_points}"
            ));
        }
    }
    for (index, value) in array(lot, PATH_POINT_INDICES).iter().enumerate() {
        if *value != Value::Int32(index as i32) {
            return Err(format!(
                "written-contract: path global index row {index} is {value:?}, expected {}",
                index as i32
            ));
        }
    }
    for (hash, label) in [
        (PATH_POINT_POINTS, "point"),
        (PATH_POINT_TANGENTS, "tangent"),
    ] {
        for (index, value) in array(lot, hash).iter().enumerate() {
            let Value::Vector3(vector) = value else {
                return Err(format!(
                    "written-contract: path {label} row {index} is not Vector3"
                ));
            };
            if vector.iter().any(|component| !component.is_finite()) {
                return Err(format!(
                    "written-contract: path {label} row {index} is non-finite"
                ));
            }
        }
    }

    let mut global_indices = HashMap::new();
    for (index, unit) in document.units.iter().enumerate() {
        if let SceneUnit::PathPoint { id, .. } = unit {
            let pool_index = document.units[..index]
                .iter()
                .filter(|candidate| matches!(candidate, SceneUnit::PathPoint { .. }))
                .count() as i32;
            if global_indices.insert(id.as_str(), pool_index).is_some() {
                return Err(format!("written-contract: duplicate pathPoint id {id}"));
            }
        }
    }
    let mut expected_pairs = Vec::new();
    let mut expected_entries = Vec::new();
    let mut expected_scopes = Vec::new();
    let mut expected_connects = Vec::new();
    let mut custom_entries = Vec::new();
    let mut builtin_entries = Vec::new();
    for (path_index, path) in document.paths.iter().enumerate() {
        if path.point_ids.len() < 2 {
            return Err(format!(
                "written-contract: path #{path_index} has {} point(s); a usable path needs at least two",
                path.point_ids.len()
            ));
        }
        let mut indices = Vec::new();
        for id in &path.point_ids {
            let Some(index) = global_indices.get(id.as_str()) else {
                return Err(format!(
                    "written-contract: path #{path_index} references missing pathPoint {id}"
                ));
            };
            indices.push(*index);
        }
        indices.sort_unstable();
        expected_pairs.push(Value::Int32(indices[0]));
        expected_pairs.push(Value::Int32(*indices.last().expect("non-empty path")));
        let recipe = super::path_catalog::resolve(
            path.kind.as_deref(),
            path.entry.as_deref(),
            path.scope_local,
            path.road_connect,
        )
        .map_err(|error| format!("written-contract: path #{path_index}: {error}"))?;
        expected_entries.push(Value::Key(Key {
            type_id: PROPERTY_TYPE,
            group: recipe.entry_group,
            instance: recipe.entry_instance,
        }));
        expected_scopes.push(Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: if recipe.scope_local {
                super::native_profile::PATH_SCOPE_LOCAL
            } else {
                super::native_profile::PATH_SCOPE_GLOBAL
            },
        }));
        expected_connects.push(Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: if recipe.road_connect {
                super::native_profile::PATH_ROAD_CONNECT_FIRST
            } else {
                super::native_profile::PATH_ROAD_CONNECT_NONE
            },
        }));
        let entry_id = ResourceId {
            type_id: PROPERTY_TYPE,
            group: recipe.entry_group,
            instance: recipe.entry_instance,
        };
        if recipe.from_catalog {
            builtin_entries.push(format!("{:08X}", recipe.entry_instance));
        } else {
            if !resources.contains_key(&entry_id) {
                return Err(format!(
                    "written-contract: custom PathEntry {entry_id} must be shipped in the final package"
                ));
            }
            custom_entries.push(entry_id.to_string());
        }
    }
    for (hash, label, expected) in [
        (PATH_PAIRS, "pairs", expected_pairs.clone()),
        (PATH_ENTRIES, "entries", expected_entries.clone()),
        (PATH_SCOPES, "scopes", expected_scopes.clone()),
        (PATH_CONNECTS, "road connects", expected_connects.clone()),
    ] {
        let actual = array(lot, hash);
        if actual != expected {
            return Err(format!(
                "written-contract: path {label} column {hash:08X} mismatch: actual {actual:?}, expected {expected:?}"
            ));
        }
    }
    Ok(serde_json::json!({
        "paths": document.paths.len(),
        "points": expected_points,
        "pairs": expected_pairs.len() / 2,
        "builtinExternalEntries": builtin_entries,
        "customPackagedEntries": custom_entries,
    }))
}

fn validate_props(
    lot: &PropertyFile,
    document: &SceneDocument,
    writes: &SceneWrites,
) -> Result<serde_json::Value, String> {
    use sc_properties::lot_unit::{PROP_ID_BASE, PROP_SLOT_BASE, PROP_TRANSFORM_BASE};

    let mut bins = writes.prop_bins.clone();
    if writes.authoritative {
        for unit in &document.units {
            if let SceneUnit::Prop { bin, .. } = unit {
                bins.push(*bin);
            }
        }
    }
    bins.sort_unstable();
    bins.dedup();
    let mut report = serde_json::Map::new();
    for bin in bins {
        let expected: Vec<Option<u32>> = document
            .units
            .iter()
            .filter_map(|unit| match unit {
                SceneUnit::Prop {
                    bin: candidate,
                    resource_id,
                    ..
                } if *candidate == bin => Some(*resource_id),
                _ => None,
            })
            .collect();
        let ids = array(lot, PROP_ID_BASE + bin as u32);
        let transforms = array(lot, PROP_TRANSFORM_BASE + bin as u32);
        let slots = array(lot, PROP_SLOT_BASE + bin as u32);
        if ids.len() != expected.len() || transforms.len() != expected.len() {
            return Err(format!(
                "written-contract: prop bin {bin} ids/transforms lengths {}/{} != expected {}",
                ids.len(),
                transforms.len(),
                expected.len()
            ));
        }
        if !slots.is_empty() && slots.len() != expected.len() {
            return Err(format!(
                "written-contract: prop bin {bin} slot column has {} rows, expected {}",
                slots.len(),
                expected.len()
            ));
        }
        for (index, (value, expected_id)) in ids.iter().zip(expected.iter()).enumerate() {
            let Value::Key(key) = value else {
                return Err(format!(
                    "written-contract: prop bin {bin} row {index} id is not a Key"
                ));
            };
            if Some(key.instance) != *expected_id {
                return Err(format!(
                    "written-contract: prop bin {bin} row {index} resource {:08X} != authored {:?}",
                    key.instance,
                    expected_id.map(|id| format!("{id:08X}"))
                ));
            }
        }
        report.insert(
            bin.to_string(),
            serde_json::json!({"count": expected.len()}),
        );
    }
    Ok(serde_json::Value::Object(report))
}

fn validate_models(
    resources: &Resources,
    lot: &PropertyFile,
    model_ids: &LodModelIds,
    strict_author: bool,
) -> Result<serde_json::Value, String> {
    let unique: HashSet<ResourceId> = model_ids.iter().copied().collect();
    if unique.len() != 4 {
        return Err(
            "written-contract: LOD1..LOD4 must reference four distinct model resources".into(),
        );
    }
    let mut validated_slots = HashSet::new();
    let mut reports = Vec::new();
    let mut lod1_bounds = None;
    for (index, (hash, expected_id)) in LOD_MODEL_HASHES.into_iter().zip(model_ids).enumerate() {
        let key = scalar_key(lot, hash).ok_or_else(|| {
            format!(
                "written-contract: LOD{} model key {hash:08X} is missing",
                index + 1
            )
        })?;
        let actual_id = ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        };
        if actual_id != *expected_id {
            return Err(format!(
                "written-contract: LOD{} points to {actual_id}, expected {expected_id}",
                index + 1
            ));
        }
        let bytes = resources.get(expected_id).ok_or_else(|| {
            format!(
                "written-contract: LOD{} model {expected_id} is missing",
                index + 1
            )
        })?;
        let file = Rw4File::parse(bytes).map_err(|error| {
            format!(
                "written-contract: LOD{} model {expected_id}: {error}",
                index + 1
            )
        })?;
        let meshes: Vec<_> = file.sections_of_type(SectionType::MESH).collect();
        if (strict_author && meshes.len() != 1) || (!strict_author && meshes.is_empty()) {
            return Err(format!(
                "written-contract: LOD{} model {expected_id} has {} meshes; {} required",
                index + 1,
                meshes.len(),
                if strict_author {
                    "exactly one Building40 mesh"
                } else {
                    "at least one mesh"
                }
            ));
        }
        let bindings = file.decode_mesh_material_bindings(bytes);
        if bindings.len() != meshes.len() {
            return Err(format!(
                "written-contract: LOD{} has {} meshes but {} material bindings",
                index + 1,
                meshes.len(),
                bindings.len()
            ));
        }
        for binding in &bindings {
            if !meshes
                .iter()
                .any(|section| section.number == binding.mesh_section)
                || file.payload(bytes, binding.material_section).is_err()
            {
                return Err(format!(
                    "written-contract: LOD{} has dangling mesh/material binding {:?}",
                    index + 1,
                    binding
                ));
            }
        }

        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        let mut degenerate = 0usize;
        for section in &meshes {
            let mesh = file.decode_mesh(bytes, section.number).map_err(|error| {
                format!(
                    "written-contract: LOD{} mesh #{}: {error}",
                    index + 1,
                    section.number
                )
            })?;
            if !mesh.is_exportable() {
                return Err(format!(
                    "written-contract: LOD{} mesh #{} is empty or not exportable",
                    index + 1,
                    section.number
                ));
            }
            vertices += mesh.vertices.len();
            triangles += mesh.triangles.len();
            for vertex in &mesh.vertices {
                let position = vertex.position().ok_or_else(|| {
                    format!(
                        "written-contract: LOD{} has a vertex without POSITION",
                        index + 1
                    )
                })?;
                if position.iter().any(|value| !value.is_finite()) {
                    return Err(format!(
                        "written-contract: LOD{} has non-finite POSITION",
                        index + 1
                    ));
                }
                for axis in 0..3 {
                    min[axis] = min[axis].min(position[axis]);
                    max[axis] = max[axis].max(position[axis]);
                }
                if strict_author {
                    validate_author_vertex(vertex, index + 1)?;
                }
            }
            if strict_author {
                for triangle in &mesh.triangles {
                    let [a, b, c] = *triangle;
                    if a == b || b == c || c == a {
                        degenerate += 1;
                        continue;
                    }
                    let pa = mesh.vertices[a as usize]
                        .position()
                        .expect("position checked");
                    let pb = mesh.vertices[b as usize]
                        .position()
                        .expect("position checked");
                    let pc = mesh.vertices[c as usize]
                        .position()
                        .expect("position checked");
                    let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
                    let ac = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
                    let cross = [
                        ab[1] * ac[2] - ab[2] * ac[1],
                        ab[2] * ac[0] - ab[0] * ac[2],
                        ab[0] * ac[1] - ab[1] * ac[0],
                    ];
                    if cross.iter().map(|value| value * value).sum::<f32>() <= 1e-20 {
                        degenerate += 1;
                    }
                }
            }
        }
        if strict_author && degenerate != 0 {
            return Err(format!(
                "written-contract: LOD{} contains {degenerate} degenerate triangles",
                index + 1
            ));
        }
        if strict_author && (0..3).any(|axis| max[axis] - min[axis] <= 1e-6) {
            return Err(format!(
                "written-contract: LOD{} has a flat/degenerate bounds {:?}..{:?}",
                index + 1,
                min,
                max
            ));
        }
        if index == 0 {
            lod1_bounds = Some((min, max));
        }

        let material_sections: Vec<_> = file.sections_of_type(SectionType::MATERIAL).collect();
        if strict_author && material_sections.len() != 1 {
            return Err(format!(
                "written-contract: LOD{} has {} materials; Building40 requires exactly one",
                index + 1,
                material_sections.len()
            ));
        }
        let mut slot_instances = [0u32; 6];
        for section in &material_sections {
            let material = file
                .decode_material(bytes, section.number)
                .map_err(|error| {
                    format!(
                        "written-contract: LOD{} material #{}: {error}",
                        index + 1,
                        section.number
                    )
                })?;
            let MaterialSection::Decoded(material) = material else {
                return Err(format!(
                    "written-contract: LOD{} material #{} has an unknown layout",
                    index + 1,
                    section.number
                ));
            };
            for slot in 0..=5u8 {
                let rows: Vec<_> = material
                    .texture_slots()
                    .filter(|row| row.slot_byte() == slot)
                    .collect();
                if strict_author && rows.len() != 1 {
                    return Err(format!(
                        "written-contract: LOD{} material #{} slot{} has {} rows, expected one",
                        index + 1,
                        section.number,
                        slot,
                        rows.len()
                    ));
                }
                for row in rows {
                    if row.texture_instance == 0 {
                        if strict_author {
                            return Err(format!(
                                "written-contract: LOD{} slot{} is unbound",
                                index + 1,
                                slot
                            ));
                        }
                        continue;
                    }
                    let expected_type = if matches!(slot, 0 | 4 | 5) {
                        MODEL_TYPE
                    } else {
                        RASTER_TYPE
                    };
                    let resource_id = ResourceId {
                        type_id: expected_type,
                        group: 0,
                        instance: row.texture_instance,
                    };
                    if validated_slots.insert(resource_id) {
                        validate_slot_payload(resources, resource_id, slot)?;
                    }
                    slot_instances[slot as usize] = row.texture_instance;
                }
            }
        }
        if strict_author && slot_instances.iter().any(|instance| *instance == 0) {
            return Err(format!(
                "written-contract: LOD{} six-slot table is incomplete",
                index + 1
            ));
        }
        reports.push(serde_json::json!({
            "lod": index + 1,
            "instance": format!("{:08X}", expected_id.instance),
            "meshes": meshes.len(),
            "vertices": vertices,
            "triangles": triangles,
            "degenerateTriangles": degenerate,
            "bounds": [min, max],
            "slots": slot_instances.map(|instance| format!("{instance:08X}")),
        }));
    }
    if strict_author {
        let (min, max) = lod1_bounds.expect("LOD1 bounds recorded");
        let asset_bounds = match lot.get(0x00F9_EFBA).map(|property| &property.kind) {
            Some(Kind::Scalar(Value::BoundingBox { min, max })) => Some((*min, *max)),
            Some(Kind::Array(values)) => values.iter().find_map(|value| match value {
                Value::BoundingBox { min, max } => Some((*min, *max)),
                _ => None,
            }),
            _ => None,
        }
        .ok_or("written-contract: Asset BoundingBox is missing for Building40")?;
        let close =
            |a: [f32; 3], b: [f32; 3]| (0..3).all(|axis| (a[axis] - b[axis]).abs() <= 0.002);
        if !close(asset_bounds.0, min) || !close(asset_bounds.1, max) {
            return Err(format!(
                "written-contract: Asset BoundingBox {:?} does not match LOD1 geometry {:?}",
                asset_bounds,
                (min, max)
            ));
        }
    }
    Ok(serde_json::Value::Array(reports))
}

fn validate_author_vertex(vertex: &rw4::DecodedVertex, lod: usize) -> Result<(), String> {
    for (direction, label) in [(vertex.normal(), "NORMAL"), (vertex.tangent(), "TANGENT")] {
        let direction =
            direction.ok_or_else(|| format!("written-contract: LOD{lod} vertex lacks {label}"))?;
        if direction.iter().any(|value| !value.is_finite())
            || direction.iter().map(|value| value * value).sum::<f32>() <= 1e-12
        {
            return Err(format!(
                "written-contract: LOD{lod} has invalid/zero {label}"
            ));
        }
    }
    let uv = vertex
        .uv()
        .ok_or_else(|| format!("written-contract: LOD{lod} vertex lacks UV"))?;
    if uv.iter().any(|value| !value.is_finite()) {
        return Err(format!("written-contract: LOD{lod} has non-finite UV"));
    }
    Ok(())
}

fn validate_slot_payload(resources: &Resources, id: ResourceId, slot: u8) -> Result<(), String> {
    let bytes = resources.get(&id).ok_or_else(|| {
        format!(
            "written-contract: material slot{} resource {id} is missing",
            slot
        )
    })?;
    if bytes.is_empty() {
        return Err(format!(
            "written-contract: material slot{} resource {id} is empty",
            slot
        ));
    }
    if id.type_id == RASTER_TYPE {
        let raster = rw4::RasterImage::parse(bytes)
            .map_err(|error| format!("written-contract: slot{} raster {id}: {error}", slot))?;
        let pixels = raster
            .decode_top_mip_rgba()
            .map_err(|error| format!("written-contract: slot{} raster {id}: {error}", slot))?;
        if raster.width == 0 || raster.height == 0 || pixels.is_empty() {
            return Err(format!(
                "written-contract: slot{} raster {id} has no pixels",
                slot
            ));
        }
    } else {
        let texture = Rw4File::parse(bytes)
            .map_err(|error| format!("written-contract: slot{} texture {id}: {error}", slot))?;
        if texture.file_type() != rw4::FileType::Texture
            || texture.sections_of_type(SectionType::TEXTURE).count() != 1
        {
            return Err(format!(
                "written-contract: slot{} resource {id} is not a single native texture",
                slot
            ));
        }
    }
    Ok(())
}

fn scalar_key(file: &PropertyFile, hash: u32) -> Option<Key> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Key(key))) => Some(*key),
        _ => None,
    }
}

fn array(file: &PropertyFile, hash: u32) -> &[Value] {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Array(values)) => values,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::lot_unit::{PATH_PAIRS, PROP_ID_BASE};
    use serde_json::json;

    fn scene() -> SceneDocument {
        super::super::scene::parse_document(&json!({
            "units": [
                {"id":"pathPoint:0","kind":"pathPoint","point":[0,0,0],"tangent":[1,0,0]},
                {"id":"pathPoint:1","kind":"pathPoint","point":[4,0,0],"tangent":[1,0,0]},
                {"id":"pathPoint:2","kind":"pathPoint","point":[8,0,0],"tangent":[1,0,0]},
                {"id":"prop:1:0","kind":"prop","bin":1,"resourceId":42,
                 "transform":{"matrix":[1,0,0,0,1,0,0,0,1,0,0,0],"flags":15}}
            ],
            "editor":{"workbench":{"paths":[
                {"type":"pedestrian","pointIds":["pathPoint:0","pathPoint:1"]},
                {"type":"pedestrian","pointIds":["pathPoint:1","pathPoint:2"]}
            ]}}
        }))
        .unwrap()
    }

    fn writes() -> SceneWrites {
        SceneWrites {
            paths: true,
            prop_bins: vec![1],
            ..Default::default()
        }
    }

    fn lot(document: &SceneDocument, writes: &SceneWrites) -> PropertyFile {
        let mut file = PropertyFile::default();
        super::super::scene::apply_units(&mut file, document, writes).unwrap();
        file
    }

    #[test]
    fn final_scene_accepts_shared_global_point_pool() {
        let document = scene();
        let file = lot(&document, &writes());
        let report = validate_scene(&Resources::new(), &file, &document, &writes()).unwrap();
        assert_eq!(report.pointer("/paths/points"), Some(&json!(3)));
        assert_eq!(report.pointer("/paths/paths"), Some(&json!(2)));
        assert_eq!(report.pointer("/props/1/count"), Some(&json!(1)));
    }

    #[test]
    fn final_scene_rejects_missing_path_pairs() {
        let document = scene();
        let mut file = lot(&document, &writes());
        file.values.retain(|property| property.hash != PATH_PAIRS);
        let error = validate_scene(&Resources::new(), &file, &document, &writes()).unwrap_err();
        assert!(error.contains("pairs"), "{error}");
    }

    #[test]
    fn final_scene_rejects_missing_prop_column() {
        let document = scene();
        let mut file = lot(&document, &writes());
        file.values
            .retain(|property| property.hash != PROP_ID_BASE + 1);
        let error = validate_scene(&Resources::new(), &file, &document, &writes()).unwrap_err();
        assert!(error.contains("prop bin 1"), "{error}");
    }

    #[test]
    fn final_scene_requires_custom_path_entry_in_package() {
        let mut document = scene();
        document.paths[0].kind = Some("custom".into());
        document.paths[0].entry = Some("1234ABCD".into());
        let file = lot(&document, &writes());
        let error = validate_scene(&Resources::new(), &file, &document, &writes()).unwrap_err();
        assert!(error.contains("custom PathEntry"), "{error}");
    }

    fn source_ground_case() -> (
        SceneDocument,
        Resources,
        ResourceId,
        PropertyFile,
        Resources,
        ResourceId,
    ) {
        let document = super::super::scene::parse_document(&json!({
            "lot": {
                "size": [80, 80],
                "tilePeriod": [15, 15],
                "baseTile": 2,
                "mask": {"kind": "source"}
            }
        }))
        .unwrap();
        let source_lot = ResourceId {
            type_id: PROPERTY_TYPE,
            group: 0x41F0_C000,
            instance: 0x0483_239E,
        };
        let source_mask = ResourceId {
            type_id: RASTER_TYPE,
            group: 0,
            instance: 0xDEDC_4095,
        };
        let final_mask = ResourceId {
            type_id: RASTER_TYPE,
            group: 0,
            instance: 0x1234_5678,
        };
        let source_lot_bytes = PropertyFile {
            claimed_count: 1,
            values: vec![sc_properties::Property {
                hash: LOT_MASK_HASH,
                prop_type: sc_properties::PropType::Key,
                kind: Kind::Scalar(Value::Key(Key {
                    type_id: 0,
                    group: 0,
                    instance: source_mask.instance,
                })),
                encoding: sc_properties::PropertyEncoding::default(),
            }],
        }
        .encode_canonical()
        .unwrap();
        let pixels = (0..12)
            .map(|index| [index as u8, 200 - index as u8, index as u8 * 5, 0])
            .collect::<Vec<_>>();
        let raster = super::super::materials::encode_rgba_raster(4, 3, &pixels).unwrap();
        let source = Resources::from([
            (source_lot, source_lot_bytes.clone()),
            (source_mask, raster.clone()),
        ]);
        let (final_lot_bytes, _) = super::super::package::apply_scene_lot_semantics(
            &source_lot_bytes,
            Some(&document),
            None,
            &SceneWrites::default(),
        )
        .unwrap()
        .unwrap();
        let mut final_lot = PropertyFile::parse(&final_lot_bytes).unwrap();
        let mask_property = final_lot
            .values
            .iter_mut()
            .find(|property| property.hash == LOT_MASK_HASH)
            .unwrap();
        mask_property.kind = Kind::Scalar(Value::Key(Key {
            type_id: 0,
            group: 0,
            instance: final_mask.instance,
        }));
        let final_lot = PropertyFile::parse(&final_lot.encode_canonical().unwrap()).unwrap();
        let final_resources = Resources::from([(final_mask, raster)]);
        (
            document,
            source,
            source_lot,
            final_lot,
            final_resources,
            final_mask,
        )
    }

    #[test]
    fn final_ground_accepts_exact_source_pixels_and_canonical_style() {
        let (document, source, source_lot, final_lot, final_resources, _) = source_ground_case();
        let (mask, style) = validate_ground(
            &final_resources,
            &final_lot,
            &document,
            Some((&source, source_lot)),
        )
        .unwrap();
        assert_eq!(mask.unwrap().pointer("/final/width"), Some(&json!(4)));
        assert_eq!(
            style.unwrap().pointer("/fields/1/hash"),
            Some(&json!("0CCB7FD6"))
        );
        assert_eq!(
            final_lot.get(0x0CCB_7FD6).unwrap().prop_type,
            sc_properties::PropType::Int32
        );
    }

    #[test]
    fn final_ground_rejects_old_white_synthetic_substitution() {
        let (document, source, source_lot, final_lot, mut final_resources, final_mask) =
            source_ground_case();
        let white = vec![[255, 255, 255, 255]; 80 * 80];
        final_resources.insert(
            final_mask,
            super::super::materials::encode_rgba_raster(80, 80, &white).unwrap(),
        );
        let error = validate_ground(
            &final_resources,
            &final_lot,
            &document,
            Some((&source, source_lot)),
        )
        .unwrap_err();
        assert!(error.contains("pixel mismatch"), "{error}");
        final_resources.clear();
        let error = validate_ground(
            &final_resources,
            &final_lot,
            &document,
            Some((&source, source_lot)),
        )
        .unwrap_err();
        assert!(error.contains("final LotMask raster is missing"), "{error}");
    }
}
