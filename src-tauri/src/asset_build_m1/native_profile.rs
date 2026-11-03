//! Parent-free property identity (the author backend's `profile` route).
//!
//! Port of the external packager 0.8.0 chain:
//! - `flat_chain` materializes the generic schema chains (00B2CCCB walk,
//!   first record wins, parent edge dropped) — `independent_park_profile.flat`;
//! - the Asset dictionary (group 40E1C000) receives every Lot numeric field
//!   via `encode_lot_numeric` (`native_property_codec.LOT_TYPES`), the
//!   rotation-transformed ground raster/box, and the compiled prop/light
//!   instance columns (`prop_contract` / `light_contract`);
//! - the Unit leaf is the author's `tested-unit.bin` generic cUnit with
//!   cost/maintenance/category overrides (`backend_compile.profile`);
//! - props and lights are NOT lowered into a Lot property here (legacy
//!   route); they are written into the Asset dictionary exactly like the
//!   author's `compile_fields`.
//!
//! Menu/icon/locale/marquee resources stay with the game-verified `menu.rs`
//! shape; this module builds only the Unit + Asset dictionary identities.

use super::connector;
use super::menu::MenuRegistrationSpec;
use super::package::LodModelIds;
use super::scene::{SceneDocument, SceneUnit};
use dbpf::ResourceId;
use sc_properties::{
    Key, Kind, Property, PropertyEncoding, PropertyFile, PropType, Text, Transform, Value,
};
use sha2::{Digest, Sha256};

/// Generic Asset dictionary schema chain root (`independent_park_profile`).
/// The interaction/UI fields (`0AD90138`, `0AFE2FEB`, UI_FIELDS) are restored
/// separately from the `34CE7E14` chain — see INTERACTION_FIELDS.
pub const GENERIC_ASSET: ResourceId = ResourceId {
    type_id: 0x00B1_B104,
    group: 0x40E1_C000,
    instance: 0x9EC4_74FF,
};
/// The lot-group twin of the generic unit template: the top of the
/// interaction chain that converges through `9EC474FF`.
pub const INTERACTION_CHAIN: ResourceId = ResourceId {
    type_id: 0x00B1_B104,
    group: 0x40E1_C000,
    instance: 0x34CE_7E14,
};
/// Author-selected interaction/query fields restored from the `34CE7E14`
/// chain (`interactive_park_profile`: `[0x0ad90138, 0x0afe2feb, *UI_FIELDS]`).
const INTERACTION_FIELDS: [u32; 15] = [
    0x0AD9_0138, // generic interaction flag (must be true; author byte-verifies)
    0x0AFE_2FEB,
    0x0A8E_881D,
    0x0A8E_881E,
    0x0CAB_B2CF,
    0x0CAB_B2FC,
    0x0CAF_BDC3,
    0x0CD4_E49C,
    0x0D7E_ADAC,
    0x0E54_4C70,
    0x0E7A_6FDE,
    0x0D79_70ED,
    0x0E8B_A0C6,
    0x0E8B_A0C7,
    0x0CA5_2005,
];
/// Generic menu schema chain root (presence check only; the Menu2 itself is
/// the game-verified minimal shape in `menu.rs`).
pub const GENERIC_MENU: ResourceId = ResourceId {
    type_id: 0x00B1_B104,
    group: 0x48E1_EB00,
    instance: 0xF970_A822,
};
/// Asset dictionary property group (the author's `asset_key` group).
pub const ASSET_GROUP: u32 = 0x40E1_C000;
/// Parent edge hash (00B2CCCB) — dropped when materializing a chain.
const HASH_PARENT: u32 = 0x00B2_CCCB;
/// cUnit → Asset dictionary edge (author `unit[0x08e17ed7]=key(asset_key)`).
const HASH_UNIT_ASSET: u32 = 0x08E1_7ED7;
/// Unit/Asset locale title edge (author `text(0x09fb78cb, table, 1)`).
const HASH_TITLE_LINK: u32 = 0x09FB_78CB;
/// Cost / maintenance per hour (author floatprop overrides on the unit).
const HASH_COST: u32 = 0x0A8A_41DA;
const HASH_MAINTENANCE: u32 = 0x09AE_19D7;
/// Unit simulation classification (author: category instance, type-less).
const HASH_UNIT_CATEGORY: u32 = 0x0AFE_27B1;
/// LOD model references on the Asset dictionary.
pub const LOD_HASHES: [u32; 4] = [0x00F9_EFBB, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE];
/// Asset model bounds (author `array(0x00f9efba, 57, [6f], 24, 0x801c)`).
const HASH_MODEL_BOUNDS: u32 = 0x00F9_EFBA;
/// Ground parcel size / overlay offset / tiling period / base tile.
const HASH_LOT_SIZE: u32 = 0x0CCB_7FC8;
const HASH_LOT_OFFSET: u32 = 0x0CCB_7FC9;
const HASH_LOT_PERIOD: u32 = 0x0CCB_7FD0;
const HASH_BASE_TILE: u32 = 0x0CCB_7FD6;
/// Ground mask raster reference (author `asset[0x0ccb7fd5]=key(mask_key)`).
pub const GROUND_MASK_HASH: u32 = 0x0CCB_7FD5;
/// Ground box (author `array(0x0fe01484, 57, [6f], 24, 0x801c)`).
const HASH_GROUND_BOX: u32 = 0x0FE0_1484;
/// Ground plane width/depth floats (author floatprops `0BBBCDED`/`0BBBCE2A`).
const HASH_GROUND_WIDTH: u32 = 0x0BBB_CDED;
const HASH_GROUND_DEPTH: u32 = 0x0BBB_CE2A;
/// Native ground atlas models required by the author chain
/// (`require((MODEL,0,0xa0da9e6c) in inventory …)`).
pub const GROUND_ATLAS_A: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0xA0DA_9E6C,
};
pub const GROUND_ATLAS_B: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0x60E7_805D,
};
/// Lot numeric fields re-encoded into the Asset dictionary
/// (`native_property_codec.LOT_TYPES`).
const LOT_TYPES: &[(u32, PropType)] = &[
    (0x0CCB_7FC8, PropType::Vector2),
    (0x0CCB_7FC9, PropType::Vector2),
    (0x0CCB_7FD0, PropType::Vector2),
    (0x0CCB_7FD6, PropType::Int32),
    (0x0D02_D586, PropType::ColorRgba),
    (0x0D02_D587, PropType::ColorRgba),
    (0x0D02_D588, PropType::ColorRgba),
    (0x0D02_D589, PropType::ColorRgba),
    (0x0D02_D58A, PropType::Vector4),
    (0x0D02_D58B, PropType::Vector4),
    (0x0D7A_F042, PropType::ColorRgba),
    (0x0D7A_F043, PropType::ColorRgba),
    (0x0D7A_F044, PropType::ColorRgba),
    (0x0D7A_F045, PropType::ColorRgba),
    (0x0D7A_F046, PropType::Float),
    (0x0D7A_F047, PropType::Float),
    (0x0D7A_F048, PropType::Float),
    (0x0D7A_F049, PropType::Float),
];
/// Ground key fields exempt from the numeric whitelist (author
/// `require(h in ('0CCB7FD4','0CCB7FD5','0D0082F1'))`).
const GROUND_MODEL_KEY: u32 = 0x0CCB_7FD4;
const GROUND_ATLAS_KEY: u32 = 0x0D00_82F1;

