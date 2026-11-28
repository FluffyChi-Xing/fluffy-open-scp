use sc_properties::{Kind, PropType, PropertyFile, Value};

use super::{CameraCurve, CommandError, source_error};

// cTiltShiftInfo::FillFromProps (dev VA 0x79C780), SetCameraRelativeDoF and Update.
pub(super) const MODE: u32 = 0x0D2D_0452;
const MANUAL: [(u32, f32); 4] = [
    (0x0E9F_FE89, 1024.0),
    (0x0E9F_FE8A, 20.0),
    (0x0E9F_FE8B, 500.0),
    (0x0E9F_FE8C, 500.0),
];

// FillFromProps consumes these eleven columns as one quality-indexed table.
const AUTO: [u32; 11] = [
    0x0C00_63E9,
    0x0C00_63EA,
    0x0C00_63EB,
    0x0C00_63EC,
    0x0D50_8F2D,
    0x0C11_C5E5,
    0x0C11_C5E6,
    0x0C11_C5E4,
    0x0C00_63ED,
    0x0E3A_FF71,
    0x0E3A_FF70,
];

pub(super) fn editable(
    local: &PropertyFile,
    merged: &PropertyFile,
) -> Result<Vec<CameraCurve>, CommandError> {
    [(MODE, 0.0)]
        .into_iter()
        .chain(MANUAL)
        .map(|(hash, default)| {
            let property = merged.get(hash);
            let value = match property.map(|property| property.scalar()) {
                None => default,
                Some(Some(Value::Int32(value))) if hash == MODE && (0..=2).contains(value) => {
                    *value as f32
                }
                Some(Some(Value::Float(value))) if hash != MODE && value.is_finite() => *value,
                _ => return Err(source_error(format!("{hash:08X}"))),
            };
            Ok(CameraCurve {
                hash,
                samples: vec![value],
                inherited: property.is_some() && local.get(hash).is_none(),
            })
        })
        .collect()
}

pub(super) fn auto_table(file: &PropertyFile) -> Option<Vec<Vec<f32>>> {
    let columns = AUTO
        .into_iter()
        .map(|hash| {
            file.get(hash)?
                .array()?
                .iter()
                .map(|value| match value {
                    Value::Float(number) if number.is_finite() => Some(*number),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    let count = columns[0].len();
    if !(1..=3).contains(&count) || columns.iter().any(|column| column.len() != count) {
        return None;
    }
    Some(
        (0..count)
            .map(|index| columns.iter().map(|column| column[index]).collect())
            .collect(),
    )
}

pub(super) fn edit_kind(hash: u32, samples: &[f32]) -> Option<(PropType, Kind)> {
    let &[value] = samples else { return None };
    if hash == MODE && matches!(value, 0.0 | 1.0 | 2.0) {
        return Some((PropType::Int32, Kind::Scalar(Value::Int32(value as i32))));
    }
    if MANUAL.iter().any(|(field, _)| *field == hash)
        && value.is_finite()
        && (value > 0.0 || (hash == 0x0E9F_FE8A && value == 0.0))
    {
        return Some((PropType::Float, Kind::Scalar(Value::Float(value))));
    }
    None
}
