use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};

use super::{CommandError, source_error};

// Retail Config/Properties.txt declares OptionMotionBlur. The config
// manager loads option variants from group 40470100 | setting. The stock
// On variant contains precisely one Float (108F460E = 1).
const ON: ResourceId = ResourceId {
    type_id: 0x00B1_B104,
    group: 0x4047_0101,
    instance: 0x108F_3B4C,
};
const STRENGTH: u32 = 0x108F_460E;
pub(super) const DEFAULT_STRENGTH: f32 = 1.0;

pub(super) struct MotionOption {
    pub bytes: Vec<u8>,
    pub strength: f32,
    file: PropertyFile,
}

pub(super) fn read(package: &Package) -> Result<Option<MotionOption>, CommandError> {
    let Some(entry) = package.entry(ON) else {
        return Ok(None);
    };
    let bytes = package.read(entry).map_err(source_error)?;
    let file = PropertyFile::parse(&bytes).map_err(source_error)?;
    let Some(Value::Float(strength)) = file.get(STRENGTH).and_then(Property::scalar) else {
        return Err(source_error("Motion blur option has no Float strength"));
    };
    if file.claimed_count as usize != file.values.len() || !strength.is_finite() || *strength < 0.0
    {
        return Err(source_error("Invalid motion blur option"));
    }
    Ok(Some(MotionOption {
        bytes,
        strength: *strength,
        file,
    }))
}

pub(super) fn entry(
    source: Option<MotionOption>,
    requested: Option<f32>,
) -> Result<Option<OverlayEntry>, CommandError> {
    let Some(value) = requested else {
        return Ok(source
            .filter(|option| option.strength != DEFAULT_STRENGTH)
            .map(|option| OverlayEntry::new(ON, option.bytes)));
    };
    if !(0.0..=1.0).contains(&value) {
        return Err(CommandError::new(
            "camera_edit_invalid",
            "Motion blur strength",
        ));
    }
    let mut file = source
        .map(|option| option.file)
        .unwrap_or_else(|| PropertyFile {
            claimed_count: 1,
            values: vec![Property {
                hash: STRENGTH,
                prop_type: PropType::Float,
                kind: Kind::Scalar(Value::Float(DEFAULT_STRENGTH)),
                encoding: PropertyEncoding::default(),
            }],
        });
    file.values
        .iter_mut()
        .find(|property| property.hash == STRENGTH)
        .ok_or_else(|| source_error("Motion blur option has no strength"))?
        .kind = Kind::Scalar(Value::Float(value));
    let data = file
        .encode_canonical()
        .map_err(|e| CommandError::new("camera_export_failed", e.to_string()))?;
    Ok(Some(OverlayEntry::new(ON, data)))
}