// -- prop_contract.compile_fields ------------------------------------------

/// Prop instance-array field hashes (author `202567456+bin … 202567520+bin`).
const PROP_RESOURCE_KEY: u32 = 0x0C12_EF20;
const PROP_TRANSFORMS: u32 = 0x0C12_EF30;
const PROP_INDICES: u32 = 0x0C12_EF40;
const PROP_MARKERS: [u32; 2] = [0x0C12_EF50, 0x0C12_EF60];

// -- light_contract.compile_fields -----------------------------------------

/// Light resource group / night profile constants (`light_contract.GROUP`,
/// `light_native_profile.json`). `pub(crate)`: scene.rs reuses these instead
/// of hand-copying — a transcribed copy (0x1295_6239 for 0x1295_2639) kept
/// every light dark in University v9-v11. The decimal pins live in the
/// `light_profile_constants_match_author_json` test.
pub(crate) const LIGHT_GROUP: u32 = 0x8E43_F125;
pub(crate) const NIGHT_PERIOD: u32 = 0x1295_2639;
const NIGHT_RANGE: [f32; 2] = [0.875, 0.215_273_01];
const CULL_TIER: u32 = 0x3E7E_124D;
const LIGHT_POINT_TYPE: u32 = 0x75D4_C8CD;
const LIGHT_SPOT_TYPE: u32 = 0x2F0F_F9FD;
/// Line lights are vanilla-proven (fire station asset dict E917279C light #3:
/// type 0x0820ABAF, radius 5 / inner 1 / diffuse 0.5 / length 35) — the
/// author packager only validates POINT/SPOT, but coercing schema LINE to
/// POINT (pre-s2 behavior) turned authored 60 m facade strips into two faint
/// blobs. Pass LINE through verbatim.
const LIGHT_LINE_TYPE: u32 = 0x0820_ABAF;
/// Per-instance light columns: `(hash, prop_type)`.
const LIGHT_COLUMNS: [(u32, PropType); 14] = [
    (0x0CAA_8F10, PropType::Key),       // [0,0,GROUP]
    (0x0CAA_8F11, PropType::Key),       // [0,0,point|spot type]
    (0x0CAA_8F12, PropType::Transform), // flags 14 + 12f matrix
    (0x0CAA_8F13, PropType::ColorRgb),  // color
    (0x0CAA_8F14, PropType::Float),     // radius
    (0x0CAA_8F15, PropType::Float),     // innerRadius
    (0x0CAA_8F16, PropType::Float),     // intensity
    (0x0CAA_8F17, PropType::String8),   // "SC5_"+id[:24]
    (0x0D4C_96B4, PropType::Float),     // specular
    (0x0D4C_AD23, PropType::Float),     // length
    (0x0E01_200F, PropType::Key),       // [0,0,cullTier]
    (0x0E4D_F244, PropType::Float),     // 0.0
    (0x0E98_D445, PropType::Bool),      // false
    (0x0EC4_567E, PropType::Float),     // 0.0
];
/// Fixed light profile fields `(hash, prop_type)`.
const LIGHT_FIXED_GROUP: (u32, PropType) = (0x0CE7_1B10, PropType::Key);
const LIGHT_FIXED_NIGHT_PERIOD: (u32, PropType) = (0x0CE7_1B11, PropType::Key);
const LIGHT_FIXED_NIGHT_RANGE: (u32, PropType) = (0x0CE7_1B12, PropType::Vector2);
const LIGHT_FIXED_ZEROS: (u32, PropType) = (0x0CE7_1B13, PropType::Vector2);

/// Parent-free identity output.
pub struct ParentFreeIdentity {
    /// Asset dictionary (40E1C000, new instance) with ground numerics,
    /// props/lights, LOD chain and ground box.
    pub asset: PropertyFile,
    /// Generic cUnit leaf (40E0C000, new instance, `tested-unit.bin` clone).
    pub unit: PropertyFile,
    /// Rotation-transformed ground raster for the new mask instance.
    pub mask: Option<(ResourceId, Vec<u8>)>,
    /// Build-report evidence (generic chain sources, instance counts).
    pub evidence: serde_json::Value,
}

/// Materialize a generic schema chain: walk `00B2CCCB` parents collecting the
/// first occurrence of every field (author `flat()`); the parent edge itself
/// is dropped.
pub(crate) fn flat_chain(
    resources: &connector::Resources,
    start: ResourceId,
) -> Result<(PropertyFile, Vec<String>), String> {
    let mut records: PropertyFile = PropertyFile::default();
    let mut seen: std::collections::HashSet<ResourceId> = std::collections::HashSet::new();
    let mut sources = Vec::new();
    let mut cursor = Some(start);
    while let Some(key) = cursor {
        if !seen.insert(key) {
            return Err(format!("cyclic generic schema at {key}"));
        }
        let bytes = resources.get(&key).ok_or_else(|| {
            format!(
                "generic schema chain is missing {key}; the reference package must contain the game's generic templates"
            )
        })?;
        let file = PropertyFile::parse(bytes).map_err(|e| format!("generic schema {key}: {e}"))?;
        sources.push(format!("{:08X}:{:08X}:{:08X}", key.type_id, key.group, key.instance));
        cursor = None;
        for property in file.values {
            if property.hash == HASH_PARENT {
                let parent = match &property.kind {
                    Kind::Scalar(value) => Some(value),
                    Kind::Array(values) => values.first(),
                    Kind::Empty => None,
                };
                if let Some(Value::Key(parent)) = parent {
                    cursor = Some(ResourceId {
                        type_id: parent.type_id,
                        group: parent.group,
                        instance: parent.instance,
                    });
                }
                continue;
            }
            if records.get(property.hash).is_none() {
                records.values.push(property);
            }
        }
    }
    records.claimed_count = records.values.len() as u32;
    Ok((records, sources))
}

