//! SimCity 属性列表（property list，资源类型 `0x00B1B104`）解析。
//!
//! 迁移自 C# `SimCityPak\PackageReader\Properties\`（Gibbed.Spore.Properties
//! 的 SimCity 分支）。格式为全大端：
//!
//! ```text
//! u32      count
//! repeat:
//!   u32    hash
//!   u16    type_id    （元素类型，查 PropType 表）
//!   u16    flags      （&0x30 区分标量/变体；&0x40 区分数组/空；&0x100 String8 短长度）
//!   标量:   值本体
//!   数组:   i32 count, i32 item_size, count × 值
//!   空变体: 无载荷
//! ```
//!
//! # 用法
//!
//! ```no_run
//! let data = std::fs::read("prop.dump").unwrap();
//! let file = sc_properties::PropertyFile::parse(&data).unwrap();
//! for prop in &file.values {
//!     println!("{:08X} {} = {}", prop.hash, prop.prop_type.name(), /* ... */ "");
//! }
//! ```

mod combine;
mod error;
pub mod locale;
mod lot;
mod lot_unit;
mod model;

use std::fmt;

pub use combine::{AssetGroup, MODEL_DETAILS_HASH, combine_assets, has_model_details};
pub use error::{Error, Result};
pub use locale::{
    LOCALE_RESOURCE_TYPE, Locale, MODEL_RESOURCE_TYPE, NAME_PROPERTY_HASHES, collect_name_map,
    parse_string_table,
};
pub use lot::{
    LOD1_MODEL_HASH, LOT_MASK_HASH, LOT_PLACEMENT_HASH, LOT_SIZE_HASH, LotEditorDocument,
    PROPERTY_RESOURCE_TYPE,
};
pub use lot_unit::{
    LotUnit, LotUnits, UnitField, UnitKey, UnitTransform, assemble_units,
};
pub use model::{Key, Kind, PropType, Property, PropertyEncoding, Text, Transform, Value};

/// A parsed `0x00B1B104` property list. Entries keep file order (hashes are
/// sorted in files written by the game, but this is not relied on).
#[derive(Debug, Clone, Copy)]
pub struct ParseLimits {
    pub max_entries: usize,
    pub max_array_items: usize,
    pub max_string_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_entries: 65_536,
            max_array_items: 65_536,
            max_string_bytes: 1 << 20,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct PropertyFile {
    pub values: Vec<Property>,
    /// Entry count claimed by the header (parsing stops early on EOF; the
    /// C# port notes one known game file under-reports its payload).
    pub claimed_count: u32,
}

impl PropertyFile {
    pub fn parse(data: &[u8]) -> Result<PropertyFile> {
        Self::parse_with_limits(data, ParseLimits::default())
    }

    pub fn parse_with_limits(data: &[u8], limits: ParseLimits) -> Result<PropertyFile> {
        let mut r = Reader { data, pos: 0 };
        let claimed_count = r.u32()?;
        if claimed_count as usize > limits.max_entries {
            return Err(Error::LimitExceeded("entry count"));
        }

        let mut values = Vec::new();
        // EOF tolerance mirrors the C# reader: one known game PROPs file
        // claims more entries than it carries.
        while values.len() < claimed_count as usize && r.pos() < data.len() {
            let hash = r.u32()?;
            let file_type = r.u16()?;
            let flags = r.u16()?;

            if values.iter().any(|p: &Property| p.hash == hash) {
                return Err(Error::DuplicateHash(hash));
            }

            let prop_type = PropType::from_file_type(file_type)
                .ok_or(Error::UnknownPropertyType(file_type, hash))?;

            let (kind, array_item_size) = if flags & 0x30 == 0 {
                (
                    Kind::Scalar(read_value(&mut r, prop_type, flags, limits)?),
                    None,
                )
            } else if flags & 0x40 == 0 {
                let count = r.i32()?;
                let item_size = r.i32()?;
                let count = usize::try_from(count).map_err(|_| Error::Truncated {
                    needed: 0,
                    at: r.pos(),
                    size: data.len(),
                })?;
                if count > limits.max_array_items {
                    return Err(Error::LimitExceeded("array item count"));
                }
                let mut vals = Vec::with_capacity(count.min(4096));
                for _ in 0..count {
                    vals.push(read_value(&mut r, prop_type, 0, limits)?);
                }
                (Kind::Array(vals), Some(item_size))
            } else {
                (Kind::Empty, None)
            };

            values.push(Property {
                hash,
                prop_type,
                kind,
                encoding: PropertyEncoding {
                    flags,
                    array_item_size,
                },
            });
        }

        Ok(PropertyFile {
            values,
            claimed_count,
        })
    }

