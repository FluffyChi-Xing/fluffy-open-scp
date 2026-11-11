//! Road frontage uses the Lot frame's +Y edge. The game applies the
//! placement transform after aligning that frame to the road, and uses
//! its inverse when drawing the Lot overlay.

use sc_properties::light_alignment::Transform34;
use sc_properties::{
    Kind, LOT_PLACEMENT_HASH, LOT_SIZE_HASH, PropType, Property, PropertyEncoding, PropertyFile,
    Transform, Value,
};

const PLACEMENT_ARRAY_ENCODING: PropertyEncoding = PropertyEncoding {
    flags: 0x801C,
    array_item_size: Some(56),
};

pub(super) fn compensation(degrees: Option<u32>) -> Result<u32, String> {
    match degrees.unwrap_or(0) {
        0 => Ok(0),
        90 => Ok(270),
        180 => Ok(180),
        270 => Ok(90),
        value => Err(format!(
            "streetFacingDegrees: only 0/90/180/270 are supported (got {value})"
        )),
    }
}

fn frame_rotation(degrees: u32) -> Transform34 {
    let (sin, cos) = match degrees {
        90 => (1.0, 0.0),
        180 => (0.0, -1.0),
        270 => (-1.0, 0.0),
        _ => (0.0, 1.0),
    };
    Transform34 {
        axes: [[cos, sin, 0.0], [-sin, cos, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0; 3],
    }
}

/// The game samples the Lot mask at (+X, -Y). In the existing raster
/// helper a positive quarter-turn sends increasing columns to increasing
/// rows, so the extra turn for the street edge uses the opposite sign.
pub(super) fn mask_rotation(asset_rotation: u32, placement_turn: u32) -> u32 {
    (asset_rotation + 360 - placement_turn) % 360
}

pub(super) fn ground_offset(offset: [f32; 2], degrees: u32) -> [f32; 2] {
    let [x, y, _] = frame_rotation(degrees).transform_point([offset[0], offset[1], 0.0]);
    [x, y]
}

pub(super) fn generated_transform(file: &PropertyFile) -> Option<&Transform> {
    let property = file.get(LOT_PLACEMENT_HASH)?;
    if property.encoding.flags != PLACEMENT_ARRAY_ENCODING.flags
        || property.encoding.array_item_size != PLACEMENT_ARRAY_ENCODING.array_item_size
    {
        return None;
    }
    match &property.kind {
        Kind::Array(values) => match values.as_slice() {
            [Value::Transform(transform)] => Some(transform),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn apply(asset: &mut PropertyFile, degrees: u32) -> Result<Option<Transform>, String> {
    if degrees == 0 {
        return Ok(None);
    }
    let base = match asset.get(LOT_PLACEMENT_HASH).map(|property| &property.kind) {
        None => Transform34::identity(),
        Some(kind) => {
            let value = match kind {
                Kind::Scalar(value) => Some(value),
                Kind::Array(values) => values.first(),
                Kind::Empty => None,
            };
            let Some(Value::Transform(value)) = value else {
                return Err("street placement: missing base LotPlacementTransform value".into());
            };
            match value.matrix.as_slice() {
                [x, y, z] if value.flags == 12 => Transform34::from_translation([*x, *y, *z]),
                [_, x, y, z] if value.flags == 13 => Transform34::from_translation([*x, *y, *z]),
                _ => Transform34::from_transform(value)
                    .ok_or("street placement: invalid base LotPlacementTransform")?,
            }
        }
    };
    // Q changes the Lot basis. P' = Q ∘ P preserves the model-local Lot
    // origin, including when the existing placement has an offset.
    let placement = frame_rotation(degrees).compose(&base);
    let matrix: Vec<f32> = placement
        .axes
        .into_iter()
        .flatten()
        .chain(placement.translation)
        .collect();
    if matrix.iter().any(|value| !value.is_finite()) {
        return Err("street placement: non-finite LotPlacementTransform".into());
    }
    let transform = Transform {
        flags: 14,
        unknown: None,
        matrix,
    };
    write(asset, &transform);
    Ok(Some(transform))
}

pub(super) fn write(file: &mut PropertyFile, transform: &Transform) {
    // The engine's Variant conversion reads the first transform. Use
    // the one-item array layout found in native Lot dictionaries.
    file.values
        .retain(|property| property.hash != LOT_PLACEMENT_HASH);
    file.values.push(Property {
        hash: LOT_PLACEMENT_HASH,
        prop_type: PropType::Transform,
        kind: Kind::Array(vec![Value::Transform(transform.clone())]),
        encoding: PLACEMENT_ARRAY_ENCODING,
    });
}

pub(super) fn verify(
    resources: &super::connector::Resources,
    instance: u32,
    requested: u32,
    expected: &Transform,
) -> Result<serde_json::Value, String> {
    let mut asset = None;
    for group in [super::menu::lot_group(), super::menu::GRAPHICS_UNIT_GROUP] {
        let id = dbpf::ResourceId {
            type_id: super::menu::PROPERTY_TYPE,
            group,
            instance,
        };
        let bytes = resources
            .get(&id)
            .ok_or_else(|| format!("street placement: missing {id}"))?;
        let file = PropertyFile::parse(bytes)
            .map_err(|error| format!("street placement: {id}: {error}"))?;
        if generated_transform(&file) != Some(expected) {
            return Err(format!(
                "street placement: {id} lost its LotPlacementTransform"
            ));
        }
        if group == super::menu::lot_group() {
            asset = Some(file);
        }
    }
    let asset = asset.expect("checked the Asset dictionary");
    let Some(Value::Vector2(size)) = asset
        .get(LOT_SIZE_HASH)
        .and_then(|property| property.scalar())
    else {
        return Err("street placement: missing LotSize".into());
    };
    for (hash, dimension) in [(0x0BBB_CDED, size[0]), (0x0BBB_CE2A, size[1])] {
        if asset.get(hash).and_then(|property| property.scalar()) != Some(&Value::Float(dimension))
        {
            return Err(format!(
                "street placement: frontage/depth field {hash:08X} differs from LotSize"
            ));
        }
    }
    Ok(serde_json::json!({"edgeDegrees": requested, "lotSize": size, "transform": expected}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chosen_edge_maps_to_positive_y_and_scene_ground_points_keep_their_relative_position() {
        let point = [5.0, -9.0, 0.0];
        for (edge, normal) in [
            (0, [0.0, 1.0, 0.0]),
            (90, [-1.0, 0.0, 0.0]),
            (180, [0.0, -1.0, 0.0]),
            (270, [1.0, 0.0, 0.0]),
        ] {
            let turn = compensation(Some(edge)).unwrap();
            let q = frame_rotation(turn);
            assert_eq!(q.transform_point(normal), [0.0, 1.0, 0.0]);
            let undo = frame_rotation((360 - turn) % 360);
            assert_eq!(undo.transform_point(q.transform_point(point)), point);
            let mut asset = PropertyFile::default();
            let old = Transform {
                flags: 12,
                unknown: None,
                matrix: vec![3.0, 7.0, 0.0],
            };
            write(&mut asset, &old);
            if let Some(value) = apply(&mut asset, turn).unwrap() {
                let p = Transform34::from_transform(&value).unwrap();
                assert_eq!(undo.transform_point(p.translation), [3.0, 7.0, 0.0]);
            } else {
                assert_eq!(generated_transform(&asset), Some(&old));
            }
        }
        assert!(compensation(Some(45)).is_err());
    }

    #[test]
    fn ground_mask_samples_same_texel_after_changing_the_lot_basis() {
        let mask =
            image::RgbaImage::from_fn(8, 4, |x, y| image::Rgba([(x + y * 8) as u8, 0, 0, 255]));
        let size = [8.0, 4.0];
        let offset = [3.0, -2.0];
        for turn in [0, 90, 180, 270] {
            let (rotated, size_in_lot_frame, _, _) = super::super::native_profile::rotate_lot(
                &mask,
                size,
                offset,
                size,
                mask_rotation(0, turn),
            )
            .unwrap();
            let rotated_offset = ground_offset(offset, turn);
            for (x, y, pixel) in mask.enumerate_pixels() {
                let point = [
                    offset[0] + x as f32 + 0.5 - size[0] / 2.0,
                    offset[1] - y as f32 - 0.5 + size[1] / 2.0,
                ];
                let new = ground_offset(point, turn);
                let u = (new[0] - rotated_offset[0]) / size_in_lot_frame[0] + 0.5;
                let v = (rotated_offset[1] - new[1]) / size_in_lot_frame[1] + 0.5;
                assert_eq!(
                    rotated.get_pixel(
                        (u * rotated.width() as f32) as u32,
                        (v * rotated.height() as f32) as u32
                    ),
                    pixel
                );
            }
        }
    }
}