fn upsert(file: &mut PropertyFile, property: Property) {
    match file.values.iter_mut().find(|p| p.hash == property.hash) {
        Some(existing) => *existing = property,
        None => file.values.push(property),
    }
}

fn scalar(hash: u32, prop_type: PropType, value: Value) -> Property {
    Property {
        hash,
        prop_type,
        kind: Kind::Scalar(value),
        encoding: PropertyEncoding::default(),
    }
}

fn array(hash: u32, prop_type: PropType, values: Vec<Value>) -> Property {
    // Author-declared item sizes (light_native_profile.json / build_candidate
    // array calls): the game loader validates the declared step even though
    // each value serializes to the canonical size.
    let item_size = match prop_type {
        PropType::Key => Some(12),
        PropType::Float | PropType::Int32 | PropType::UInt32 => Some(4),
        PropType::Bool => Some(1),
        PropType::ColorRgb | PropType::Vector3 => Some(12),
        PropType::Vector2 => Some(8),
        PropType::String8 => Some(16),
        PropType::Transform => Some(56),
        PropType::BoundingBox => Some(24),
        PropType::Text => Some(sc_properties::TEXT_ARRAY_ITEM_SIZE),
        _ => None,
    };
    Property {
        hash,
        prop_type,
        kind: Kind::Array(values),
        encoding: PropertyEncoding {
            flags: 0,
            array_item_size: item_size,
        },
    }
}

fn key_value(key: Key) -> Value {
    Value::Key(key)
}

fn zero_key(instance: u32) -> Value {
    key_value(Key {
        type_id: 0,
        group: 0,
        instance,
    })
}

fn text_value() -> Value {
    Value::Text(Text {
        table_id: super::menu::LOCALE_TABLE,
        instance_id: 1,
    })
}

fn vector2(property: Option<&Property>) -> Option<[f32; 2]> {
    match property.and_then(|p| p.scalar()) {
        Some(Value::Vector2(value)) => Some(*value),
        _ => None,
    }
}

/// `encode_lot_numeric` (`native_property_codec.py`): regenerate a Lot
/// numeric field from its typed value, refusing anything outside the
/// verified profile.
fn encode_lot_numeric(property: &Property) -> Result<Property, String> {
    let expected = LOT_TYPES
        .iter()
        .find(|(hash, _)| *hash == property.hash)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| format!("unsupported Lot numeric field 0x{:08X}", property.hash))?;
    if property.prop_type != expected {
        return Err(format!(
            "Lot field 0x{:08X}: expected type {}, found {}",
            property.hash,
            expected.name(),
            property.prop_type.name()
        ));
    }
    if property.encoding.flags != 0 && property.encoding.flags & 0x30 != 0 {
        return Err(format!(
            "Lot numeric field 0x{:08X} must be scalar",
            property.hash
        ));
    }
    Ok(property.clone())
}

/// `asset_transforms.rotate_lot`: rotate the ground mask CCW by
/// `degrees/90` quarters and transform size/offset/period accordingly.
fn rotate_lot(
    mask: &image::RgbaImage,
    size: [f32; 2],
    offset: [f32; 2],
    period: [f32; 2],
    degrees: u32,
) -> Result<(image::RgbaImage, [f32; 2], [f32; 2], [f32; 2]), String> {
    let swapped = matches!(degrees, 90 | 270);
    let mut new_size = size;
    let mut new_period = period;
    if swapped {
        new_size = [size[1], size[0]];
        new_period = [period[1], period[0]];
    }
    let matrix = super::native_model::rotate_xyz_matrix(degrees)?;
    let rotated_offset = super::native_model::rotate_point(&matrix, [offset[0], offset[1], 0.0]);
    let quarters = degrees / 90;
    let mut out = mask.clone();
    for _ in 0..quarters {
        out = rot90_ccw(&out);
    }
    Ok((out, new_size, [rotated_offset[0], rotated_offset[1]], new_period))
}

/// `np.rot90` counter-clockwise: `out[i][j] = in[j][rows-1-i]`.
fn rot90_ccw(image: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (image.width(), image.height());
    let mut out = image::RgbaImage::new(h, w);
    for i in 0..h {
        for j in 0..w {
            out.put_pixel(i, j, *image.get_pixel(j, h - 1 - i));
        }
    }
    out
}

/// Validate and decode the ground mask raster (author `profile()` checks,
/// then `raw[28:][h,w,4][:,:,[2,1,0,3]]`).
fn decode_ground_raster(raw: &[u8]) -> Result<image::RgbaImage, String> {
    let be_u32 = |at: usize| -> Result<u32, String> {
        raw.get(at..at + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .ok_or_else(|| "truncated ground raster".to_string())
    };
    let version = be_u32(0)?;
    let width = be_u32(4)?;
    let height = be_u32(8)?;
    let kind = be_u32(16)?;
    let format = be_u32(20)?;
    // The author required w==h because their exporter only produced square
    // parcels. Native rectangular lots carry rectangular masks (the university
    // 192×96m lot ships 256×128), so validate per dimension instead.
    let pow2 = |v: u32| v & (v - 1) == 0;
    if version != 2
        || !(16..=2048).contains(&width)
        || !(16..=2048).contains(&height)
        || !pow2(width)
        || !pow2(height)
    {
        return Err("unsupported ground raster dimensions".into());
    }
    if format != 21 || kind != 8 {
        return Err("unsupported ground raster encoding".into());
    }
    let top_mip = be_u32(24)?;
    if top_mip != width * height * 4 || raw.len() < 28 + width as usize * height as usize * 4 {
        return Err("truncated ground raster".into());
    }
    let mut out = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let at = 28 + ((y * width + x) * 4) as usize;
            out.put_pixel(x, y, image::Rgba([raw[at + 2], raw[at + 1], raw[at], raw[at + 3]]));
        }
    }
    Ok(out)
}