    /// Encode a deterministic, semantically equivalent property resource.
    ///
    /// The encoder uses canonical entry ordering and preserves parsed encoding
    /// flags where they remain compatible with the property's shape.
    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        let mut entries: Vec<&Property> = self.values.iter().collect();
        entries.sort_by_key(|property| property.hash);

        let count = u32::try_from(entries.len()).map_err(|_| Error::EncodeSizeOverflow)?;
        let mut out = Vec::new();
        out.extend_from_slice(&count.to_be_bytes());
        for property in entries {
            out.extend_from_slice(&property.hash.to_be_bytes());
            out.extend_from_slice(&property.prop_type.file_type().to_be_bytes());
            let flags = canonical_flags(property)?;
            out.extend_from_slice(&flags.to_be_bytes());
            match &property.kind {
                Kind::Scalar(value) => write_value(&mut out, property, value, flags)?,
                Kind::Array(values) => {
                    let count = i32::try_from(values.len()).map_err(|_| Error::ArrayTooLarge {
                        hash: property.hash,
                    })?;
                    let item_size = property.encoding.array_item_size.unwrap_or(0);
                    if item_size < 0
                        || fixed_value_size(property.prop_type)
                            .is_some_and(|expected| item_size != expected)
                    {
                        return Err(Error::InvalidArrayItemSize {
                            hash: property.hash,
                            expected: fixed_value_size(property.prop_type).unwrap_or(0),
                            actual: item_size,
                        });
                    }
                    out.extend_from_slice(&count.to_be_bytes());
                    out.extend_from_slice(&item_size.to_be_bytes());
                    for value in values {
                        write_value(&mut out, property, value, 0)?;
                    }
                }
                Kind::Empty => {}
            }
        }
        Ok(out)
    }

    /// Look up an entry by hash.
    pub fn get(&self, hash: u32) -> Option<&Property> {
        self.values.iter().find(|p| p.hash == hash)
    }
}

