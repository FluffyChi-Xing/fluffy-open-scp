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
mod model;

use std::fmt;

pub use combine::{AssetGroup, MODEL_DETAILS_HASH, combine_assets, has_model_details};
pub use error::{Error, Result};
pub use locale::{
    LOCALE_RESOURCE_TYPE, Locale, MODEL_RESOURCE_TYPE, NAME_PROPERTY_HASHES, collect_name_map,
    parse_string_table,
};
pub use model::{Key, Kind, PropType, Property, Text, Transform, Value};

/// A parsed `0x00B1B104` property list. Entries keep file order (hashes are
/// sorted in files written by the game, but this is not relied on).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PropertyFile {
    pub values: Vec<Property>,
    /// Entry count claimed by the header (parsing stops early on EOF; the
    /// C# port notes one known game file under-reports its payload).
    pub claimed_count: u32,
}

impl PropertyFile {
    pub fn parse(data: &[u8]) -> Result<PropertyFile> {
        let mut r = Reader { data, pos: 0 };
        let claimed_count = r.u32()?;

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

            let kind = if flags & 0x30 == 0 {
                Kind::Scalar(read_value(&mut r, prop_type, flags)?)
            } else if flags & 0x40 == 0 {
                let count = r.i32()?;
                let _item_size = r.i32()?;
                let count = usize::try_from(count).map_err(|_| Error::Truncated {
                    needed: 0,
                    at: r.pos(),
                    size: data.len(),
                })?;
                let mut vals = Vec::with_capacity(count.min(4096));
                for _ in 0..count {
                    vals.push(read_value(&mut r, prop_type, 0)?);
                }
                Kind::Array(vals)
            } else {
                Kind::Empty
            };

            values.push(Property {
                hash,
                prop_type,
                kind,
            });
        }

        Ok(PropertyFile {
            values,
            claimed_count,
        })
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

fn read_value(r: &mut Reader<'_>, prop_type: PropType, flags: u16) -> Result<Value> {
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
            Value::String8(String::from_utf8_lossy(r.bytes(length)?).into_owned())
        }
        PropType::String16 => {
            let count = r.i32()?;
            let count = usize::try_from(count).map_err(|_| Error::Truncated {
                needed: 0,
                at: r.pos(),
                size: r.len(),
            })?;
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
        let end = start + n;
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