fn resolve_raster(resources: &connector::Resources, key: &Key) -> Result<Vec<u8>, String> {
    let raster = super::materials::raster_type();
    let candidates = [
        ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: raster,
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: raster,
            group: 0,
            instance: key.instance,
        },
    ];
    candidates
        .iter()
        .find_map(|id| resources.get(id).cloned())
        .ok_or_else(|| {
            format!(
                "ground mask raster 0x{:08X} is not in the reference package",
                key.instance
            )
        })
}

fn alloc_raster_instance(resources: &connector::Resources, bytes: &[u8]) -> u32 {
    let mut instance = u32::from_be_bytes(Sha256::digest(bytes)[..4].try_into().unwrap());
    let raster = super::materials::raster_type();
    while resources.contains_key(&ResourceId {
        type_id: raster,
        group: 0,
        instance,
    }) {
        instance = instance.wrapping_add(1);
    }
    instance
}

/// Model bounds from the compiled model's BBox section (author passes
/// `model_bounds` through; the compiled RW4 carries the same values).
fn model_bounds(
    resources: &connector::Resources,
    model_ids: &LodModelIds,
) -> Result<[f32; 6], String> {
    let bytes = resources
        .get(&model_ids[0])
        .ok_or("compiled model missing for bounds")?;
    let file = rw4::Rw4File::parse(bytes).map_err(|e| format!("compiled model parse: {e}"))?;
    let section = file
        .sections_of_type(rw4::SectionType::BBOX)
        .next()
        .ok_or("compiled model has no BBox section")?;
    let payload = file
        .payload(bytes, section.number)
        .map_err(|e| e.to_string())?;
    if payload.len() < 28 {
        return Err("compiled model BBox truncated".into());
    }
    let le = |at: usize| f32::from_le_bytes(payload[at..at + 4].try_into().unwrap());
    Ok([le(0), le(4), le(8), le(16), le(20), le(24)])
}

/// Rotate a prop/light local matrix about Z (`prop_contract.rotate_matrix`):
/// integer-quantized rotation `c = round(cos a); s = round(sin a)`, applied
/// to the three basis rows and the translation row alike.
fn rotate_matrix(matrix: &[f32], degrees: u32) -> Result<[f32; 12], String> {
    if matrix.len() != 12 {
        return Err("unit transform matrix must contain 12 floats".into());
    }
    let (c, s) = match degrees {
        0 => (1.0_f32, 0.0_f32),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        other => return Err(format!("朝向仅支持 0、90、180、270 度（收到 {other}）")),
    };
    let mut out = [0f32; 12];
    for row in 0..4 {
        let x = matrix[row * 3];
        let y = matrix[row * 3 + 1];
        let z = matrix[row * 3 + 2];
        out[row * 3] = c * x - s * y;
        out[row * 3 + 1] = s * x + c * y;
        out[row * 3 + 2] = z;
    }
    Ok(out)
}

const IDENTITY_12: [f32; 12] = [
    1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
];

/// `prop_contract.compile_fields` driven by the workbench prop units.
/// Instances group by (bin, resource id) exactly like the author groups by
/// preset; every group writes the five `0x0C12EF{20,30,40,50,60}+bin` fields.
fn compile_prop_fields(
    scene: Option<&SceneDocument>,
    degrees: u32,
) -> Result<Vec<Property>, String> {
    let Some(scene) = scene else {
        return Ok(Vec::new());
    };
    let mut groups: Vec<(usize, u32, Vec<Transform>)> = Vec::new();
    for unit in &scene.units {
        let SceneUnit::Prop {
            bin,
            resource_id,
            transform,
            scale,
            ..
        } = unit
        else {
            continue;
        };
        let Some(resource) = resource_id else {
            return Err("prop unit is missing resourceId/modelTgi.instance".into());
        };
        let matrix = match transform {
            Some(matrix) => rotate_matrix(&matrix.matrix, degrees)?,
            None => IDENTITY_12,
        };
        let scale = scale.unwrap_or(1.0);
        if !(0.001..=100.0).contains(&scale) {
            return Err("道具缩放超出支持范围".into());
        }
        let record = Transform {
            // Author '>H13f' with [15, scale, *m]: flags 15 carries the
            // unknown float slot, which the author uses for the scale.
            flags: 15,
            unknown: Some(scale),
            matrix: matrix.to_vec(),
        };
        let bin = *bin;
        match groups
            .iter_mut()
            .find(|(group_bin, group_resource, _)| *group_bin == bin && *group_resource == *resource)
        {
            Some(group) => group.2.push(record),
            None => groups.push((bin, *resource, vec![record])),
        }
    }
    let mut out = Vec::new();
    for (bin, resource, records) in groups {
        // Author semantics: the bin value IS the field offset
        // (`fields[202567456+bin]`); the author catalog uses 9..=12 (their
        // trees) and commercial777's control package used 10/11. Pass the
        // workbench bin through verbatim.
        let bin = bin as u32;
        // Resource key: wire order instance,type,group (author pack('>III')).
        out.push(array(
            PROP_RESOURCE_KEY + bin,
            PropType::Key,
            vec![zero_key(resource)],
        ));
        out.push(array(
            PROP_TRANSFORMS + bin,
            PropType::Transform,
            records
                .iter()
                .map(|record| Value::Transform(record.clone()))
                .collect(),
        ));
        out.push(array(
            PROP_INDICES + bin,
            PropType::Int32,
            (0..records.len() as i32).map(Value::Int32).collect(),
        ));
        for field in PROP_MARKERS {
            out.push(scalar(field + bin, PropType::Bool, Value::Bool(false)));
        }
    }
    Ok(out)
}