impl fmt::Display for PropertyFile {
    /// `0x<hash8>  <Type>  = <value>` lines, sorted by hash (dump format of
    /// the C# `DumpProp`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries: Vec<&Property> = self.values.iter().collect();
        entries.sort_by_key(|p| p.hash);
        for p in entries {
            match &p.kind {
                Kind::Scalar(v) => {
                    writeln!(f, "0x{:08X}  {}  = {}", p.hash, p.prop_type.name(), v)?
                }
                Kind::Array(vals) => {
                    if vals.is_empty() {
                        writeln!(f, "0x{:08X}  {}[]  = <empty>", p.hash, p.prop_type.name())?;
                    }
                    for v in vals {
                        writeln!(f, "0x{:08X}  {}[]  = {}", p.hash, p.prop_type.name(), v)?;
                    }
                }
                Kind::Empty => {
                    writeln!(
                        f,
                        "0x{:08X}  {}  = <empty variant>",
                        p.hash,
                        p.prop_type.name()
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn canonical_flags(property: &Property) -> Result<u16> {
    let flags = match &property.kind {
        Kind::Scalar(_) if property.encoding.flags == 0 => 0,
        Kind::Array(_) if property.encoding.flags == 0 => 0x30,
        Kind::Empty if property.encoding.flags == 0 => 0x70,
        _ => property.encoding.flags,
    };
    let valid = match &property.kind {
        Kind::Scalar(_) => flags & 0x30 == 0,
        Kind::Array(_) => flags & 0x30 != 0 && flags & 0x40 == 0,
        Kind::Empty => flags & 0x30 != 0 && flags & 0x40 != 0,
    };
    if valid {
        Ok(flags)
    } else {
        Err(Error::InvalidFlags {
            hash: property.hash,
            flags,
        })
    }
}

fn fixed_value_size(prop_type: PropType) -> Option<i32> {
    Some(match prop_type {
        PropType::Bool => 1,
        PropType::Int32 | PropType::UInt32 | PropType::Float => 4,
        PropType::Key => 12,
        PropType::Text => 8,
        PropType::Vector2 => 8,
        PropType::Vector3 | PropType::ColorRgb => 12,
        PropType::Vector4 | PropType::ColorRgba => 16,
        PropType::BoundingBox => 24,
        PropType::Transform | PropType::String8 | PropType::String16 => return None,
    })
}

fn write_value(out: &mut Vec<u8>, property: &Property, value: &Value, flags: u16) -> Result<()> {
    let type_matches = matches!(
        (property.prop_type, value),
        (PropType::Bool, Value::Bool(_))
            | (PropType::Int32, Value::Int32(_))
            | (PropType::UInt32, Value::UInt32(_))
            | (PropType::Float, Value::Float(_))
            | (PropType::Key, Value::Key(_))
            | (PropType::Text, Value::Text(_))
            | (PropType::String8, Value::String8(_))
            | (PropType::String16, Value::String16(_))
            | (PropType::Vector2, Value::Vector2(_))
            | (PropType::Vector3, Value::Vector3(_))
            | (PropType::ColorRgb, Value::ColorRgb { .. })
            | (PropType::Vector4, Value::Vector4(_))
            | (PropType::ColorRgba, Value::ColorRgba { .. })
            | (PropType::Transform, Value::Transform(_))
            | (PropType::BoundingBox, Value::BoundingBox { .. })
    );
    if !type_matches {
        return Err(Error::ValueTypeMismatch {
            hash: property.hash,
            prop_type: property.prop_type,
        });
    }

    match value {
        Value::Bool(value) => out.push(u8::from(*value)),
        Value::Int32(value) => out.extend_from_slice(&value.to_be_bytes()),
        Value::UInt32(value) => out.extend_from_slice(&value.to_be_bytes()),
        Value::Float(value) => out.extend_from_slice(&value.to_be_bytes()),
        Value::Key(value) => {
            out.extend_from_slice(&value.instance.to_be_bytes());
            out.extend_from_slice(&value.type_id.to_be_bytes());
            out.extend_from_slice(&value.group.to_be_bytes());
        }
        Value::Text(value) => {
            out.extend_from_slice(&value.table_id.to_be_bytes());
            out.extend_from_slice(&value.instance_id.to_be_bytes());
        }
        Value::String8(value) => {
            let bytes = value.as_bytes();
            if flags & 0x100 != 0 {
                if bytes.len() > u8::MAX as usize {
                    return Err(Error::StringTooLong {
                        hash: property.hash,
                    });
                }
                out.extend_from_slice(&0u32.to_be_bytes());
                out.push(bytes.len() as u8);
            } else {
                let length = u32::try_from(bytes.len()).map_err(|_| Error::StringTooLong {
                    hash: property.hash,
                })?;
                out.extend_from_slice(&length.to_be_bytes());
            }
            out.extend_from_slice(bytes);
        }
        Value::String16(value) => {
            let units: Vec<u16> = value.encode_utf16().collect();
            let count = i32::try_from(units.len()).map_err(|_| Error::StringTooLong {
                hash: property.hash,
            })?;
            out.extend_from_slice(&count.to_be_bytes());
            for unit in units {
                out.extend_from_slice(&unit.to_be_bytes());
            }
        }
        Value::Vector2(values) => write_f32s(out, values),
        Value::Vector3(values) => write_f32s(out, values),
        Value::Vector4(values) => write_f32s(out, values),
        Value::ColorRgb { r, g, b } => write_f32s(out, &[*r, *g, *b]),
        Value::ColorRgba { r, g, b, a } => write_f32s(out, &[*r, *g, *b, *a]),
        Value::BoundingBox { min, max } => {
            write_f32s(out, min);
            write_f32s(out, max);
        }
        Value::Transform(transform) => {
            let expected = match transform.flags {
                0x0C => 3,
                0x0D => 4,
                _ => 12,
            };
            if transform.matrix.len() != expected
                || (transform.flags == 15) != transform.unknown.is_some()
            {
                return Err(Error::InvalidTransform {
                    hash: property.hash,
                });
            }
            out.extend_from_slice(&transform.flags.to_be_bytes());
            if let Some(unknown) = transform.unknown {
                out.extend_from_slice(&unknown.to_be_bytes());
            }
            write_f32s(out, &transform.matrix);
        }
    }
    Ok(())
}

fn write_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_be_bytes());
    }
}

fn read_value(
    r: &mut Reader<'_>,
    prop_type: PropType,
    flags: u16,
    limits: ParseLimits,
) -> Result<Value> {
    let v = match prop_type {
        PropType::Bool => Value::Bool(r.u8()? != 0),
        PropType::Int32 => Value::Int32(r.i32()?),
        PropType::UInt32 => Value::UInt32(r.u32()?),
        PropType::Float => Value::Float(r.f32()?),
        PropType::Key => Value::Key(Key {
            instance: r.u32()?,
            type_id: r.u32()?,
            group: r.u32()?,
        }),
        PropType::Text => Value::Text(Text {
            table_id: r.u32()?,
            instance_id: r.u32()?,
        }),
        PropType::String8 => {
            let mut length = r.u32()? as usize;
            // C# quirk: a zero length with flag 0x100 set is followed by a
            // one-byte length instead.
            if length == 0 && flags & 0x100 != 0 {
                length = r.u8()? as usize;
            }
            if length > limits.max_string_bytes {
                return Err(Error::LimitExceeded("string bytes"));
            }
            Value::String8(String::from_utf8_lossy(r.bytes(length)?).into_owned())
        }
        PropType::String16 => {
            let count = r.i32()?;
            let count = usize::try_from(count).map_err(|_| Error::Truncated {
                needed: 0,
                at: r.pos(),
                size: r.len(),
            })?;
            if count.checked_mul(2).is_none() || count * 2 > limits.max_string_bytes {
                return Err(Error::LimitExceeded("string bytes"));
            }
            let bytes = r.bytes(count * 2)?;
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_be_bytes(*p))
                .collect();
            Value::String16(String::from_utf16_lossy(&units))
        }
        PropType::Vector2 => Value::Vector2([r.f32()?, r.f32()?]),
        PropType::Vector3 => Value::Vector3([r.f32()?, r.f32()?, r.f32()?]),
        PropType::Vector4 => Value::Vector4([r.f32()?, r.f32()?, r.f32()?, r.f32()?]),
        PropType::ColorRgb => Value::ColorRgb {
            r: r.f32()?,
            g: r.f32()?,
            b: r.f32()?,
        },
        PropType::ColorRgba => Value::ColorRgba {
            r: r.f32()?,
            g: r.f32()?,
            b: r.f32()?,
            a: r.f32()?,
        },
        PropType::BoundingBox => Value::BoundingBox {
            min: [r.f32()?, r.f32()?, r.f32()?],
            max: [r.f32()?, r.f32()?, r.f32()?],
        },
        PropType::Transform => {
            let tflags = r.u16()?;
            let count = match tflags {
                0x0C => 3,
                0x0D => 4,
                _ => 12,
            };
            let mut unknown = None;
            if tflags == 15 {
                unknown = Some(r.f32()?);
            }
            let mut matrix = Vec::with_capacity(count);
            for _ in 0..count {
                matrix.push(r.f32()?);
            }
            Value::Transform(Transform {
                flags: tflags,
                unknown,
                matrix,
            })
        }
    };
    Ok(v)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn pos(&self) -> usize {
        self.pos
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let start = self.pos;
        let end = start.checked_add(n).ok_or(Error::Truncated {
            needed: n,
            at: start,
            size: self.data.len(),
        })?;
        let slice = self.data.get(start..end).ok_or(Error::Truncated {
            needed: n,
            at: start,
            size: self.data.len(),
        })?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n)
    }
}

#[cfg(test)]
mod tests;