/// `light_contract.compile_fields` driven by the workbench light units.
fn compile_light_fields(
    scene: Option<&SceneDocument>,
    degrees: u32,
) -> Result<Vec<Property>, String> {
    let Some(scene) = scene else {
        return Ok(Vec::new());
    };
    let mut lights: Vec<(u32, [f32; 12], [f32; 3], f32, f32, f32, f32, f32, String)> = Vec::new();
    for unit in &scene.units {
        let SceneUnit::Light {
            transform,
            light_type,
            color,
            outer_radius,
            inner_radius,
            diffuse,
            specular,
            length,
            debug_name,
            ..
        } = unit
        else {
            continue;
        };
        let matrix = match transform {
            Some(matrix) => rotate_matrix(&matrix.matrix, degrees)?,
            None => IDENTITY_12,
        };
        // Author rows carry POINT/SPOT; LINE passes through verbatim (see
        // LIGHT_LINE_TYPE). Anything unrecognized falls back to POINT.
        let kind = match *light_type {
            Some(hash) if hash == LIGHT_SPOT_TYPE || hash == LIGHT_LINE_TYPE => hash,
            _ => LIGHT_POINT_TYPE,
        };
        let name = format!(
            "SC5_{}",
            debug_name.as_deref().unwrap_or("Light").chars().take(24).collect::<String>()
        );
        lights.push((
            kind,
            matrix,
            color.unwrap_or([1.0, 1.0, 1.0]),
            outer_radius.unwrap_or(10.0),
            inner_radius.unwrap_or(0.0),
            diffuse.unwrap_or(1.0),
            specular.unwrap_or(0.0),
            length.unwrap_or(0.1),
            name,
        ));
    }
    if lights.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (hash, prop_type) in LIGHT_COLUMNS {
        let values = lights
            .iter()
            .map(
                |(kind, matrix, color, radius, inner, intensity, specular, length, name)| {
                    match (hash, prop_type) {
                        (0x0CAA_8F10, _) => zero_key(LIGHT_GROUP),
                        (0x0CAA_8F11, _) => zero_key(*kind),
                        (0x0CAA_8F12, _) => Value::Transform(Transform {
                            // Author '>H12f' with [14, *m].
                            flags: 14,
                            unknown: None,
                            matrix: matrix.to_vec(),
                        }),
                        (0x0CAA_8F13, _) => Value::ColorRgb {
                            r: color[0],
                            g: color[1],
                            b: color[2],
                        },
                        (0x0CAA_8F14, _) => Value::Float(*radius),
                        (0x0CAA_8F15, _) => Value::Float(*inner),
                        (0x0CAA_8F16, _) => Value::Float(*intensity),
                        (0x0CAA_8F17, _) => Value::String8(name.clone()),
                        (0x0D4C_96B4, _) => Value::Float(*specular),
                        (0x0D4C_AD23, _) => Value::Float(*length),
                        (0x0E01_200F, _) => zero_key(CULL_TIER),
                        (0x0E4D_F244, _) => Value::Float(0.0),
                        (0x0E98_D445, _) => Value::Bool(false),
                        (0x0EC4_567E, _) => Value::Float(0.0),
                        _ => unreachable!("light column table is exhaustive"),
                    }
                },
            )
            .collect();
        out.push(array(hash, prop_type, values));
    }
    let (group_hash, group_type) = LIGHT_FIXED_GROUP;
    out.push(array(group_hash, group_type, vec![zero_key(LIGHT_GROUP)]));
    let (period_hash, period_type) = LIGHT_FIXED_NIGHT_PERIOD;
    out.push(array(period_hash, period_type, vec![zero_key(NIGHT_PERIOD)]));
    let (range_hash, range_type) = LIGHT_FIXED_NIGHT_RANGE;
    out.push(array(range_hash, range_type, vec![Value::Vector2(NIGHT_RANGE)]));
    let (zeros_hash, zeros_type) = LIGHT_FIXED_ZEROS;
    out.push(array(zeros_hash, zeros_type, vec![Value::Vector2([0.0, 0.0])]));
    Ok(out)
}

/// Build the parent-free Unit + Asset identities from the reference package.
///
/// `ground_lot` names the ground/lot property the numeric parcel fields and
/// the ground raster are read from (author: the sidecar `.lot.package`).
/// `instance` is the new shared identity for the Unit/Asset properties.
#[allow(clippy::too_many_arguments)]
pub fn build(
    spec: &MenuRegistrationSpec,
    resources: &connector::Resources,
    ground_lot: ResourceId,
    instance: u32,
    scene: Option<&SceneDocument>,
    model_ids: Option<&LodModelIds>,
    rotation_degrees: u32,
) -> Result<ParentFreeIdentity, String> {
    let (mut asset, sources) = flat_chain(resources, GENERIC_ASSET)?;
    // Restore the author-selected interaction/query fields from the deeper
    // 34CE7E14 chain (first occurrence wins, like the author's setdefault).
    let (interaction, interaction_sources) = flat_chain(resources, INTERACTION_CHAIN)?;
    let interaction_flag = interaction.get(INTERACTION_FIELDS[0]).cloned();
    for hash in INTERACTION_FIELDS {
        if let Some(property) = interaction.get(hash) {
            upsert(&mut asset, property.clone());
        }
    }
    if let Some(flag) = &interaction_flag {
        if flag.prop_type != PropType::Bool || flag.kind != Kind::Scalar(Value::Bool(true)) {
            return Err(format!(
                "generic interaction flag profile changed: expected 0AD90138=true, got {flag:?}"
            ));
        }
    }
    if resources
        .get(&ResourceId {
            type_id: GENERIC_MENU.type_id,
            group: GENERIC_MENU.group,
            instance: GENERIC_MENU.instance,
        })
        .is_none()
    {
        return Err(format!(
            "generic menu chain root {GENERIC_MENU} missing from the reference package"
        ));
    }
    // Ground property: re-encode every numeric Lot field into the Asset
    // dictionary; anything else must be one of the two ground key fields.
    let ground_bytes = resources
        .get(&ground_lot)
        .ok_or_else(|| format!("ground lot {ground_lot} missing from the reference package"))?;
    let ground =
        PropertyFile::parse(ground_bytes).map_err(|e| format!("ground lot parse: {e}"))?;
    for property in &ground.values {
        if LOT_TYPES.iter().any(|(hash, _)| *hash == property.hash) {
            upsert(&mut asset, encode_lot_numeric(property)?);
            continue;
        }
        if property.hash != HASH_PARENT
            && property.hash != GROUND_MODEL_KEY
            && property.hash != GROUND_ATLAS_KEY
            && property.hash != GROUND_MASK_HASH
        {
            return Err(format!(
                "ground lot carries unsupported field 0x{:08X} ({}) — export a minimal ground property",
                property.hash,
                property.prop_type.name()
            ));
        }
    }
    // Parcel geometry, rotated to the asset orientation (author order:
    // size > offset > period > baseTile > width/depth > ground box).
    let size = vector2(ground.get(HASH_LOT_SIZE)).ok_or("ground lot is missing LotSize (0x0CCB7FC8)")?;
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return Err("invalid parcel dimensions".into());
    }
    let offset = vector2(ground.get(HASH_LOT_OFFSET)).unwrap_or([0.0, 0.0]);
    let period = vector2(ground.get(HASH_LOT_PERIOD)).unwrap_or([8.0, 8.0]);
    let base_tile = match ground.get(HASH_BASE_TILE).and_then(|p| p.scalar()) {
        Some(Value::Int32(tile)) => *tile,
        _ => 1,
    };
    let (mut new_size, mut new_offset, mut new_period) = (size, offset, period);
    let mut mask = None;
    if let Some(mask_property) = ground.get(GROUND_MASK_HASH) {
        let Kind::Scalar(Value::Key(key)) = &mask_property.kind else {
            return Err("ground mask is not a Key".into());
        };
        let raw = resolve_raster(resources, key)?;
        let decoded = decode_ground_raster(&raw)?;
        let (rotated, rotated_size, rotated_offset, rotated_period) =
            rotate_lot(&decoded, size, offset, period, rotation_degrees)?;
        let encoded = super::materials::encode_rgba_raster(
            rotated.width(),
            rotated.height(),
            &rotated.pixels().map(|p| p.0).collect::<Vec<_>>(),
        )?;
        let instance = alloc_raster_instance(resources, &encoded);
        let mask_id = ResourceId {
            type_id: super::materials::raster_type(),
            group: 0,
            instance,
        };
        mask = Some((mask_id, encoded));
        upsert(&mut asset, scalar(GROUND_MASK_HASH, PropType::Key, key_value(Key {
            type_id: mask_id.type_id,
            group: mask_id.group,
            instance: mask_id.instance,
        })));
        new_size = rotated_size;
        new_offset = rotated_offset;
        new_period = rotated_period;
    }
    upsert(&mut asset, scalar(HASH_LOT_SIZE, PropType::Vector2, Value::Vector2(new_size)));
    upsert(&mut asset, scalar(HASH_LOT_OFFSET, PropType::Vector2, Value::Vector2(new_offset)));
    upsert(&mut asset, scalar(HASH_LOT_PERIOD, PropType::Vector2, Value::Vector2(new_period)));
    upsert(
        &mut asset,
        scalar(HASH_BASE_TILE, PropType::Int32, Value::Int32(base_tile)),
    );
    upsert(
        &mut asset,
        scalar(HASH_GROUND_WIDTH, PropType::Float, Value::Float(new_size[0])),
    );
    upsert(
        &mut asset,
        scalar(HASH_GROUND_DEPTH, PropType::Float, Value::Float(new_size[1])),
    );
    // Ground box + native ground atlases.
    let bounds = match model_ids {
        Some(model_ids) => model_bounds(resources, model_ids)?,
        None => [0.0, 0.0, 0.0, new_size[0], new_size[1], 1.0],
    };
    upsert(
        &mut asset,
        array(
            HASH_GROUND_BOX,
            PropType::BoundingBox,
            vec![Value::BoundingBox {
                min: [new_offset[0] - new_size[0] / 2.0, new_offset[1] - new_size[1] / 2.0, 0.0],
                max: [new_offset[0] + new_size[0] / 2.0, new_offset[1] + new_size[1] / 2.0, bounds[5]],
            }],
        ),
    );
    for atlas in [GROUND_ATLAS_A, GROUND_ATLAS_B] {
        if !resources.contains_key(&atlas) {
            return Err(format!(
                "native ground atlas {atlas} missing from the reference package"
            ));
        }
    }
    upsert(&mut asset, scalar(GROUND_MODEL_KEY, PropType::Key, key_value(Key {
        type_id: GROUND_ATLAS_A.type_id,
        group: GROUND_ATLAS_A.group,
        instance: GROUND_ATLAS_A.instance,
    })));
    upsert(&mut asset, scalar(GROUND_ATLAS_KEY, PropType::Key, key_value(Key {
        type_id: GROUND_ATLAS_B.type_id,
        group: GROUND_ATLAS_B.group,
        instance: GROUND_ATLAS_B.instance,
    })));
    // LOD chain + model bounds (author `for i,h in enumerate(range(0x00f9efbb,…))`).
    if let Some(model_ids) = model_ids {
        for (hash, model) in LOD_HASHES.iter().zip(model_ids.iter()) {
            upsert(&mut asset, scalar(*hash, PropType::Key, key_value(Key {
                type_id: model.type_id,
                group: model.group,
                instance: model.instance,
            })));
        }
    }
    upsert(
        &mut asset,
        array(
            HASH_MODEL_BOUNDS,
            PropType::BoundingBox,
            vec![Value::BoundingBox {
                min: [bounds[0], bounds[1], bounds[2]],
                max: [bounds[3], bounds[4], bounds[5]],
            }],
        ),
    );
    // Props and lights → Asset dictionary columns (author compile_fields
    // wire format, byte-verified against prepare_asset.py output). Spawner
    // and path columns deferred until the base display is confirmed.
    for property in compile_prop_fields(scene, rotation_degrees)? {
        upsert(&mut asset, property);
    }
    for property in compile_light_fields(scene, rotation_degrees)? {
        upsert(&mut asset, property);
    }
    // Unit/Asset locale title edge (author `unit/asset[0x09fb78cb]=text(t,1)`).
    upsert(&mut asset, array(HASH_TITLE_LINK, PropType::Text, vec![text_value()]));
    // Interior controller enabler (report §5.3): without 0F169C46=true the
    // game never creates the per-building interior controller and the model
    // uploads fixed thresholds (1,1,1) — windows stay in one state forever
    // (day rooms visible, night lights never fire). The author's
    // build_candidate ships it via asset_bool_overrides.
    upsert(
        &mut asset,
        scalar(0x0F16_9C46, PropType::Bool, Value::Bool(true)),
    );
    asset.claimed_count = asset.values.len() as u32;

    // Unit = the author's generic tested-unit.bin + overrides
    // (`backend_compile.profile`: cost, maintenance, category instance).
    let mut unit = PropertyFile::parse(include_bytes!("../../resources/sc5/tested-unit.bin"))
        .map_err(|e| format!("tested-unit.bin parse: {e}"))?;
    if let Some(cost) = spec.construction_cost {
        upsert(&mut unit, scalar(HASH_COST, PropType::Float, Value::Float(cost as f32)));
    }
    if let Some(maintenance) = spec.maintenance_cost {
        upsert(
            &mut unit,
            scalar(HASH_MAINTENANCE, PropType::Float, Value::Float(maintenance as f32)),
        );
    }
    upsert(
        &mut unit,
        scalar(
            HASH_UNIT_CATEGORY,
            PropType::Key,
            zero_key(spec.category_instance.unwrap_or(0x95A9_D6FD)),
        ),
    );
    upsert(&mut unit, scalar(HASH_UNIT_ASSET, PropType::Key, key_value(Key {
        type_id: 0x00B1_B104,
        group: ASSET_GROUP,
        instance,
    })));
    upsert(&mut unit, array(HASH_TITLE_LINK, PropType::Text, vec![text_value()]));
    unit.claimed_count = unit.values.len() as u32;

    Ok(ParentFreeIdentity {
        asset,
        unit,
        mask,
        evidence: serde_json::json!({
            "profile": "project-native-display-v1",
            "genericSources": sources,
            "interactionSources": interaction_sources,
            "rotationDegrees": rotation_degrees,
            "props": compile_prop_count(scene),
            "lights": compile_light_count(scene),
        }),
    })
}

fn compile_prop_count(scene: Option<&SceneDocument>) -> usize {
    scene
        .map(|scene| {
            scene
                .units
                .iter()
                .filter(|unit| matches!(unit, SceneUnit::Prop { .. }))
                .count()
        })
        .unwrap_or(0)
}

fn compile_light_count(scene: Option<&SceneDocument>) -> usize {
    scene
        .map(|scene| {
            scene
                .units
                .iter()
                .filter(|unit| matches!(unit, SceneUnit::Light { .. }))
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m1::scene::{parse_document as parse_scene, SceneDocument};
    use serde_json::json;

    /// Decimal pins against the author's `light_native_profile.json` (the
    /// packager's measured native profile). NIGHT_PERIOD was once hand-
    /// transcribed as 0x1295_6239 — a digit swap of 0x1295_2639 — and every
    /// light in University v9-v11 stayed dark because the light→night-period
    /// binding never resolved. Asserting the decimal values makes any hex
    /// transcription error a loud test failure instead of a dark building.
    #[test]
    fn light_profile_constants_match_author_json() {
        assert_eq!(LIGHT_GROUP, 0x8E43_F125, "light_contract.GROUP");
        assert_eq!(
            NIGHT_PERIOD, 311_764_537,
            "light_native_profile.json nightPeriod == 0x1295_2639"
        );
        assert_eq!(
            CULL_TIER, 1_048_449_613,
            "light_native_profile.json cullTier == 0x3E7E_124D"
        );
        assert_eq!(
            LIGHT_POINT_TYPE, 0x75D4_C8CD,
            "light_contract LightTypes Point"
        );
        assert_eq!(
            LIGHT_SPOT_TYPE, 0x2F0F_F9FD,
            "light_contract LightTypes Spot"
        );
        assert_eq!(
            LIGHT_LINE_TYPE, 0x0820_ABAF,
            "vanilla fire station E917279C light #3 type (Line)"
        );
    }

    fn scene_with_prop() -> SceneDocument {
        let value = json!({
            "units": [{
                "kind": "prop",
                "bin": 0,
                "resourceId": 305419896u64,
                "scale": 2.0,
                "transform": {
                    "flags": 15,
                    "matrix": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 4.0, 5.0, 6.0]
                }
            }]
        });
        parse_scene(&value).expect("scene")
    }

    fn scene_with_light(kind: &str) -> SceneDocument {
        let value = json!({
            "units": [{
                "kind": "light",
                "lightType": kind,
                "color": [1.0, 0.5, 0.25],
                "outerRadius": 12.0,
                "innerRadius": 1.5,
                "diffuse": 3.0,
                "specular": 0.5,
                "length": 0.2,
                "debugName": "Window",
                "transform": {
                    "flags": 15,
                    "matrix": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 3.0]
                }
            }]
        });
        parse_scene(&value).expect("scene")
    }

    /// Light type column carries the authored type hash verbatim: LINE stays
    /// LINE (vanilla fire station E917279C light #3 uses 0x0820ABAF); it must
    /// not be coerced to POINT — that coercion was s1's "5 strips → 2 faint
    /// blobs" regression.
    #[test]
    fn light_fields_pass_authored_type_through() {
        let kind_of = |scene: &SceneDocument| {
            compile_light_fields(Some(scene), 0)
                .expect("light fields")
                .iter()
                .find(|p| p.hash == 0x0CAA_8F11)
                .and_then(|p| match &p.kind {
                    Kind::Array(values) => values.first(),
                    _ => None,
                })
                .cloned()
                .expect("light type column")
        };
        for (name, expected) in [
            ("Point", LIGHT_POINT_TYPE),
            ("Spot", LIGHT_SPOT_TYPE),
            ("Line", LIGHT_LINE_TYPE),
        ] {
            match kind_of(&scene_with_light(name)) {
                Value::Key(key) => assert_eq!(key.instance, expected, "lightType {name}"),
                other => panic!("light type must be a key, got {other:?}"),
            }
        }
    }

    /// Author wire: record header `>IHHII` with flags 0x809C, and the
    /// resource key value carries the instance in wire order (instance,type,group).
    #[test]
    fn prop_fields_match_author_wire() {
        let scene = scene_with_prop();
        let fields = compile_prop_fields(Some(&scene), 180).expect("prop fields");
        let encoded = PropertyFile {
            values: fields,
            claimed_count: 0,
        }
        .encode_canonical()
        .expect("encode");
        // First record: 0x0C12EF20, type 32, flags 0x809C, count 1, size 12.
        let header = struct_pack_header(&encoded, 0x0C12_EF20);
        assert_eq!(header, (0x0C12_EF20, 32, 0x809C, 1, 12));
        let payload = record_payload(&encoded, 0x0C12_EF20);
        // pack('>III', rid, 0, 0): instance first.
        assert_eq!(&payload[0..4], &305419896u32.to_be_bytes());
        assert_eq!(&payload[4..12], &[0; 8]);
        // Transforms record: type 56 (Transform), count 1, size 56.
        let transform_header = struct_pack_header(&encoded, 0x0C12_EF30);
        assert_eq!(transform_header, (0x0C12_EF30, 56, 0x809C, 1, 56));
    }

    /// Light columns land at the author's hashes with the fixed profile
    /// fields alongside; the point/spot split follows the type hash.
    #[test]
    fn light_fields_match_author_wire() {
        let scene = scene_with_light("Point");
        let fields = compile_light_fields(Some(&scene), 0).expect("light fields");
        let by_hash: std::collections::HashMap<u32, &Property> =
            fields.iter().map(|p| (p.hash, p)).collect();
        assert!(by_hash.contains_key(&0x0CAA_8F10));
        assert!(by_hash.contains_key(&0x0CE7_1B10));
        let kind = by_hash
            .get(&0x0CAA_8F11)
            .and_then(|p| match &p.kind {
                Kind::Array(values) => values.first(),
                _ => None,
            })
            .cloned()
            .expect("light type column");
        match kind {
            Value::Key(key) => assert_eq!(key.instance, LIGHT_POINT_TYPE),
            _ => panic!("light type must be a key"),
        }
        let spot = compile_light_fields(Some(&scene_with_light("Spot")), 0).expect("spot fields");
        let spot_kind = spot
            .iter()
            .find(|p| p.hash == 0x0CAA_8F11)
            .and_then(|p| match &p.kind {
                Kind::Array(values) => values.first(),
                _ => None,
            })
            .cloned()
            .expect("spot type column");
        match spot_kind {
            Value::Key(key) => assert_eq!(key.instance, LIGHT_SPOT_TYPE),
            _ => panic!("spot type must be a key"),
        }
    }

    /// rotate_matrix 180° negates x/y basis rows and translation.
    #[test]
    fn rotate_matrix_quantized_rotation() {
        let matrix = rotate_matrix(&IDENTITY_12, 180).expect("rotate");
        assert_eq!(matrix[0], -1.0);
        assert_eq!(matrix[4], -1.0);
        assert_eq!(matrix[8], 1.0);
        // Translation row flips too (author applies the same row transform).
        let translated = rotate_matrix(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 4.0, 5.0, 6.0], 270)
            .expect("rotate");
        assert!((translated[9] - 5.0).abs() < 1e-6);
        assert!((translated[10] + 4.0).abs() < 1e-6);
    }

    /// `encode_lot_numeric` rejects unknown hashes (fail closed, like the
    /// author's `require(hash_id in LOT_TYPES)`).
    #[test]
    fn encode_lot_numeric_rejects_unknown_field() {
        let property = scalar(0x1234_5678, PropType::Float, Value::Float(1.0));
        assert!(encode_lot_numeric(&property).is_err());
        let ok = scalar(0x0D7A_F046, PropType::Float, Value::Float(0.006));
        assert!(encode_lot_numeric(&ok).is_ok());
    }

    /// PropertyFile byte layout helper mirroring the author parser:
    /// array record = hash(u32) + type(u16) + flags(u16) + count(u32) + item(u32).
    fn struct_pack_header(encoded: &[u8], hash: u32) -> (u32, u16, u16, u32, u32) {
        let (offset, _) = record_span(encoded, hash);
        let read = |at: usize| u32::from_be_bytes(encoded[at..at + 4].try_into().unwrap());
        let read16 = |at: usize| u16::from_be_bytes(encoded[at..at + 2].try_into().unwrap());
        (
            read(offset),
            read16(offset + 4),
            read16(offset + 6),
            read(offset + 8),
            read(offset + 12),
        )
    }

    fn record_payload(encoded: &[u8], hash: u32) -> &[u8] {
        let (offset, size) = record_span(encoded, hash);
        &encoded[offset + 16..offset + size]
    }

    fn record_span(encoded: &[u8], hash: u32) -> (usize, usize) {
        // Scan records: count then (hash u32, type u16, flags u16) + payload.
        let count = u32::from_be_bytes(encoded[0..4].try_into().unwrap()) as usize;
        let mut pos = 4usize;
        for _ in 0..count {
            let start = pos;
            let record_hash = u32::from_be_bytes(encoded[pos..pos + 4].try_into().unwrap());
            let prop_type = u16::from_be_bytes(encoded[pos + 4..pos + 6].try_into().unwrap());
            let flags = u16::from_be_bytes(encoded[pos + 6..pos + 8].try_into().unwrap());
            pos += 8;
            if flags & 0x30 == 0 {
                pos += scalar_size(prop_type);
            } else {
                let items = u32::from_be_bytes(encoded[pos..pos + 4].try_into().unwrap()) as usize;
                let item = u32::from_be_bytes(encoded[pos + 4..pos + 8].try_into().unwrap()) as usize;
                pos += 8 + items * item;
            }
            if record_hash == hash {
                return (start, pos - start);
            }
        }
        panic!("record 0x{hash:08X} not found");
    }

    fn scalar_size(prop_type: u16) -> usize {
        match prop_type {
            1 => 1,
            9 | 10 | 13 => 4,
            32 => 12,
            34 => 8,
            48 => 8,
            49 | 50 => 12,
            51 | 52 => 16,
            57 => 24,
            other => panic!("variable/unknown type {other} in test helper"),
        }
    }
}
